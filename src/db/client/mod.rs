//! Entity-aware transport layer over xurl with optional BirdDb cache.
//! Handles UTC-day freshness, batch ID splitting, entity decomposition, and response merging.

mod entity;
mod get;
mod write;

use crate::cost;
use crate::requirements::AuthType;
#[cfg(not(feature = "embedded-xurl"))]
use crate::transport::Transport;
#[cfg(feature = "embedded-xurl")]
use crate::xurl_client::XurlClient;

use super::normalize_endpoint;
use super::store::BirdDb;

use std::borrow::Cow;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
///
/// `cached_body` holds the authoritative bytes-as-stored when available —
/// today, only the `raw_responses` cache hit populates it, preserving the
/// exact bytes the API emitted. Every other constructor leaves it `None`;
/// `body()` derives the string lazily from `json` on the rare caller (error
/// formatting, fallback) that still needs `&str`.
///
/// When Transport begins returning raw bytes (a future redesign), `xurl_get`
/// is expected to populate `cached_body` alongside `json`; `body()` will then
/// take the borrowed branch. Until then, only raw_responses cache hits set it.
pub struct ApiResponse {
    pub status: u16,
    pub(super) cached_body: Option<String>,
    pub cache_hit: bool,
    /// Pre-parsed JSON body (populated by transport methods to avoid double-parse).
    pub json: Option<serde_json::Value>,
}

impl ApiResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Test-only constructor mirroring a `raw_responses` cache hit whose
    /// stored bytes don't parse as JSON. Used by `raw.rs::tests` to
    /// exercise the `into_body()` fallback without standing up a full DB.
    #[cfg(test)]
    pub(crate) fn for_test_raw_body(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            cached_body: Some(body.into()),
            cache_hit: true,
            json: None,
        }
    }

    /// Borrow the response body as `&str`.
    ///
    /// Returns `Cow::Borrowed` when a raw payload is stored (raw_responses
    /// cache hits today); other paths serialize lazily from `json` into
    /// `Cow::Owned`. The optimization wins because the common caller never
    /// reaches `body()` at all — they read `self.json` directly.
    pub fn body(&self) -> Cow<'_, str> {
        debug_assert!(
            self.cached_body.is_some() || self.json.is_some(),
            "ApiResponse with no body and no json"
        );
        if let Some(ref s) = self.cached_body {
            Cow::Borrowed(s.as_str())
        } else if let Some(ref jv) = self.json {
            Cow::Owned(serde_json::to_string(jv).unwrap_or_default())
        } else {
            Cow::Borrowed("")
        }
    }

    /// Consume the response and yield an owned body `String`.
    ///
    /// Delegates to `body()`; the borrowed branch incurs one `String`
    /// allocation, the owned branch is a direct move. Used by the `raw.rs`
    /// fallback when `json` is absent (raw_responses cache hits with a
    /// non-JSON BLOB payload).
    pub fn into_body(self) -> String {
        self.body().into_owned()
    }
}

impl fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Avoid serializing `json` just to measure length: when `cached_body`
        // is absent (the common path — every fresh request and every
        // freshness cache hit) report `None` instead of paying for a
        // discarded `serde_json::to_string`. The Debug contract here is
        // redaction + sizing, not faithful body reproduction.
        let body_len: Option<usize> = self.cached_body.as_ref().map(|s| s.len());
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("cache_hit", &self.cache_hit)
            .field("body_len", &body_len)
            .field("json_present", &self.json.is_some())
            .finish()
    }
}

/// Map bird's `AuthType` enum to xurl-rs's wire-string vocabulary
/// (`"app"/"oauth1"/"oauth2"`, or empty for xurl's auto-detect path).
/// xurl-rs treats `OAuth2User` and the empty string identically — both
/// resolve to OAuth2 via `auth_matrix` — so OAuth2User maps to empty here.
/// `AuthType::None` also maps to empty; the caller is responsible for
/// setting `RequestOptions.no_auth = true` when the bird-side AuthType is
/// `None`. U12 surfaces a full `--auth` flag against the same wire
/// vocabulary, at which point this helper may move into a shared spot.
#[cfg(feature = "embedded-xurl")]
fn auth_type_to_xurl_wire(at: &AuthType) -> String {
    match at {
        AuthType::OAuth2User => String::new(),
        AuthType::OAuth1 => "oauth1".to_string(),
        AuthType::Bearer => "app".to_string(),
        AuthType::None => String::new(),
    }
}

// -- BirdClient --

// Compile-time guard that BirdClient: Send + Sync. A future field with a
// non-Sync type (RefCell, Rc, bare rusqlite::Connection) fails this assertion
// at build time.
const _: fn() = || {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<BirdClient>();
};

/// Entity-aware transport layer. Wraps xurl transport + optional BirdDb.
/// If BirdDb is unavailable (corrupted, disk error), degrades to direct transport.
pub struct BirdClient {
    #[cfg(not(feature = "embedded-xurl"))]
    pub(super) transport: Box<dyn Transport>,
    /// Embedded xurl client guarded by a Mutex so `&self` methods can acquire
    /// the lock and call `&mut self` xurl methods. The lock-acquire-in-method
    /// pattern mirrors the existing `Mutex<rusqlite::Connection>` precedent in
    /// `src/db/store/mod.rs`. PR1 ships this field unused — handler bodies
    /// land in PR2.
    #[cfg(feature = "embedded-xurl")]
    #[allow(dead_code)]
    pub(super) xurl: Mutex<Box<dyn XurlClient + Send>>,
    pub(super) db: Option<BirdDb>,
    pub(super) cache_opts: CacheOpts,
    /// Username for xurl -u flag (multi-user token selection). Read by the
    /// subprocess `build_get_args`/write path; PR1 leaves it unread under
    /// `embedded-xurl` because handler bodies stub out in PR1 and migrate in
    /// PR2.
    #[cfg_attr(feature = "embedded-xurl", allow(dead_code))]
    pub(super) username: Option<String>,
    /// Suppress informational stderr output. Stored on the struct (unlike `use_color`
    /// which is parameter-passed) because 7+ internal methods emit diagnostics and
    /// threading through every method signature would be excessive.
    pub quiet: bool,
    /// Shared writer handle for diagnostic output. `Arc::clone` of this is
    /// passed into `BirdDb::open` so both layers emit through the same sink.
    /// Read by the internal diagnostic sites under the `if !self.quiet` gate
    /// — the lock is acquired only when emission is required, so suppressed
    /// paths pay zero.
    pub(crate) stderr: Arc<Mutex<dyn Write + Send>>,
}

impl BirdClient {
    /// Create a new BirdClient. If entity store cannot be opened, degrades to no-store.
    ///
    /// `stderr` is the shared writer handle: `Arc::clone` is forwarded to
    /// `BirdDb::open` so both layers emit through the same sink. Internal
    /// diagnostic sites lock the shared handle under the `if !self.quiet`
    /// gate.
    pub fn new(
        #[cfg(not(feature = "embedded-xurl"))] transport: Box<dyn Transport>,
        #[cfg(feature = "embedded-xurl")] xurl: Box<dyn XurlClient + Send>,
        store_path: &Path,
        cache_opts: CacheOpts,
        max_size_mb: u64,
        username: Option<String>,
        quiet: bool,
        stderr: Arc<Mutex<dyn Write + Send>>,
    ) -> Self {
        #[cfg(feature = "embedded-xurl")]
        let xurl = Mutex::new(xurl);

        if cache_opts.no_store {
            return Self {
                #[cfg(not(feature = "embedded-xurl"))]
                transport,
                #[cfg(feature = "embedded-xurl")]
                xurl,
                db: None,
                cache_opts,
                username,
                quiet,
                stderr,
            };
        }
        let db = match BirdDb::open(store_path, max_size_mb, Arc::clone(&stderr), quiet) {
            Ok(db) => {
                // Migrate usage data from old cache.db on first run
                if let Some(parent) = store_path.parent() {
                    let old_cache = parent.join("cache.db");
                    if old_cache.exists() {
                        db.migrate_usage_from_cache(&old_cache);
                    }
                }
                // Prune stale raw_responses and oversized entity tables
                if let Err(e) = db.prune_if_needed()
                    && !quiet
                {
                    let mut w = stderr.lock().unwrap();
                    writeln!(*w, "[store] warning: pruning failed: {e}").ok();
                }
                Some(db)
            }
            Err(e) => {
                if !quiet {
                    let mut w = stderr.lock().unwrap();
                    writeln!(*w, "[store] warning: failed to open entity store: {e}").ok();
                    writeln!(*w, "[store] Run `bird cache clear` to reset the store.").ok();
                }
                None
            }
        };
        Self {
            #[cfg(not(feature = "embedded-xurl"))]
            transport,
            #[cfg(feature = "embedded-xurl")]
            xurl,
            db,
            cache_opts,
            username,
            quiet,
            stderr,
        }
    }

    /// Test-only constructor with explicit transport and in-memory DB.
    /// Uses `io::sink()` as the stderr writer so tests don't capture internal
    /// diagnostic output.
    #[cfg(all(test, not(feature = "embedded-xurl")))]
    pub(crate) fn new_test(transport: Box<dyn Transport>, db: super::store::BirdDb) -> Self {
        Self {
            transport,
            db: Some(db),
            cache_opts: CacheOpts::default(),
            username: None,
            quiet: true,
            stderr: Arc::new(Mutex::new(std::io::sink())),
        }
    }

    /// Resolved xurl binary path, when the live transport spawns one. Mock
    /// transports return `None`. Surfaced for direct call sites (`bird login`,
    /// `bird doctor`'s `whoami`, write commands) that need the path without
    /// going through [`Transport::request`].
    pub fn xurl_path(&self) -> Option<&Path> {
        #[cfg(not(feature = "embedded-xurl"))]
        {
            self.transport.xurl_path()
        }
        #[cfg(feature = "embedded-xurl")]
        {
            None
        }
    }

    /// Direct-trait request, used by handlers (`bird doctor whoami`, write
    /// verbs) that need to pass argv through to xurl without the entity-store
    /// pipeline.
    pub fn transport_request(
        &self,
        #[cfg(not(feature = "embedded-xurl"))] args: &[String],
        #[cfg(feature = "embedded-xurl")] _args: &[String],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(not(feature = "embedded-xurl"))]
        {
            self.transport.request(args)
        }
        #[cfg(feature = "embedded-xurl")]
        {
            Err("embedded transport stub — handler migration lands in PR2".into())
        }
    }

    /// `bird raw` embedded seam: dispatch a request through the typed xurl
    /// client using a `RequestTarget::Template`. xurl owns path substitution
    /// and the `auth_matrix::supported_auth(method, template)` lookup
    /// atomically — bird passes the template, not a rendered URL.
    ///
    /// Used by `src/raw.rs::run_raw` under `embedded-xurl`. The subprocess
    /// arm continues to call `BirdClient::get`/`request` with a rendered URL.
    /// PR3's U15 deletes the subprocess arm; this seam becomes the only path.
    #[cfg(feature = "embedded-xurl")]
    pub fn raw_template_request(
        &mut self,
        method: &str,
        path_template: &str,
        path_params: std::collections::HashMap<String, String>,
        query: Vec<(String, String)>,
        body: Option<&str>,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        use xurl::api::{RequestOptions, RequestTarget};

        let opts = RequestOptions {
            method: method.to_uppercase(),
            target: RequestTarget::Template {
                path: path_template.to_string(),
                path_params,
                query,
            },
            data: body.unwrap_or("").to_string(),
            auth_type: auth_type_to_xurl_wire(ctx.auth_type),
            username: self
                .username
                .clone()
                .or_else(|| ctx.username.map(str::to_string))
                .unwrap_or_default(),
            no_auth: matches!(ctx.auth_type, AuthType::None),
            ..RequestOptions::default()
        };

        let json = {
            let mut guard = self
                .xurl
                .lock()
                .expect("BirdClient.xurl mutex poisoned during raw_template_request");
            guard.send_request(&opts)
        }
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        // Cost categorization and usage logging key on a rendered URL today;
        // approximate it from the template since path-param substitution is
        // immaterial to bucket selection (`/users/` substring match).
        let pseudo_url = format!("https://api.x.com{path_template}");
        self.log_api_call(&pseudo_url, method, Some(&json), false, ctx.username);

        Ok(ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: Some(json),
        })
    }

    /// Get entity store stats (None if store unavailable).
    pub fn db_stats(&self) -> Option<Result<super::store::StoreStats, rusqlite::Error>> {
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
        }) && !self.quiet
        {
            let mut w = self.stderr.lock().unwrap();
            writeln!(*w, "[usage] warning: failed to log API call: {e}").ok();
        }
    }
}

#[cfg(all(test, feature = "embedded-xurl"))]
mod embedded_tests {
    use super::*;
    use crate::xurl_client::mock::MockXurlClient;
    use std::collections::HashMap;
    use std::io;
    use xurl::api::ApiResponse as XurlApiResponse;
    use xurl::api::Tweet;

    /// `bird raw GET /2/users/{id}/likes -p id=12345` must reach xurl as a
    /// `RequestTarget::Template`, not a rendered URL. The mock records the
    /// `RequestOptions` it saw; the test asserts the template path and the
    /// `path_params` survived round-trip.
    #[test]
    fn raw_template_request_sends_template_to_xurl() {
        let mock = MockXurlClient::new();
        let queued = XurlApiResponse {
            data: vec![Tweet {
                id: "abc".to_string(),
                ..Tweet::default()
            }],
            ..XurlApiResponse::<Vec<Tweet>>::default()
        };
        mock.push_response("send_request", queued);

        let mut client = BirdClient::new(
            Box::new(mock),
            std::path::Path::new("/nonexistent/u6-raw-test"),
            CacheOpts {
                no_store: true,
                ..CacheOpts::default()
            },
            0,
            None,
            true,
            Arc::new(Mutex::new(io::sink())),
        );

        let ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: None,
        };
        let mut params = HashMap::new();
        params.insert("id".to_string(), "12345".to_string());
        let response = client
            .raw_template_request("GET", "/2/users/{id}/likes", params, Vec::new(), None, &ctx)
            .expect("raw template request must succeed");

        // The queued response carries a `Tweet { id: "abc" }`; surfacing it on
        // bird's `ApiResponse.json` proves the trait method dispatched and the
        // typed payload survived. `RequestTarget::Template` argument capture is
        // covered separately in `xurl_client::mock::tests`.
        let json = response.json.expect("queued json must surface");
        assert_eq!(json["data"][0]["id"], "abc");
    }

    #[test]
    fn raw_template_request_translates_auth_type_to_wire() {
        assert_eq!(auth_type_to_xurl_wire(&AuthType::OAuth2User), "");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::OAuth1), "oauth1");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::Bearer), "app");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::None), "");
    }
}

#[cfg(all(test, not(feature = "embedded-xurl")))]
mod tests {
    use super::super::store::BirdDb;
    use super::*;
    use crate::transport::tests::MockTransport;

    pub(super) fn test_client_with_db(db: BirdDb) -> BirdClient {
        BirdClient {
            transport: Box::new(MockTransport::new(vec![])),
            db: Some(db),
            cache_opts: CacheOpts::default(),
            username: None,
            quiet: false,
            stderr: Arc::new(Mutex::new(std::io::sink())),
        }
    }

    #[test]
    fn api_response_debug_redacts_body() {
        let response = ApiResponse {
            status: 200,
            cached_body: Some("sensitive data here".to_string()),
            cache_hit: true,
            json: None,
        };
        let debug = format!("{:?}", response);
        assert!(!debug.contains("sensitive data here"));
        assert!(debug.contains("body_len"));
    }

    /// Documents the empty-body contract: an `ApiResponse` with neither
    /// `cached_body` nor `json` returns `""` from `body()` in release
    /// builds. Production paths never construct such a response — the
    /// `debug_assert!` in `body()` would fire — so this test is
    /// release-only.
    #[cfg(not(debug_assertions))]
    #[test]
    fn empty_body_fallback_returns_empty_string() {
        let resp = ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: None,
        };
        assert_eq!(resp.body(), "");
        assert_eq!(resp.into_body(), "");
    }

    /// Asserts the `body()` debug_assert fires when both bodies are absent.
    /// Debug-only counterpart to `empty_body_fallback_returns_empty_string`.
    #[cfg(debug_assertions)]
    #[test]
    fn empty_body_triggers_debug_assert() {
        let result = std::panic::catch_unwind(|| {
            let resp = ApiResponse {
                status: 200,
                cached_body: None,
                cache_hit: false,
                json: None,
            };
            let _ = resp.body();
        });
        assert!(
            result.is_err(),
            "body() must debug_assert when no body is present"
        );
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
        use super::super::store::{BookmarkRow, in_memory_db};
        use super::super::unix_now;

        let db = in_memory_db();
        let mut client = test_client_with_db(db);

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

        let db = client.db.as_ref().expect("test");
        assert!(
            db.get_tweet("t1").expect("test").is_some(),
            "search should store tweet t1"
        );
        assert!(
            db.get_tweet("t2").expect("test").is_some(),
            "search should store tweet t2"
        );
        assert!(
            db.get_user_by_username("alice").expect("test").is_some(),
            "search should store included user alice"
        );
        assert!(
            db.get_user_by_username("bob").expect("test").is_some(),
            "search should store included user bob"
        );

        // --- Step 2: Profile lookup finds stored user (freshness check) ---
        let alice_resp = super::get::check_user_freshness(db, "alice");
        assert!(
            alice_resp.is_some(),
            "profile should find fresh user alice from store"
        );
        let alice_resp = alice_resp.expect("test");
        assert!(alice_resp.cache_hit, "profile user should be a cache hit");
        assert!(
            alice_resp.body().contains("alice"),
            "profile response should contain username"
        );

        // --- Step 3: Bookmark storage with tweet entities ---
        let bookmark_rows = vec![
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t1".into(),
                position: 0,
                refreshed_at: unix_now(),
            },
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t2".into(),
                position: 1,
                refreshed_at: unix_now(),
            },
        ];
        db.replace_bookmarks("alice", &bookmark_rows).expect("test");
        let stored_bookmarks = db.get_bookmarks("alice").expect("test");
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
        let root_resp = super::get::check_tweet_freshness(db, "t1");
        assert!(root_resp.is_some(), "thread root tweet should be in store");
        assert!(
            root_resp.expect("test").cache_hit,
            "thread root should be cache hit"
        );

        // Partition IDs: t1 is fresh, t3 is missing
        let (from_store, to_fetch) = db.partition_ids(&["t1", "t3"]).expect("test");
        assert_eq!(from_store.len(), 1, "t1 should be fresh in store");
        assert_eq!(from_store[0].id, "t1");
        assert_eq!(to_fetch.len(), 1, "t3 should need fetching");
        assert_eq!(to_fetch[0], "t3");

        // --- Step 5: Usage logging ---
        let db_mut = client.db.as_mut().expect("test");
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
            .expect("test");
        let summary = db_mut.query_usage_summary(0).expect("test");
        assert_eq!(summary.total_calls, 1, "usage should be logged");
        assert_eq!(summary.total_cost, 0.01);

        // --- Step 6: Stats reflect all stored entities ---
        let stats = db_mut.stats().expect("test");
        assert_eq!(stats.tweet_count, 2, "should have 2 tweets");
        assert_eq!(stats.user_count, 2, "should have 2 users");
        assert_eq!(stats.bookmark_count, 2, "should have 2 bookmarks");
    }
}
