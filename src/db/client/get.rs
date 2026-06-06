//! Read path: entity-aware GET, batch ID partitioning, single-entity and
//! raw-response freshness lookups, direct (no-store) GET, xurl GET.

use std::collections::HashMap;

#[cfg(not(feature = "embedded-xurl"))]
use crate::cli::auth_scheme;

use super::super::store::{BirdDb, TweetRow};
use super::entity::{compute_raw_cache_key, is_entity_endpoint};
use super::{ApiResponse, BirdClient, RequestContext};

/// Extract batch IDs from `ids=` or `usernames=` query parameter.
pub(super) fn extract_batch_ids(parsed: &url::Url) -> Option<Vec<String>> {
    for (key, value) in parsed.query_pairs() {
        if key == "ids" || key == "usernames" {
            let ids: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !ids.is_empty() {
                return Some(ids);
            }
        }
    }
    None
}

/// Extract single tweet ID from path: `/2/tweets/{numeric_id}`
pub(super) fn extract_single_tweet_id(parsed: &url::Url) -> Option<String> {
    let parts: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() == 3 && parts[0] == "2" && parts[1] == "tweets" {
        let id = parts[2];
        if id.len() >= 2 && id.chars().all(|c| c.is_ascii_digit()) {
            return Some(id.to_string());
        }
    }
    None
}

/// Extract username from path: `/2/users/by/username/{username}`
pub(super) fn extract_username_from_url(parsed: &url::Url) -> Option<String> {
    let parts: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() == 5
        && parts[0] == "2"
        && parts[1] == "users"
        && parts[2] == "by"
        && parts[3] == "username"
    {
        return Some(parts[4].to_string());
    }
    None
}

/// Rebuild URL replacing the `ids=` parameter with a reduced set.
pub(super) fn rebuild_url_with_ids(url: &str, ids: &[String]) -> String {
    let mut parsed =
        url::Url::parse(url).expect("invariant: caller validated URL via earlier parse in get()");
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| k != "ids")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    parsed.query_pairs_mut().clear();
    parsed.query_pairs_mut().append_pair("ids", &ids.join(","));
    for (k, v) in pairs {
        parsed.query_pairs_mut().append_pair(&k, &v);
    }
    parsed.to_string()
}

/// Build an `ApiResponse` representing a store-only hit for a tweet batch.
fn build_store_response(rows: &[TweetRow]) -> Result<ApiResponse, serde_json::Error> {
    let data: Vec<serde_json::Value> = rows
        .iter()
        .filter_map(|t| serde_json::from_str(&t.raw_json).ok())
        .collect();
    let json = serde_json::json!({"data": data});
    Ok(ApiResponse {
        status: 200,
        cached_body: None,
        cache_hit: true,
        json: Some(json),
    })
}

/// Check if a single tweet is fresh in the store. Returns response if fresh.
pub(super) fn check_tweet_freshness(db: &BirdDb, id: &str) -> Option<ApiResponse> {
    let tweet = db.get_tweet(id).ok()??;
    if BirdDb::is_stale(tweet.last_refreshed_at, chrono::Utc::now()) {
        return None;
    }
    let jv: serde_json::Value = serde_json::from_str(&tweet.raw_json).ok()?;
    let json = serde_json::json!({"data": jv});
    Some(ApiResponse {
        status: 200,
        cached_body: None,
        cache_hit: true,
        json: Some(json),
    })
}

/// Check if a user (by username) is fresh in the store. Returns response if fresh.
pub(super) fn check_user_freshness(db: &BirdDb, username: &str) -> Option<ApiResponse> {
    let user = db.get_user_by_username(username).ok()??;
    if BirdDb::is_stale(user.last_refreshed_at, chrono::Utc::now()) {
        return None;
    }
    let jv: serde_json::Value = serde_json::from_str(&user.raw_json).ok()?;
    let json = serde_json::json!({"data": jv});
    Some(ApiResponse {
        status: 200,
        cached_body: None,
        cache_hit: true,
        json: Some(json),
    })
}

/// Try serving from the raw_responses table. Stores the exact bytes the API
/// emitted in `cached_body` so `body()` returns `Cow::Borrowed` (the cache
/// key here is content-addressed; preserving the original payload matters).
pub(super) fn try_raw_response(db: &BirdDb, url: &str) -> Option<ApiResponse> {
    let key = compute_raw_cache_key("GET", url);
    let raw = db.get_raw_response(&key).ok()??;
    let body = String::from_utf8_lossy(&raw.body).into_owned();
    let json = serde_json::from_str(&body).ok();
    Some(ApiResponse {
        status: raw.status_code as u16,
        cached_body: Some(body),
        cache_hit: true,
        json,
    })
}

impl BirdClient {
    /// Entity-aware GET. For entity endpoints: checks store freshness, splits batch IDs,
    /// decomposes responses into entities, and merges results.
    /// For non-entity endpoints: stores raw responses.
    pub fn get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        if self.cache_opts.no_store || self.db.is_none() {
            return self.direct_get(url, ctx);
        }

        let parsed_url = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
        let entity_type = is_entity_endpoint(&parsed_url);
        let skip_reads = self.cache_opts.refresh && !self.cache_opts.cache_only;

        if entity_type.is_some() && !skip_reads {
            if let Some(ids) = extract_batch_ids(&parsed_url) {
                return self.batch_get(url, ctx, &ids);
            }
            if let Some(tweet_id) = extract_single_tweet_id(&parsed_url) {
                let hit = {
                    let db = self
                        .db
                        .as_ref()
                        .expect("invariant: db.is_none() short-circuits at get() entry");
                    check_tweet_freshness(db, &tweet_id)
                };
                if let Some(resp) = hit {
                    self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                    return Ok(resp);
                }
            }
            if let Some(username) = extract_username_from_url(&parsed_url) {
                let hit = {
                    let db = self
                        .db
                        .as_ref()
                        .expect("invariant: db.is_none() short-circuits at get() entry");
                    check_user_freshness(db, &username)
                };
                if let Some(resp) = hit {
                    self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                    return Ok(resp);
                }
            }
        }

        if self.cache_opts.cache_only {
            let hit = {
                let db = self
                    .db
                    .as_ref()
                    .expect("invariant: db.is_none() short-circuits at get() entry");
                try_raw_response(db, url)
            };
            if let Some(resp) = hit {
                self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                return Ok(resp);
            }
            return Err("entity not in local store; run without --cache-only to fetch".into());
        }

        let response = self.xurl_get(url, ctx)?;

        if response.is_success()
            && let Some(ref jv) = response.json
        {
            if entity_type.is_some() {
                self.decompose_and_upsert(url, jv);
            } else {
                self.store_raw_response(url, response.status, &response.body());
            }
        }

        self.log_api_call(url, "GET", response.json.as_ref(), false, ctx.username);
        Ok(response)
    }

    /// Build xurl args for a GET request with auth and username flags.
    #[cfg(not(feature = "embedded-xurl"))]
    fn build_get_args(&self, url: &str, ctx: &RequestContext<'_>) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if let Some(flag) = auth_scheme::auth_flag(ctx.auth_type) {
            args.extend_from_slice(&["--auth".into(), flag.into()]);
        }
        if let Some(ref username) = self.username {
            args.extend_from_slice(&["-u".into(), username.clone()]);
        }
        args.push(url.into());
        args
    }

    /// GET via xurl transport. Returns ApiResponse with parsed JSON.
    fn xurl_get(
        &self,
        url: &str,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(not(feature = "embedded-xurl"))]
        {
            let args = self.build_get_args(url, ctx);
            let json_value = self.transport.request(&args)?;
            Ok(ApiResponse {
                status: 200,
                cached_body: None,
                cache_hit: false,
                json: Some(json_value),
            })
        }
        #[cfg(feature = "embedded-xurl")]
        {
            let json = self.xurl_send_raw_url("GET", url, "", ctx)?;
            Ok(ApiResponse {
                status: 200,
                cached_body: None,
                cache_hit: false,
                json: Some(json),
            })
        }
    }

    /// Direct GET without store interaction (for no_store / no db paths).
    fn direct_get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.xurl_get(url, ctx)?;
        self.log_api_call(url, "GET", response.json.as_ref(), false, ctx.username);
        Ok(response)
    }

    /// Batch ID get: partition into fresh (from store) vs stale/missing (from API), merge.
    fn batch_get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
        ids: &[String],
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (from_store, ids_to_fetch) = {
            let db = self
                .db
                .as_ref()
                .expect("invariant: db.is_none() short-circuits at get() entry");
            let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            db.partition_ids(&id_refs)?
        };

        // All fresh -> serve entirely from store
        if ids_to_fetch.is_empty() {
            let response = build_store_response(&from_store)?;
            self.log_api_call(url, "GET", response.json.as_ref(), true, ctx.username);
            return Ok(response);
        }

        // cache_only: return what we have
        if self.cache_opts.cache_only {
            if from_store.is_empty() {
                return Err("entity not in local store; run without --cache-only to fetch".into());
            }
            let response = build_store_response(&from_store)?;
            self.log_api_call(url, "GET", response.json.as_ref(), true, ctx.username);
            return Ok(response);
        }

        // No store hits -> standard request (no URL rebuild needed)
        if from_store.is_empty() {
            let response = self.xurl_get(url, ctx)?;
            if response.is_success()
                && let Some(ref jv) = response.json
            {
                self.decompose_and_upsert(url, jv);
            }
            self.log_api_call(url, "GET", response.json.as_ref(), false, ctx.username);
            return Ok(response);
        }

        // Mixed: split request — fetch only stale/missing IDs
        let fetch_url = rebuild_url_with_ids(url, &ids_to_fetch);
        let response = self.xurl_get(&fetch_url, ctx)?;
        let response_status = response.status;
        let api_json = response.json.unwrap_or(serde_json::Value::Null);

        if (200..300).contains(&response_status) {
            self.decompose_and_upsert(&fetch_url, &api_json);
        }

        // Build merged response: combine API + store data in original ID order
        let mut api_data: HashMap<String, serde_json::Value> = HashMap::new();
        if let Some(data) = api_json.get("data") {
            for item in data.as_array().into_iter().flatten() {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    api_data.insert(id.to_string(), item.clone());
                }
            }
        }

        let store_map: HashMap<&str, &TweetRow> =
            from_store.iter().map(|t| (t.id.as_str(), t)).collect();

        let mut merged: Vec<serde_json::Value> = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(item) = api_data.get(id) {
                merged.push(item.clone());
            } else if let Some(tweet) = store_map.get(id.as_str())
                && let Ok(j) = serde_json::from_str(&tweet.raw_json)
            {
                merged.push(j);
            }
        }

        let mut merged_json = serde_json::json!({"data": merged});
        if let Some(includes) = api_json.get("includes") {
            merged_json["includes"] = includes.clone();
        }
        if let Some(meta) = api_json.get("meta") {
            merged_json["meta"] = meta.clone();
        }
        if let Some(errors) = api_json.get("errors") {
            merged_json["errors"] = errors.clone();
        }

        self.log_api_call(&fetch_url, "GET", Some(&api_json), false, ctx.username);

        Ok(ApiResponse {
            status: response_status,
            cached_body: None,
            cache_hit: false,
            json: Some(merged_json),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::store::{TweetRow, UserRow, in_memory_db};
    use super::*;
    use crate::db::unix_now;
    use std::borrow::Cow;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).expect("test")
    }

    #[test]
    fn batch_ids_extraction() {
        assert_eq!(
            extract_batch_ids(&parse(
                "https://api.x.com/2/tweets?ids=1,2,3&tweet.fields=text"
            )),
            Some(vec!["1".into(), "2".into(), "3".into()])
        );
        assert_eq!(
            extract_batch_ids(&parse("https://api.x.com/2/users/by?usernames=alice,bob")),
            Some(vec!["alice".into(), "bob".into()])
        );
        assert!(
            extract_batch_ids(&parse(
                "https://api.x.com/2/tweets/search/recent?query=rust"
            ))
            .is_none()
        );
        assert!(extract_batch_ids(&parse("https://api.x.com/2/users/me")).is_none());
    }

    #[test]
    fn single_tweet_id_extraction() {
        assert_eq!(
            extract_single_tweet_id(&parse("https://api.x.com/2/tweets/1234567890")),
            Some("1234567890".into())
        );
        assert!(
            extract_single_tweet_id(&parse("https://api.x.com/2/tweets/search/recent")).is_none()
        );
        assert!(extract_single_tweet_id(&parse("https://api.x.com/2/tweets/1")).is_none());
    }

    #[test]
    fn username_extraction() {
        assert_eq!(
            extract_username_from_url(&parse("https://api.x.com/2/users/by/username/jack")),
            Some("jack".into())
        );
        assert!(extract_username_from_url(&parse("https://api.x.com/2/users/me")).is_none());
    }

    #[test]
    fn url_rebuild_with_ids() {
        let url = "https://api.x.com/2/tweets?ids=1,2,3&tweet.fields=text";
        let rebuilt = rebuild_url_with_ids(url, &["2".into(), "3".into()]);
        assert!(rebuilt.contains("ids=2%2C3") || rebuilt.contains("ids=2,3"));
        assert!(rebuilt.contains("tweet.fields=text"));
        assert!(!rebuilt.contains("ids=1"));
    }

    #[test]
    fn check_tweet_freshness_returns_fresh() {
        let db = in_memory_db();
        db.upsert_tweet(&TweetRow {
            id: "t1".into(),
            author_id: Some("u1".into()),
            conversation_id: None,
            raw_json: r#"{"id":"t1","text":"hello"}"#.into(),
            last_refreshed_at: unix_now(),
        })
        .expect("test");

        let resp = check_tweet_freshness(&db, "t1");
        assert!(resp.is_some());
        let resp = resp.expect("test");
        assert!(resp.cache_hit);
        // Cache hits must leave `cached_body` empty so we don't round-trip
        // the JSON through `serde_json::to_string`. Callers that need a
        // `&str` pay the conversion lazily via `body()`.
        assert!(
            resp.cached_body.is_none(),
            "cache hit must not store a re-serialized body"
        );
        assert!(resp.json.is_some(), "cache hit should keep parsed json");
        assert!(matches!(resp.body(), Cow::Owned(_)));
        assert!(resp.body().contains("t1"));
    }

    #[test]
    fn check_tweet_freshness_returns_none_for_stale() {
        let db = in_memory_db();
        db.upsert_tweet(&TweetRow {
            id: "t1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"t1"}"#.into(),
            last_refreshed_at: 1000,
        })
        .expect("test");
        assert!(check_tweet_freshness(&db, "t1").is_none());
    }

    #[test]
    fn check_user_freshness_returns_fresh() {
        let db = in_memory_db();
        db.upsert_user(&UserRow {
            id: "u1".into(),
            username: Some("alice".into()),
            raw_json: r#"{"id":"u1","username":"alice"}"#.into(),
            last_refreshed_at: unix_now(),
        })
        .expect("test");

        let resp = check_user_freshness(&db, "alice").expect("cache hit expected");
        assert!(resp.cache_hit);
        assert!(
            resp.cached_body.is_none(),
            "cache hit must not store a re-serialized body"
        );
        assert!(resp.json.is_some());
    }

    #[test]
    fn try_raw_response_returns_stored() {
        // The raw_responses table preserves exact API bytes; `body()` must
        // return them borrowed (no re-serialization, no allocation).
        let db = in_memory_db();
        let url = "https://api.x.com/2/usage/tweets";
        let key = compute_raw_cache_key("GET", url);
        db.upsert_raw_response(&key, url, 200, b"test body")
            .expect("test");

        let resp = try_raw_response(&db, url).expect("cache hit expected");
        assert!(resp.cache_hit);
        assert!(matches!(resp.body(), Cow::Borrowed(_)));
        assert_eq!(resp.body(), "test body");
    }
}
