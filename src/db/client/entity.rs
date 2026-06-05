//! Entity decomposition for cache writes, raw-response storage, and the
//! content-addressed `raw_responses` cache key.

use sha2::{Digest, Sha256};

use super::super::store::{TweetRow, UserRow};
use super::BirdClient;

// -- Entity classification --

#[derive(Clone, Copy)]
pub(super) enum EntityType {
    Tweet,
    User,
}

/// Classify a URL as an entity endpoint. Returns None for non-entity endpoints
/// (usage, auth, search/counts) which bypass entity decomposition.
pub(super) fn is_entity_endpoint(parsed: &url::Url) -> Option<EntityType> {
    let p = parsed.path();
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

/// Extract TweetRows from a JSON data value (array or single object).
pub(super) fn extract_tweets(data: &serde_json::Value, tweets: &mut Vec<TweetRow>) {
    if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(tweet) = TweetRow::from_api_json(item) {
                tweets.push(tweet);
            }
        }
    } else if data.is_object()
        && let Some(tweet) = TweetRow::from_api_json(data)
    {
        tweets.push(tweet);
    }
}

/// Extract UserRows from a JSON data value (array or single object).
pub(super) fn extract_users(data: &serde_json::Value, users: &mut Vec<UserRow>) {
    if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(user) = UserRow::from_api_json(item) {
                users.push(user);
            }
        }
    } else if data.is_object()
        && let Some(user) = UserRow::from_api_json(data)
    {
        users.push(user);
    }
}

// -- Raw response cache key --

/// SHA-256 key for raw_responses table (method + normalized URL, auth-agnostic).
pub(super) fn compute_raw_cache_key(method: &str, url: &str) -> String {
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

impl BirdClient {
    /// Decompose an API response into entities and upsert them.
    pub(super) fn decompose_and_upsert(&self, url: &str, json: &serde_json::Value) {
        let Some(ref db) = self.db else { return };
        let parsed = match url::Url::parse(url) {
            Ok(u) => u,
            Err(_) => return,
        };
        let Some(entity_type) = is_entity_endpoint(&parsed) else {
            return;
        };

        let mut tweets = Vec::new();
        let mut users = Vec::new();

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

        if let Some(includes) = json.get("includes")
            && let Some(inc_users) = includes.get("users").and_then(|u| u.as_array())
        {
            for item in inc_users {
                if let Some(user) = UserRow::from_api_json(item) {
                    users.push(user);
                }
            }
        }

        if let Some(errors) = json.get("errors").and_then(|e| e.as_array())
            && !errors.is_empty()
            && !self.quiet
        {
            let mut w = self.stderr.lock().unwrap();
            writeln!(
                *w,
                "[store] {} API error(s) in 200 response (processing available data)",
                errors.len()
            )
            .ok();
        }

        if let Err(e) = db.upsert_entities(&tweets, &users)
            && !self.quiet
        {
            let mut w = self.stderr.lock().unwrap();
            writeln!(*w, "[store] warning: entity upsert failed: {e}").ok();
        }
    }

    /// Store a raw response for non-entity endpoints.
    pub(super) fn store_raw_response(&self, url: &str, status: u16, body: &str) {
        let Some(ref db) = self.db else { return };
        let key = compute_raw_cache_key("GET", url);
        if let Err(e) = db.upsert_raw_response(&key, url, status, body.as_bytes())
            && !self.quiet
        {
            let mut w = self.stderr.lock().unwrap();
            writeln!(*w, "[store] warning: raw response store failed: {e}").ok();
        }
    }
}

#[cfg(all(test, not(feature = "embedded-xurl")))]
mod tests {
    use super::super::super::store::in_memory_db;
    use super::super::tests::test_client_with_db;
    use super::*;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).expect("test")
    }

    #[test]
    fn entity_endpoint_classification() {
        assert!(matches!(
            is_entity_endpoint(&parse(
                "https://api.x.com/2/tweets/search/recent?query=test"
            )),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint(&parse("https://api.x.com/2/tweets/123")),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint(&parse("https://api.x.com/2/tweets?ids=1,2,3")),
            Some(EntityType::Tweet)
        ));
        assert!(matches!(
            is_entity_endpoint(&parse("https://api.x.com/2/users/me")),
            Some(EntityType::User)
        ));
        assert!(matches!(
            is_entity_endpoint(&parse("https://api.x.com/2/users/by/username/jack")),
            Some(EntityType::User)
        ));
        assert!(matches!(
            is_entity_endpoint(&parse("https://api.x.com/2/users/123/bookmarks")),
            Some(EntityType::Tweet)
        ));
        assert!(is_entity_endpoint(&parse("https://api.x.com/2/usage/tweets")).is_none());
        assert!(
            is_entity_endpoint(&parse("https://api.x.com/2/tweets/search/counts/recent")).is_none()
        );
        assert!(is_entity_endpoint(&parse("https://api.x.com/2/oauth2/token")).is_none());
    }

    #[test]
    fn raw_cache_key_deterministic() {
        let key1 = compute_raw_cache_key("GET", "https://api.x.com/2/users/me");
        let key2 = compute_raw_cache_key("GET", "https://api.x.com/2/users/me");
        assert_eq!(key1, key2);
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
        let client = test_client_with_db(db);
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

        let db = client.db.as_ref().expect("test");
        assert!(db.get_tweet("t1").expect("test").is_some());
        assert!(db.get_tweet("t2").expect("test").is_some());
        assert!(db.get_user_by_username("alice").expect("test").is_some());
    }

    #[test]
    fn decompose_single_user() {
        let db = in_memory_db();
        let client = test_client_with_db(db);
        let json = serde_json::json!({
            "data": {"id": "u1", "username": "jack", "name": "Jack"}
        });
        client.decompose_and_upsert("https://api.x.com/2/users/by/username/jack", &json);

        let db = client.db.as_ref().expect("test");
        let user = db
            .get_user_by_username("jack")
            .expect("test")
            .expect("test");
        assert_eq!(user.id, "u1");
    }

    #[test]
    fn decompose_handles_absent_data() {
        let db = in_memory_db();
        let client = test_client_with_db(db);
        let json = serde_json::json!({
            "errors": [{"detail": "not found"}]
        });
        client.decompose_and_upsert("https://api.x.com/2/tweets?ids=999", &json);
    }

    #[test]
    fn decompose_non_entity_endpoint_is_noop() {
        let db = in_memory_db();
        let client = test_client_with_db(db);
        let json = serde_json::json!({"data": [{"id": "t1"}]});
        client.decompose_and_upsert("https://api.x.com/2/usage/tweets", &json);
        let db = client.db.as_ref().expect("test");
        assert!(db.get_tweet("t1").expect("test").is_none());
    }
}
