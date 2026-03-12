//! BirdClient: entity-aware transport layer replacing CachedClient.
//! Handles UTC-day freshness, batch ID splitting, entity decomposition, and response merging.

use crate::config::ResolvedConfig;
use crate::cost;
use crate::requirements::AuthType;

use super::db::{BirdDb, TweetRow, UserRow};
use super::normalize_endpoint;

use reqwest::header::HeaderMap;
use reqwest_oauth1::OAuthClientProvider;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

// -- Shared types (re-exported from db::mod) --

/// Request context for usage logging and auth tracking.
pub struct RequestContext<'a> {
    pub auth_type: &'a AuthType,
    pub username: Option<&'a str>,
}

/// Store control options from CLI flags.
/// Flag precedence (silent): `no_store` wins all; `cache_only` suppresses `refresh`.
#[derive(Default)]
pub struct CacheOpts {
    /// --no-cache: disable store entirely (no reads, no writes)
    pub no_store: bool,
    /// --refresh: skip store reads, still write entities
    pub refresh: bool,
    /// --cache-only: serve from store only, no API calls
    pub cache_only: bool,
}

/// Response from BirdClient (covers both store hits and fresh API responses).
pub struct ApiResponse {
    pub status: u16,
    pub body: String,
    pub cache_hit: bool,
    /// Pre-parsed JSON body (populated by transport methods to avoid double-parse).
    pub json: Option<serde_json::Value>,
}

impl ApiResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

impl fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("cache_hit", &self.cache_hit)
            .field("body_len", &self.body.len())
            .finish()
    }
}

// -- Entity classification --

#[derive(Clone, Copy)]
enum EntityType {
    Tweet,
    User,
}

/// Classify a URL as an entity endpoint. Returns None for non-entity endpoints
/// (usage, auth, search/counts) which bypass entity decomposition.
fn is_entity_endpoint(url: &str) -> Option<EntityType> {
    let path = url::Url::parse(url).ok()?.path().to_string();
    let p = path.as_str();
    if (p.starts_with("/2/users/") && p.contains("/bookmarks"))
        || (p.starts_with("/2/tweets") && !p.starts_with("/2/tweets/search/counts"))
    {
        Some(EntityType::Tweet)
    } else if p.starts_with("/2/users") && !p.starts_with("/2/usage") {
        Some(EntityType::User)
    } else {
        None
    }
}

/// Extract batch IDs from `ids=` or `usernames=` query parameter.
fn extract_batch_ids(url: &str) -> Option<Vec<String>> {
    let parsed = url::Url::parse(url).ok()?;
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
fn extract_single_tweet_id(url: &str) -> Option<String> {
    let path = url::Url::parse(url).ok()?.path().to_string();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() == 3 && parts[0] == "2" && parts[1] == "tweets" {
        let id = parts[2];
        if id.len() >= 2 && id.chars().all(|c| c.is_ascii_digit()) {
            return Some(id.to_string());
        }
    }
    None
}

/// Extract username from path: `/2/users/by/username/{username}`
fn extract_username_from_url(url: &str) -> Option<String> {
    let path = url::Url::parse(url).ok()?.path().to_string();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
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
fn rebuild_url_with_ids(url: &str, ids: &[String]) -> String {
    let mut parsed = url::Url::parse(url).unwrap();
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

// -- Raw response cache key --

/// SHA-256 key for raw_responses table (method + normalized URL, auth-agnostic).
fn compute_raw_cache_key(method: &str, url: &str) -> String {
    let normalized = normalize_url(url);
    let input = format!("{}\0{}", method, normalized);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(hash)
}

/// Normalize URL: sort query parameters and known ID lists.
fn normalize_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };
    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    for (key, value) in &mut pairs {
        if matches!(key.as_str(), "ids" | "usernames") {
            let mut parts: Vec<&str> = value.split(',').collect();
            parts.sort();
            *value = parts.join(",");
        }
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let path = parsed.path();
    if pairs.is_empty() {
        format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            path
        )
    } else {
        let query: String = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        format!(
            "{}://{}{}?{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            path,
            query
        )
    }
}

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

// -- BirdClient --

/// Entity-aware transport layer. Wraps reqwest::Client + optional BirdDb.
/// If BirdDb is unavailable (corrupted, disk error), degrades to direct HTTP.
pub struct BirdClient {
    http: reqwest::Client,
    db: Option<BirdDb>,
    cache_opts: CacheOpts,
}

impl BirdClient {
    /// Create a new BirdClient. If entity store cannot be opened, degrades to no-store.
    pub fn new(
        http: reqwest::Client,
        store_path: &Path,
        cache_opts: CacheOpts,
        max_size_mb: u64,
    ) -> Self {
        if cache_opts.no_store {
            return Self {
                http,
                db: None,
                cache_opts,
            };
        }
        let db = match BirdDb::open(store_path, max_size_mb) {
            Ok(db) => {
                // Migrate usage data from old cache.db on first run
                if let Some(parent) = store_path.parent() {
                    let old_cache = parent.join("cache.db");
                    if old_cache.exists() {
                        db.migrate_usage_from_cache(&old_cache);
                    }
                }
                // Prune stale raw_responses and oversized entity tables
                if let Err(e) = db.prune_if_needed() {
                    eprintln!("[store] warning: pruning failed: {e}");
                }
                Some(db)
            }
            Err(e) => {
                eprintln!("[store] warning: failed to open entity store: {e}");
                eprintln!("[store] Run `bird cache clear` to reset the store.");
                None
            }
        };
        Self {
            http,
            db,
            cache_opts,
        }
    }

    /// Entity-aware GET. For entity endpoints: checks store freshness, splits batch IDs,
    /// decomposes responses into entities, and merges results.
    /// For non-entity endpoints: stores raw responses.
    pub async fn get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        // no_store or no db: direct HTTP
        if self.cache_opts.no_store || self.db.is_none() {
            return self.direct_get(url, ctx, headers).await;
        }

        let entity_type = is_entity_endpoint(url);
        // Effective refresh: cache_only suppresses refresh
        let skip_reads = self.cache_opts.refresh && !self.cache_opts.cache_only;

        // Entity store optimizations (skip when --refresh is active)
        if entity_type.is_some() && !skip_reads {
            // Batch ID splitting
            if let Some(ids) = extract_batch_ids(url) {
                return self.batch_get(url, ctx, headers, &ids).await;
            }
            // Single tweet freshness check
            if let Some(tweet_id) = extract_single_tweet_id(url) {
                let hit = {
                    let db = self.db.as_ref().unwrap();
                    check_tweet_freshness(db, &tweet_id)
                };
                if let Some(resp) = hit {
                    self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                    return Ok(resp);
                }
            }
            // Username freshness check
            if let Some(username) = extract_username_from_url(url) {
                let hit = {
                    let db = self.db.as_ref().unwrap();
                    check_user_freshness(db, &username)
                };
                if let Some(resp) = hit {
                    self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                    return Ok(resp);
                }
            }
        }

        // cache_only: last resort — try raw_response
        if self.cache_opts.cache_only {
            let hit = {
                let db = self.db.as_ref().unwrap();
                try_raw_response(db, url)
            };
            if let Some(resp) = hit {
                self.log_api_call(url, "GET", resp.json.as_ref(), true, ctx.username);
                return Ok(resp);
            }
            return Err("entity not in local store; run without --cache-only to fetch".into());
        }

        // Standard: HTTP GET + entity decomposition
        let response = self.http_get(url, headers).await?;
        let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();

        if response.is_success() {
            if let Some(ref jv) = json {
                if entity_type.is_some() {
                    self.decompose_and_upsert(url, jv);
                } else {
                    self.store_raw_response(url, response.status, &response.body);
                }
            }
        }

        self.log_api_call(url, "GET", json.as_ref(), false, ctx.username);
        Ok(ApiResponse { json, ..response })
    }

    /// POST/PUT/DELETE — pass-through, no entity store interaction.
    pub async fn request(
        &mut self,
        method: reqwest::Method,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: HeaderMap,
        body: Option<String>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let method_str = method.as_str().to_string();
        let mut req = self.http.request(method, url).headers(headers);
        if let Some(b) = body {
            req = req.body(b);
        }
        let res = req.send().await?;
        let status = res.status().as_u16();
        let text = res.text().await?;
        let json: Option<serde_json::Value> = serde_json::from_str(&text).ok();
        self.log_api_call(url, &method_str, json.as_ref(), false, ctx.username);
        Ok(ApiResponse {
            status,
            body: text,
            cache_hit: false,
            json,
        })
    }

    /// OAuth1-signed HTTP request. GET requests get entity decomposition;
    /// POST/PUT/DELETE bypass the store (mutations must never be cached).
    pub async fn oauth1_request(
        &mut self,
        method: &str,
        url: &str,
        config: &ResolvedConfig,
        body: Option<&str>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.oauth1_http(method, url, config, body).await?;
        let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();

        // Entity decomposition for successful GET responses
        if method == "GET" && response.is_success() && !self.cache_opts.no_store {
            if let Some(ref jv) = json {
                if is_entity_endpoint(url).is_some() {
                    self.decompose_and_upsert(url, jv);
                }
            }
        }

        let username = config.username.as_deref();
        self.log_api_call(url, method, json.as_ref(), false, username);
        Ok(ApiResponse { json, ..response })
    }

    /// Inner HTTP client ref (for auth operations that bypass store).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Direct HTTP GET (bypasses store). Does NOT log.
    pub async fn http_get(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let res = self.http.get(url).headers(headers).send().await?;
        let status = res.status().as_u16();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            cache_hit: false,
            json: None,
        })
    }

    /// Get entity store stats (None if store unavailable).
    pub fn db_stats(&self) -> Option<Result<super::db::StoreStats, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.stats())
    }

    /// Clear entity data (None if store unavailable).
    pub fn db_clear(&self) -> Option<Result<u64, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.clear())
    }

    /// Get the store DB path.
    pub fn db_path(&self) -> Option<PathBuf> {
        self.db.as_ref().and_then(|db| db.path())
    }

    /// Access the underlying BirdDb (for usage queries).
    pub fn db(&self) -> Option<&BirdDb> {
        self.db.as_ref()
    }

    /// Access the underlying BirdDb mutably (for usage logging).
    pub fn db_mut(&mut self) -> Option<&mut BirdDb> {
        self.db.as_mut()
    }

    /// Whether store is explicitly disabled (--no-cache flag).
    pub fn db_disabled(&self) -> bool {
        self.cache_opts.no_store
    }

    /// Log an API call to the usage database. Non-fatal: errors are warned to stderr.
    pub fn log_api_call(
        &mut self,
        url: &str,
        method: &str,
        json: Option<&serde_json::Value>,
        cache_hit: bool,
        username: Option<&str>,
    ) {
        let Some(ref mut db) = self.db else { return };
        let endpoint = normalize_endpoint(url);
        let null = serde_json::Value::Null;
        let json = json.unwrap_or(&null);
        let estimate = cost::estimate_raw_cost(json, &endpoint);
        let object_type = if estimate.users_read > 0 && estimate.tweets_read == 0 {
            "user"
        } else if estimate.tweets_read > 0 {
            "tweet"
        } else {
            "none"
        };
        if let Err(e) = db.log_usage(&super::UsageLogEntry {
            endpoint: &endpoint,
            method,
            object_type,
            object_count: (estimate.tweets_read + estimate.users_read) as i64,
            estimated_cost: estimate.estimated_usd,
            cache_hit,
            username,
        }) {
            eprintln!("[usage] warning: failed to log API call: {e}");
        }
    }

    // -- Private helpers --

    /// Direct GET without store interaction (for no_store / no db paths).
    async fn direct_get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.http_get(url, headers).await?;
        let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
        self.log_api_call(url, "GET", json.as_ref(), false, ctx.username);
        Ok(ApiResponse { json, ..response })
    }

    /// Batch ID get: partition into fresh (from store) vs stale/missing (from API), merge.
    async fn batch_get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: HeaderMap,
        ids: &[String],
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (from_store, ids_to_fetch) = {
            let db = self.db.as_ref().unwrap();
            let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            db.partition_ids(&id_refs)?
        };

        // All fresh -> serve entirely from store
        if ids_to_fetch.is_empty() {
            let data: Vec<serde_json::Value> = from_store
                .iter()
                .filter_map(|t| serde_json::from_str(&t.raw_json).ok())
                .collect();
            let json = serde_json::json!({"data": data});
            let body = serde_json::to_string(&json)?;
            self.log_api_call(url, "GET", Some(&json), true, ctx.username);
            return Ok(ApiResponse {
                status: 200,
                body,
                cache_hit: true,
                json: Some(json),
            });
        }

        // cache_only: return what we have
        if self.cache_opts.cache_only {
            if from_store.is_empty() {
                return Err("entity not in local store; run without --cache-only to fetch".into());
            }
            let data: Vec<serde_json::Value> = from_store
                .iter()
                .filter_map(|t| serde_json::from_str(&t.raw_json).ok())
                .collect();
            let json = serde_json::json!({"data": data});
            let body = serde_json::to_string(&json)?;
            self.log_api_call(url, "GET", Some(&json), true, ctx.username);
            return Ok(ApiResponse {
                status: 200,
                body,
                cache_hit: true,
                json: Some(json),
            });
        }

        // No store hits -> standard request (no URL rebuild needed)
        if from_store.is_empty() {
            let response = self.http_get(url, headers).await?;
            let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
            if response.is_success() {
                if let Some(ref jv) = json {
                    self.decompose_and_upsert(url, jv);
                }
            }
            self.log_api_call(url, "GET", json.as_ref(), false, ctx.username);
            return Ok(ApiResponse { json, ..response });
        }

        // Mixed: split request — fetch only stale/missing IDs
        let fetch_url = rebuild_url_with_ids(url, &ids_to_fetch);
        let response = self.http_get(&fetch_url, headers).await?;
        let api_json: serde_json::Value =
            serde_json::from_str(&response.body).unwrap_or(serde_json::Value::Null);

        if response.is_success() {
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

        let mut merged: Vec<serde_json::Value> = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(item) = api_data.get(id) {
                merged.push(item.clone());
            } else if let Some(tweet) = from_store.iter().find(|t| t.id == *id) {
                if let Ok(j) = serde_json::from_str(&tweet.raw_json) {
                    merged.push(j);
                }
            }
            // Deleted/missing IDs: omitted (correct per API semantics)
        }

        let mut merged_json = serde_json::json!({"data": merged});
        // Carry over API includes (v1: stored tweets' authors may be incomplete)
        if let Some(includes) = api_json.get("includes") {
            merged_json["includes"] = includes.clone();
        }
        if let Some(meta) = api_json.get("meta") {
            merged_json["meta"] = meta.clone();
        }
        if let Some(errors) = api_json.get("errors") {
            merged_json["errors"] = errors.clone();
        }

        let body = serde_json::to_string(&merged_json)?;
        // Log only API-fetched portion for accurate cost estimation
        self.log_api_call(&fetch_url, "GET", Some(&api_json), false, ctx.username);

        Ok(ApiResponse {
            status: response.status,
            body,
            cache_hit: false,
            json: Some(merged_json),
        })
    }

    /// Decompose an API response into entities and upsert them.
    fn decompose_and_upsert(&self, url: &str, json: &serde_json::Value) {
        let Some(ref db) = self.db else { return };
        let Some(entity_type) = is_entity_endpoint(url) else {
            return;
        };

        let mut tweets = Vec::new();
        let mut users = Vec::new();

        // Extract entities from data (single object or array)
        if let Some(data) = json.get("data") {
            match entity_type {
                EntityType::Tweet => {
                    extract_tweets(data, &mut tweets);
                }
                EntityType::User => {
                    extract_users(data, &mut users);
                }
            }
        }

        // Extract included users (deduplicated within response by API)
        if let Some(includes) = json.get("includes") {
            if let Some(inc_users) = includes.get("users").and_then(|u| u.as_array()) {
                for item in inc_users {
                    if let Some(user) = UserRow::from_api_json(item) {
                        users.push(user);
                    }
                }
            }
        }

        // Handle error-in-200 pattern: log but continue processing available data
        if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                eprintln!(
                    "[store] {} API error(s) in 200 response (processing available data)",
                    errors.len()
                );
            }
        }

        if let Err(e) = db.upsert_entities(&tweets, &users) {
            eprintln!("[store] warning: entity upsert failed: {e}");
        }
    }

    /// Store a raw response for non-entity endpoints.
    fn store_raw_response(&self, url: &str, status: u16, body: &str) {
        let Some(ref db) = self.db else { return };
        let key = compute_raw_cache_key("GET", url);
        if let Err(e) = db.upsert_raw_response(&key, url, status, body.as_bytes()) {
            eprintln!("[store] warning: raw response store failed: {e}");
        }
    }

    /// OAuth1-signed HTTP call. Extracts credentials, signs, sends. Does NOT log or store.
    async fn oauth1_http(
        &self,
        method: &str,
        url: &str,
        config: &ResolvedConfig,
        body: Option<&str>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ck = config
            .oauth1_consumer_key
            .as_ref()
            .ok_or("OAuth1 consumer key missing")?;
        let cs = config
            .oauth1_consumer_secret
            .as_ref()
            .ok_or("OAuth1 consumer secret missing")?;
        let at = config
            .oauth1_access_token
            .as_ref()
            .ok_or("OAuth1 access token missing")?;
        let ats = config
            .oauth1_access_token_secret
            .as_ref()
            .ok_or("OAuth1 access token secret missing")?;
        let secrets =
            reqwest_oauth1::Secrets::new(ck.as_str(), cs.as_str()).token(at.as_str(), ats.as_str());
        let mut req = match method {
            "GET" => self.http.clone().oauth1(secrets).get(url),
            "POST" => self.http.clone().oauth1(secrets).post(url),
            "PUT" => self.http.clone().oauth1(secrets).put(url),
            "DELETE" => self.http.clone().oauth1(secrets).delete(url),
            _ => return Err(format!("unsupported method: {method}").into()),
        };
        if let Some(b) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(b.to_string());
        }
        let res = req.send().await?;
        let status = res.status().as_u16();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            cache_hit: false,
            json: None,
        })
    }
}

// -- Free helper functions --

/// Extract TweetRows from a JSON data value (array or single object).
fn extract_tweets(data: &serde_json::Value, tweets: &mut Vec<TweetRow>) {
    if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(tweet) = TweetRow::from_api_json(item) {
                tweets.push(tweet);
            }
        }
    } else if data.is_object() {
        if let Some(tweet) = TweetRow::from_api_json(data) {
            tweets.push(tweet);
        }
    }
}

/// Extract UserRows from a JSON data value (array or single object).
fn extract_users(data: &serde_json::Value, users: &mut Vec<UserRow>) {
    if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(user) = UserRow::from_api_json(item) {
                users.push(user);
            }
        }
    } else if data.is_object() {
        if let Some(user) = UserRow::from_api_json(data) {
            users.push(user);
        }
    }
}

/// Check if a single tweet is fresh in the store. Returns response if fresh.
fn check_tweet_freshness(db: &BirdDb, id: &str) -> Option<ApiResponse> {
    let tweet = db.get_tweet(id).ok()??;
    if BirdDb::is_stale(tweet.last_refreshed_at, chrono::Utc::now()) {
        return None;
    }
    let jv: serde_json::Value = serde_json::from_str(&tweet.raw_json).ok()?;
    let json = serde_json::json!({"data": jv});
    let body = serde_json::to_string(&json).ok()?;
    Some(ApiResponse {
        status: 200,
        body,
        cache_hit: true,
        json: Some(json),
    })
}

/// Check if a user (by username) is fresh in the store. Returns response if fresh.
fn check_user_freshness(db: &BirdDb, username: &str) -> Option<ApiResponse> {
    let user = db.get_user_by_username(username).ok()??;
    if BirdDb::is_stale(user.last_refreshed_at, chrono::Utc::now()) {
        return None;
    }
    let jv: serde_json::Value = serde_json::from_str(&user.raw_json).ok()?;
    let json = serde_json::json!({"data": jv});
    let body = serde_json::to_string(&json).ok()?;
    Some(ApiResponse {
        status: 200,
        body,
        cache_hit: true,
        json: Some(json),
    })
}

/// Try serving from the raw_responses table.
fn try_raw_response(db: &BirdDb, url: &str) -> Option<ApiResponse> {
    let key = compute_raw_cache_key("GET", url);
    let raw = db.get_raw_response(&key).ok()??;
    let body = String::from_utf8_lossy(&raw.body).into_owned();
    let json = serde_json::from_str(&body).ok();
    Some(ApiResponse {
        status: raw.status_code as u16,
        body,
        cache_hit: true,
        json,
    })
}

#[cfg(test)]
mod tests {
    use super::super::db::in_memory_db;
    use super::super::unix_now;
    use super::*;

    #[test]
    fn entity_endpoint_classification() {
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/tweets/search/recent?query=test"),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/tweets/123"),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/tweets?ids=1,2,3"),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/users/me"),
            Some(EntityType::User)
        ));
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/users/by/username/jack"),
            Some(EntityType::User)
        ));
        assert!(matches!(
            is_entity_endpoint("https://api.x.com/2/users/123/bookmarks"),
            Some(EntityType::Tweet)
        ));
        // Non-entity endpoints
        assert!(is_entity_endpoint("https://api.x.com/2/usage/tweets").is_none());
        assert!(is_entity_endpoint("https://api.x.com/2/tweets/search/counts/recent").is_none());
        assert!(is_entity_endpoint("https://api.x.com/2/oauth2/token").is_none());
    }

    #[test]
    fn batch_ids_extraction() {
        assert_eq!(
            extract_batch_ids("https://api.x.com/2/tweets?ids=1,2,3&tweet.fields=text"),
            Some(vec!["1".into(), "2".into(), "3".into()])
        );
        assert_eq!(
            extract_batch_ids("https://api.x.com/2/users/by?usernames=alice,bob"),
            Some(vec!["alice".into(), "bob".into()])
        );
        assert!(extract_batch_ids("https://api.x.com/2/tweets/search/recent?query=rust").is_none());
        assert!(extract_batch_ids("https://api.x.com/2/users/me").is_none());
    }

    #[test]
    fn single_tweet_id_extraction() {
        assert_eq!(
            extract_single_tweet_id("https://api.x.com/2/tweets/1234567890"),
            Some("1234567890".into())
        );
        // Not a numeric ID
        assert!(extract_single_tweet_id("https://api.x.com/2/tweets/search/recent").is_none());
        // Too short
        assert!(extract_single_tweet_id("https://api.x.com/2/tweets/1").is_none());
    }

    #[test]
    fn username_extraction() {
        assert_eq!(
            extract_username_from_url("https://api.x.com/2/users/by/username/jack"),
            Some("jack".into())
        );
        assert!(extract_username_from_url("https://api.x.com/2/users/me").is_none());
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
    fn raw_cache_key_deterministic() {
        let key1 = compute_raw_cache_key("GET", "https://api.x.com/2/users/me");
        let key2 = compute_raw_cache_key("GET", "https://api.x.com/2/users/me");
        assert_eq!(key1, key2);
        // Different method -> different key
        let key3 = compute_raw_cache_key("POST", "https://api.x.com/2/users/me");
        assert_ne!(key1, key3);
    }

    #[test]
    fn raw_cache_key_normalizes_ids() {
        let key1 = compute_raw_cache_key("GET", "https://api.x.com/2/tweets?ids=3,1,2");
        let key2 = compute_raw_cache_key("GET", "https://api.x.com/2/tweets?ids=1,2,3");
        assert_eq!(key1, key2, "ID order should not affect cache key");
    }

    #[test]
    fn decompose_tweet_response() {
        let db = in_memory_db();
        let client = BirdClient {
            http: reqwest::Client::new(),
            db: Some(db),
            cache_opts: CacheOpts::default(),
        };
        let json = serde_json::json!({
            "data": [
                {"id": "t1", "text": "hello", "author_id": "u1"},
                {"id": "t2", "text": "world", "author_id": "u1"}
            ],
            "includes": {
                "users": [{"id": "u1", "username": "alice"}]
            }
        });
        client.decompose_and_upsert("https://api.x.com/2/tweets/search/recent", &json);

        let db = client.db.as_ref().unwrap();
        assert!(db.get_tweet("t1").unwrap().is_some());
        assert!(db.get_tweet("t2").unwrap().is_some());
        assert!(db.get_user_by_username("alice").unwrap().is_some());
    }

    #[test]
    fn decompose_single_user() {
        let db = in_memory_db();
        let client = BirdClient {
            http: reqwest::Client::new(),
            db: Some(db),
            cache_opts: CacheOpts::default(),
        };
        let json = serde_json::json!({
            "data": {"id": "u1", "username": "jack", "name": "Jack"}
        });
        client.decompose_and_upsert("https://api.x.com/2/users/by/username/jack", &json);

        let db = client.db.as_ref().unwrap();
        let user = db.get_user_by_username("jack").unwrap().unwrap();
        assert_eq!(user.id, "u1");
    }

    #[test]
    fn decompose_handles_absent_data() {
        let db = in_memory_db();
        let client = BirdClient {
            http: reqwest::Client::new(),
            db: Some(db),
            cache_opts: CacheOpts::default(),
        };
        // API returns no data key when all IDs deleted
        let json = serde_json::json!({
            "errors": [{"detail": "not found"}]
        });
        // Should not panic
        client.decompose_and_upsert("https://api.x.com/2/tweets?ids=999", &json);
    }

    #[test]
    fn decompose_non_entity_endpoint_is_noop() {
        let db = in_memory_db();
        let client = BirdClient {
            http: reqwest::Client::new(),
            db: Some(db),
            cache_opts: CacheOpts::default(),
        };
        let json = serde_json::json!({"data": [{"id": "t1"}]});
        // Usage endpoint is not an entity endpoint
        client.decompose_and_upsert("https://api.x.com/2/usage/tweets", &json);
        let db = client.db.as_ref().unwrap();
        assert!(db.get_tweet("t1").unwrap().is_none());
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
        .unwrap();

        let resp = check_tweet_freshness(&db, "t1");
        assert!(resp.is_some());
        let resp = resp.unwrap();
        assert!(resp.cache_hit);
        assert!(resp.body.contains("t1"));
    }

    #[test]
    fn check_tweet_freshness_returns_none_for_stale() {
        let db = in_memory_db();
        db.upsert_tweet(&TweetRow {
            id: "t1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"t1"}"#.into(),
            last_refreshed_at: 1000, // epoch 1970 — stale
        })
        .unwrap();
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
        .unwrap();

        let resp = check_user_freshness(&db, "alice");
        assert!(resp.is_some());
        assert!(resp.unwrap().cache_hit);
    }

    #[test]
    fn try_raw_response_returns_stored() {
        let db = in_memory_db();
        let url = "https://api.x.com/2/usage/tweets";
        let key = compute_raw_cache_key("GET", url);
        db.upsert_raw_response(&key, url, 200, b"test body")
            .unwrap();

        let resp = try_raw_response(&db, url);
        assert!(resp.is_some());
        let resp = resp.unwrap();
        assert!(resp.cache_hit);
        assert_eq!(resp.body, "test body");
    }

    #[test]
    fn api_response_debug_redacts_body() {
        let response = ApiResponse {
            status: 200,
            body: "sensitive data here".to_string(),
            cache_hit: true,
            json: None,
        };
        let debug = format!("{:?}", response);
        assert!(!debug.contains("sensitive data here"));
        assert!(debug.contains("body_len"));
    }

    #[test]
    fn cache_opts_default() {
        let opts = CacheOpts::default();
        assert!(!opts.no_store);
        assert!(!opts.refresh);
        assert!(!opts.cache_only);
    }

    /// Full workflow integration: search → profile → bookmarks → thread → usage.
    /// Simulates the entity store lifecycle across multiple command paths.
    #[test]
    fn full_workflow_entity_lifecycle() {
        use super::super::db::BookmarkRow;

        let db = in_memory_db();
        let mut client = BirdClient {
            http: reqwest::Client::new(),
            db: Some(db),
            cache_opts: CacheOpts::default(),
        };

        // --- Step 1: Search stores tweet + user entities ---
        let search_response = serde_json::json!({
            "data": [
                {"id": "t1", "text": "hello rust", "author_id": "u1", "conversation_id": "t1"},
                {"id": "t2", "text": "hello world", "author_id": "u2", "conversation_id": "t2"}
            ],
            "includes": {
                "users": [
                    {"id": "u1", "username": "alice", "name": "Alice"},
                    {"id": "u2", "username": "bob", "name": "Bob"}
                ]
            }
        });
        client.decompose_and_upsert("https://api.x.com/2/tweets/search/recent", &search_response);

        let db = client.db.as_ref().unwrap();
        assert!(
            db.get_tweet("t1").unwrap().is_some(),
            "search should store tweet t1"
        );
        assert!(
            db.get_tweet("t2").unwrap().is_some(),
            "search should store tweet t2"
        );
        assert!(
            db.get_user_by_username("alice").unwrap().is_some(),
            "search should store included user alice"
        );
        assert!(
            db.get_user_by_username("bob").unwrap().is_some(),
            "search should store included user bob"
        );

        // --- Step 2: Profile lookup finds stored user (freshness check) ---
        let alice_resp = check_user_freshness(db, "alice");
        assert!(
            alice_resp.is_some(),
            "profile should find fresh user alice from store"
        );
        let alice_resp = alice_resp.unwrap();
        assert!(alice_resp.cache_hit, "profile user should be a cache hit");
        assert!(
            alice_resp.body.contains("alice"),
            "profile response should contain username"
        );

        // --- Step 3: Bookmark storage with tweet entities ---
        let bookmark_rows = vec![
            BookmarkRow {
                account_username: "alice".into(),
                tweet_id: "t1".into(),
                position: 0,
                refreshed_at: unix_now(),
            },
            BookmarkRow {
                account_username: "alice".into(),
                tweet_id: "t2".into(),
                position: 1,
                refreshed_at: unix_now(),
            },
        ];
        db.replace_bookmarks("alice", &bookmark_rows).unwrap();
        let stored_bookmarks = db.get_bookmarks("alice").unwrap();
        assert_eq!(stored_bookmarks.len(), 2, "should store 2 bookmarks");
        assert_eq!(
            stored_bookmarks[0].tweet_id, "t1",
            "bookmark ordering preserved"
        );
        assert_eq!(
            stored_bookmarks[1].tweet_id, "t2",
            "bookmark ordering preserved"
        );

        // --- Step 4: Thread lookup — root tweet from store, conversation via partition ---
        let root_resp = check_tweet_freshness(db, "t1");
        assert!(root_resp.is_some(), "thread root tweet should be in store");
        assert!(
            root_resp.unwrap().cache_hit,
            "thread root should be cache hit"
        );

        // Partition IDs: t1 is fresh, t3 is missing
        let (from_store, to_fetch) = db.partition_ids(&["t1", "t3"]).unwrap();
        assert_eq!(from_store.len(), 1, "t1 should be fresh in store");
        assert_eq!(from_store[0].id, "t1");
        assert_eq!(to_fetch.len(), 1, "t3 should need fetching");
        assert_eq!(to_fetch[0], "t3");

        // --- Step 5: Usage logging ---
        let db_mut = client.db.as_mut().unwrap();
        db_mut
            .log_usage(&super::super::usage::UsageLogEntry {
                endpoint: "/2/tweets/search/recent",
                method: "GET",
                object_type: "tweets",
                object_count: 2,
                estimated_cost: 0.01,
                cache_hit: false,
                username: Some("alice"),
            })
            .unwrap();
        let summary = db_mut.query_usage_summary(0).unwrap();
        assert_eq!(summary.total_calls, 1, "usage should be logged");
        assert_eq!(summary.total_cost, 0.01);

        // --- Step 6: Stats reflect all stored entities ---
        let stats = db_mut.stats().unwrap();
        assert_eq!(stats.tweet_count, 2, "should have 2 tweets");
        assert_eq!(stats.user_count, 2, "should have 2 users");
        assert_eq!(stats.bookmark_count, 2, "should have 2 bookmarks");
    }
}
