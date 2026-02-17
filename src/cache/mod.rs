//! Cache layer: SQLite-backed HTTP response cache with transparent CachedClient wrapper.
//! BirdDb is the application database — cache now, usage tracking in Plan 4.
//! Cache failures are never fatal: the Option<BirdDb> pattern degrades to no-cache mode.

pub mod db;
pub mod usage;

// Re-export all public types so `use crate::cache::{...}` continues to work.
pub use db::{BirdDb, CacheStats};
pub use usage::{ActualUsageDay, DailyUsage, EndpointUsage, UsageLogEntry, UsageSummary};

use crate::config::ResolvedConfig;
use crate::cost;
use crate::requirements::AuthType;
use reqwest_oauth1::OAuthClientProvider;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache context for key computation (type-safe, not strings).
pub struct RequestContext<'a> {
    pub auth_type: &'a AuthType,
    pub username: Option<&'a str>,
}

/// Cache control options from CLI flags.
#[derive(Default)]
pub struct CacheOpts {
    pub no_cache: bool,
    pub refresh: bool,
    pub cache_ttl: Option<u64>,
}

// CacheOpts uses Default derive — all fields default to false/None

/// Response from CachedClient (covers both cache hits and fresh responses).
pub struct ApiResponse {
    pub status: reqwest::StatusCode,
    pub body: String,
    pub headers: reqwest::header::HeaderMap,
    pub cache_hit: bool,
    /// Pre-parsed JSON body (populated by transport methods to avoid double-parse).
    pub json: Option<serde_json::Value>,
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

/// Transparent caching wrapper around reqwest::Client.
/// If BirdDb is unavailable (corrupted, disk error), degrades to direct HTTP.
pub struct CachedClient {
    http: reqwest::Client,
    db: Option<BirdDb>,
    cache_opts: CacheOpts,
}

impl CachedClient {
    /// Create a new CachedClient. If cache DB cannot be opened, degrades to no-cache.
    pub fn new(
        http: reqwest::Client,
        cache_path: &Path,
        cache_opts: CacheOpts,
        max_size_mb: u64,
    ) -> Self {
        if cache_opts.no_cache {
            return Self {
                http,
                db: None,
                cache_opts,
            };
        }
        let db = match BirdDb::open(cache_path, max_size_mb) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("[cache] warning: failed to open cache database: {}", e);
                eprintln!("[cache] Run `bird cache clear` to reset the cache database.");
                None
            }
        };
        Self {
            http,
            db,
            cache_opts,
        }
    }

    /// GET request with transparent caching.
    pub async fn get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: reqwest::header::HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Never cache auth endpoints or paginated requests
        if should_skip_cache(url) || self.cache_opts.no_cache {
            let response = self.http_get(url, headers).await?;
            let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
            self.log_api_call(url, "GET", json.as_ref(), false, ctx.username);
            return Ok(ApiResponse { json, ..response });
        }

        let key = compute_cache_key("GET", url, ctx);
        let ttl = self.effective_ttl(url);

        // Try cache read (unless --refresh)
        if !self.cache_opts.refresh {
            if let Some(ref db) = self.db {
                match db.get(&key) {
                    Ok(Some(entry)) => {
                        let body = String::from_utf8_lossy(&entry.body).into_owned();
                        let json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
                        self.log_api_call(url, "GET", json.as_ref(), true, ctx.username);
                        return Ok(ApiResponse {
                            status: reqwest::StatusCode::from_u16(entry.status_code as u16)
                                .unwrap_or(reqwest::StatusCode::OK),
                            body,
                            headers: reqwest::header::HeaderMap::new(),
                            cache_hit: true,
                            json,
                        });
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("[cache] warning: read failed: {}", e);
                    }
                }
            }
        }

        // Cache miss — make HTTP request
        let response = self.http_get(url, headers).await?;

        // Write to cache (only 2xx responses)
        if response.status.is_success() {
            if let Some(ref mut db) = self.db {
                if let Err(e) = db.put(
                    &key,
                    url,
                    response.status.as_u16(),
                    response.body.as_bytes(),
                    ttl,
                ) {
                    eprintln!("[cache] warning: write failed: {}", e);
                }
            }
        }

        let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
        self.log_api_call(url, "GET", json.as_ref(), false, ctx.username);
        Ok(ApiResponse { json, ..response })
    }

    /// POST/PUT/DELETE — pass-through, no caching.
    pub async fn request(
        &mut self,
        method: reqwest::Method,
        url: &str,
        ctx: &RequestContext<'_>,
        headers: reqwest::header::HeaderMap,
        body: Option<String>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let method_str = method.as_str().to_string();
        let mut req = self.http.request(method, url).headers(headers);
        if let Some(b) = body {
            req = req.body(b);
        }
        let res = req.send().await?;
        let status = res.status();
        let resp_headers = res.headers().clone();
        let text = res.text().await?;
        let json: Option<serde_json::Value> = serde_json::from_str(&text).ok();
        self.log_api_call(url, &method_str, json.as_ref(), false, ctx.username);
        Ok(ApiResponse {
            status,
            body: text,
            headers: resp_headers,
            cache_hit: false,
            json,
        })
    }

    /// Inner HTTP client ref (for auth operations that bypass cache).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Get cache stats (None if cache unavailable).
    pub fn cache_stats(&self) -> Option<Result<CacheStats, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.stats())
    }

    /// Clear cache (None if cache unavailable).
    pub fn cache_clear(&self) -> Option<Result<u64, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.clear())
    }

    /// Get the cache DB path.
    pub fn cache_path(&self) -> Option<PathBuf> {
        self.db.as_ref().and_then(|db| db.path())
    }

    /// Access the underlying BirdDb (for usage queries).
    pub fn db(&self) -> Option<&BirdDb> {
        self.db.as_ref()
    }

    /// Whether caching is explicitly disabled (--no-cache flag).
    pub fn cache_disabled(&self) -> bool {
        self.cache_opts.no_cache
    }

    /// Log an API call to the usage database. Non-fatal: errors are warned to stderr.
    /// Accepts pre-parsed JSON to avoid redundant deserialization (callers parse once).
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
        if let Err(e) = db.log_usage(&UsageLogEntry {
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

    /// Direct HTTP GET (bypasses cache). Used for endpoints where fresh data is required.
    /// Does NOT log — callers (e.g. `get()`) handle logging with pre-parsed JSON.
    pub async fn http_get(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let res = self.http.get(url).headers(headers).send().await?;
        let status = res.status();
        let resp_headers = res.headers().clone();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            headers: resp_headers,
            cache_hit: false,
            json: None,
        })
    }

    /// OAuth1-signed HTTP request. Handles credential extraction, signing, logging.
    /// GET requests go through the cache layer; POST/PUT/DELETE bypass it (mutations must never be cached).
    pub async fn oauth1_request(
        &mut self,
        method: &str,
        url: &str,
        config: &ResolvedConfig,
        body: Option<&str>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = RequestContext {
            auth_type: &AuthType::OAuth1,
            username: config.username.as_deref(),
        };

        // Only cache GET requests; skip cache for mutations, pagination, and --no-cache
        let use_cache = method == "GET" && !should_skip_cache(url) && !self.cache_opts.no_cache;

        if use_cache {
            let key = compute_cache_key("GET", url, &ctx);
            let ttl = self.effective_ttl(url);

            // Try cache read (unless --refresh)
            if !self.cache_opts.refresh {
                if let Some(ref db) = self.db {
                    match db.get(&key) {
                        Ok(Some(entry)) => {
                            let body = String::from_utf8_lossy(&entry.body).into_owned();
                            let json: Option<serde_json::Value> =
                                serde_json::from_str(&body).ok();
                            self.log_api_call(
                                url,
                                "GET",
                                json.as_ref(),
                                true,
                                ctx.username,
                            );
                            return Ok(ApiResponse {
                                status: reqwest::StatusCode::from_u16(
                                    entry.status_code as u16,
                                )
                                .unwrap_or(reqwest::StatusCode::OK),
                                body,
                                headers: reqwest::header::HeaderMap::new(),
                                cache_hit: true,
                                json,
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("[cache] warning: read failed: {}", e);
                        }
                    }
                }
            }

            // Cache miss — extract credentials, sign, and send
            let response = self.oauth1_http(method, url, config, body).await?;

            // Write to cache (only 2xx responses)
            if response.status.is_success() {
                if let Some(ref mut db) = self.db {
                    if let Err(e) = db.put(
                        &key,
                        url,
                        response.status.as_u16(),
                        response.body.as_bytes(),
                        ttl,
                    ) {
                        eprintln!("[cache] warning: write failed: {}", e);
                    }
                }
            }

            let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
            self.log_api_call(url, method, json.as_ref(), false, ctx.username);
            Ok(ApiResponse { json, ..response })
        } else {
            // Non-cacheable path: mutations, pagination, --no-cache
            let response = self.oauth1_http(method, url, config, body).await?;
            let json: Option<serde_json::Value> = serde_json::from_str(&response.body).ok();
            self.log_api_call(url, method, json.as_ref(), false, ctx.username);
            Ok(ApiResponse { json, ..response })
        }
    }

    /// Inner OAuth1-signed HTTP call. Extracts credentials, signs, sends.
    /// Does NOT log or cache — callers handle that.
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
            _ => return Err(format!("unsupported method: {}", method).into()),
        };
        if let Some(b) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(b.to_string());
        }
        let res = req.send().await?;
        let status = res.status();
        let resp_headers = res.headers().clone();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            headers: resp_headers,
            cache_hit: false,
            json: None,
        })
    }

    fn effective_ttl(&self, url: &str) -> i64 {
        if let Some(ttl) = self.cache_opts.cache_ttl {
            // Cap at 24 hours to prevent stale-forever entries; safe i64 conversion
            return ttl.min(86400) as i64;
        }
        default_ttl_for_endpoint(url)
    }
}

/// Compute SHA-256 cache key from method + normalized URL + auth_type + username.
fn compute_cache_key(method: &str, url: &str, ctx: &RequestContext<'_>) -> String {
    let normalized = normalize_url(url);
    let auth_str = match ctx.auth_type {
        AuthType::OAuth2User => "oauth2_user",
        AuthType::OAuth1 => "oauth1",
        AuthType::Bearer => "bearer",
        AuthType::None => "none",
    };
    let username = ctx.username.unwrap_or("__app__");
    let input = format!("{}\0{}\0{}\0{}", method, normalized, auth_str, username);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(hash)
}

/// Normalize URL: sort query parameters, sort known ID lists.
fn normalize_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };

    debug_assert!(
        parsed.host_str() == Some("api.x.com"),
        "unexpected host: {:?}",
        parsed.host_str()
    );

    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    // Sort comma-separated values for known ID parameters
    for (key, value) in &mut pairs {
        if matches!(key.as_str(), "ids" | "usernames") {
            let mut parts: Vec<&str> = value.split(',').collect();
            parts.sort();
            *value = parts.join(",");
        }
    }

    // Sort by parameter key
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

/// Known literal path segments that should never be replaced with `:id`.
const KNOWN_LITERALS: &[&str] = &[
    "2",
    "tweets",
    "users",
    "search",
    "recent",
    "bookmarks",
    "me",
    "by",
    "username",
    "usage",
    "oauth2",
    "token",
    "compliance",
    "lists",
    "spaces",
    "dm_conversations",
];

/// Normalize a URL to an endpoint pattern for usage grouping.
/// Replaces numeric ID segments (2+ digits) with `:id` and the parameter after
/// `/by/username/` with `:username`. Returns the path only (strips scheme, host, query params).
pub fn normalize_endpoint(url: &str) -> String {
    let path = url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string());

    let segments: Vec<&str> = path.split('/').collect();
    let mut normalized = Vec::with_capacity(segments.len());
    let mut prev_two: (Option<&str>, Option<&str>) = (None, None); // (prev-prev, prev)

    for seg in &segments {
        if seg.is_empty() {
            normalized.push(*seg);
            continue;
        }
        // After "/by/username/", the next segment is a username parameter
        if prev_two == (Some("by"), Some("username")) {
            normalized.push(":username");
        } else if KNOWN_LITERALS.contains(seg) {
            // Known literal path components (checked before numeric to preserve "2")
            normalized.push(seg);
        } else if seg.len() >= 2 && seg.chars().all(|c| c.is_ascii_digit()) {
            // Numeric ID segments (2+ digits to avoid matching version numbers like "2")
            normalized.push(":id");
        } else {
            // Unknown non-numeric segment — keep as-is (future-proofs new endpoints)
            normalized.push(seg);
        }
        prev_two = (prev_two.1, Some(seg));
    }

    normalized.join("/")
}

/// Whether to skip caching for this URL.
fn should_skip_cache(url: &str) -> bool {
    url.contains("/oauth2/token")
        || url.contains("pagination_token=")
        || url.contains("next_token=")
}

/// Per-endpoint TTL defaults (seconds). Most-specific pattern wins.
fn default_ttl_for_endpoint(url: &str) -> i64 {
    let path = url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_default();

    if path.starts_with("/2/users/") && path.contains("/bookmarks") {
        return 900; // 15 min
    }
    if path.starts_with("/2/tweets/search/") {
        return 900; // 15 min
    }
    if path.starts_with("/2/users/by/") || path.starts_with("/2/users/") {
        return 3600; // 1 hour
    }
    if path.starts_with("/2/tweets/") {
        return 900; // 15 min
    }
    900 // default 15 min
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// We need the `hex` encoding for SHA-256 output — use a minimal inline implementation
// to avoid adding another dependency.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_includes_all_components() {
        let ctx1 = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: Some("alice"),
        };
        let ctx2 = RequestContext {
            auth_type: &AuthType::Bearer,
            username: Some("alice"),
        };
        let ctx3 = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: Some("bob"),
        };

        let key1 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx1);
        let key2 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx2);
        let key3 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx3);

        // All should be different
        assert_ne!(
            key1, key2,
            "different auth_type should produce different keys"
        );
        assert_ne!(
            key1, key3,
            "different username should produce different keys"
        );
    }

    #[test]
    fn normalize_url_sorts_params() {
        let url = "https://api.x.com/2/tweets?ids=456,123&tweet.fields=text,author_id";
        let normalized = normalize_url(url);
        assert_eq!(
            normalized,
            "https://api.x.com/2/tweets?ids=123,456&tweet.fields=text,author_id"
        );
    }

    #[test]
    fn normalize_url_sorts_query_keys() {
        let url = "https://api.x.com/2/tweets?tweet.fields=text&ids=123";
        let normalized = normalize_url(url);
        assert_eq!(
            normalized,
            "https://api.x.com/2/tweets?ids=123&tweet.fields=text"
        );
    }

    #[test]
    fn normalize_url_no_query() {
        let url = "https://api.x.com/2/users/me";
        assert_eq!(normalize_url(url), url);
    }

    #[test]
    fn should_skip_oauth_and_pagination() {
        assert!(should_skip_cache("https://api.x.com/2/oauth2/token"));
        assert!(should_skip_cache(
            "https://api.x.com/2/users/123/bookmarks?pagination_token=abc"
        ));
        assert!(should_skip_cache(
            "https://api.x.com/2/tweets/search/recent?query=test&next_token=abc123"
        ));
        assert!(!should_skip_cache("https://api.x.com/2/users/me"));
    }

    #[test]
    fn ttl_defaults() {
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/me"),
            3600
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/by/username/jack"),
            3600
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/tweets/search/recent?query=test"),
            900
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/tweets/123"),
            900
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/123/bookmarks"),
            900
        );
        assert_eq!(default_ttl_for_endpoint("https://api.x.com/2/unknown"), 900);
    }

    #[test]
    fn hex_encode() {
        assert_eq!(hex::encode([0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(hex::encode([]), "");
    }

    #[test]
    fn normalize_endpoint_search_recent() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/tweets/search/recent?query=test"),
            "/2/tweets/search/recent"
        );
    }

    #[test]
    fn normalize_endpoint_users_me() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/me"),
            "/2/users/me"
        );
    }

    #[test]
    fn normalize_endpoint_tweet_by_id() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/tweets/1234567890"),
            "/2/tweets/:id"
        );
    }

    #[test]
    fn normalize_endpoint_user_bookmarks() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/123/bookmarks"),
            "/2/users/:id/bookmarks"
        );
    }

    #[test]
    fn normalize_endpoint_username_lookup() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/by/username/jack"),
            "/2/users/by/username/:username"
        );
    }

    #[test]
    fn normalize_endpoint_usage_tweets() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/usage/tweets?usage.fields=daily_project_usage"),
            "/2/usage/tweets"
        );
    }

    #[test]
    fn normalize_endpoint_full_url_strips_query() {
        let url = "https://api.x.com/2/tweets/search/recent?query=rust&max_results=100";
        assert_eq!(normalize_endpoint(url), "/2/tweets/search/recent");
    }

    #[test]
    fn oauth1_cache_key_differs_from_oauth2() {
        let url = "https://api.x.com/2/users/me";
        let oauth1_ctx = RequestContext {
            auth_type: &AuthType::OAuth1,
            username: Some("alice"),
        };
        let oauth2_ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: Some("alice"),
        };
        let key_oauth1 = compute_cache_key("GET", url, &oauth1_ctx);
        let key_oauth2 = compute_cache_key("GET", url, &oauth2_ctx);
        assert_ne!(
            key_oauth1, key_oauth2,
            "OAuth1 and OAuth2 should produce different cache keys for the same URL"
        );
    }

    #[test]
    fn api_response_debug_redacts_body() {
        let response = ApiResponse {
            status: reqwest::StatusCode::OK,
            body: "sensitive data here".to_string(),
            headers: reqwest::header::HeaderMap::new(),
            cache_hit: true,
            json: None,
        };
        let debug = format!("{:?}", response);
        assert!(!debug.contains("sensitive data here"));
        assert!(debug.contains("body_len"));
    }
}
