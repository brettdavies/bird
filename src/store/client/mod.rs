//! `BirdClient`: bird's requests through the embedded xdk-rs client, answered
//! from and observed into the file store.

mod embedded;
mod get;
mod write;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod archive_session;

pub use embedded::WriteOutcome;

use crate::cli::auth_scheme::AuthType;
use crate::cost;
use crate::store::endpoint::normalize_endpoint;
use crate::store::file_store::{FileStore, StoreAccount};
use crate::store::ledger::{self, Record, RequestRecord};
use crate::store::paths::StorePaths;
use crate::store::responses::Account;
use crate::store::status::StoreStatus;
use crate::xurl_client::XurlClient;

use std::borrow::Cow;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Request context for usage logging and auth tracking.
pub struct RequestContext<'a> {
    pub auth_type: &'a AuthType,
    pub username: Option<&'a str>,
}

impl RequestContext<'_> {
    /// Whether the request goes out with no user: app-only or unauthenticated.
    fn app_only(&self) -> bool {
        matches!(self.auth_type, AuthType::Bearer | AuthType::None)
    }
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

/// Where a client's store lives and whose requests it holds.
pub struct StoreSetup {
    /// The file store's locations.
    pub paths: StorePaths,
    /// The app and default user requests go out under.
    pub account: StoreAccount,
}

impl StoreSetup {
    /// Every location under `root`, with an empty account.
    pub fn under(root: &Path) -> Self {
        Self {
            paths: StorePaths::under(root),
            account: StoreAccount::default(),
        }
    }
}

/// Response from BirdClient (covers both store hits and fresh API responses).
///
/// `cached_body` holds bytes a caller supplied verbatim; every other
/// constructor leaves it `None`, and `body()` derives the string lazily from
/// `json` on the rare caller (error formatting, fallback) that needs `&str`.
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

    /// Test-only constructor for a response whose bytes don't parse as JSON.
    /// Used by `raw.rs::tests` to exercise the `into_body()` fallback.
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
    pub fn into_body(self) -> String {
        self.body().into_owned()
    }
}

impl fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let body_len: Option<usize> = self.cached_body.as_ref().map(|s| s.len());
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("cache_hit", &self.cache_hit)
            .field("body_len", &body_len)
            .field("json_present", &self.json.is_some())
            .finish()
    }
}

// Compile-time guard that BirdClient: Send + Sync.
const _: fn() = || {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<BirdClient>();
};

/// bird's client: the embedded X API client plus the file store.
pub struct BirdClient {
    /// Embedded X API client guarded by a Mutex so `&self` methods can call
    /// its `&mut self` methods. Read through `BirdClient::with_xurl`.
    pub(super) xurl: Mutex<Box<dyn XurlClient + Send>>,
    pub(super) store: FileStore,
    pub(super) account: StoreAccount,
    pub(super) cache_opts: CacheOpts,
    /// Username threaded into `RequestOptions.username` for multi-user token
    /// selection.
    pub(super) username: Option<String>,
    /// The authenticated user's id, once a `/2/users/me` response names it.
    pub(super) caller_id: Option<String>,
    /// Suppress informational stderr output.
    pub quiet: bool,
    #[cfg(test)]
    pub(super) _scratch: Option<tempfile::TempDir>,
}

impl BirdClient {
    /// Create a BirdClient over the store in `setup`. Store failures are
    /// warned once through `stderr` and never stop a request.
    pub fn new(
        xurl: Box<dyn XurlClient + Send>,
        setup: StoreSetup,
        cache_opts: CacheOpts,
        username: Option<String>,
        quiet: bool,
        stderr: Arc<Mutex<dyn Write + Send>>,
    ) -> Self {
        let status = StoreStatus::new(quiet, stderr);
        let store = FileStore::new(&setup.paths, status);
        if !cache_opts.no_store {
            store.prune(chrono::Utc::now());
        }
        Self {
            xurl: Mutex::new(xurl),
            store,
            account: setup.account,
            cache_opts,
            username,
            caller_id: None,
            quiet,
            #[cfg(test)]
            _scratch: None,
        }
    }

    /// Test-only constructor: the given transport over a store in a fresh
    /// temporary directory, with an account `default`/`alice`. Diagnostics
    /// go to `io::sink()`.
    #[cfg(test)]
    pub(crate) fn new_test(xurl: Box<dyn XurlClient + Send>) -> Self {
        let scratch = tempfile::tempdir().expect("test: tempdir");
        let mut setup = StoreSetup::under(scratch.path());
        setup.account = StoreAccount {
            app: "default".into(),
            default_user: Some("alice".into()),
        };
        let mut client = Self::new(
            xurl,
            setup,
            CacheOpts::default(),
            None,
            true,
            Arc::new(Mutex::new(std::io::sink())),
        );
        client._scratch = Some(scratch);
        client
    }

    /// Whether the store is disabled (`--no-cache`).
    pub fn db_disabled(&self) -> bool {
        self.cache_opts.no_store
    }

    /// The run's file store.
    pub fn store(&self) -> &FileStore {
        &self.store
    }

    /// The account a request goes out under: the requested username, else
    /// the app's default user, and no user for an app-only request.
    pub(super) fn account_for(&self, ctx: &RequestContext<'_>) -> Option<Account> {
        let requested = self.username.as_deref().or(ctx.username);
        self.account.for_request(ctx.app_only(), requested)
    }

    /// After a sent non-GET request, the account's stored responses go, even
    /// under `--no-cache`, so no later run answers from the state before it.
    pub(super) fn after_send(&self, method: &str, ctx: &RequestContext<'_>) {
        if !method.eq_ignore_ascii_case("GET") {
            self.store.invalidate(self.account_for(ctx).as_ref());
        }
    }

    /// Price an API call and append it to the ledger, returning the
    /// fresh-request estimate. A `/2/users/me` response teaches the client
    /// the caller's id, which later requests need for owned-read pricing.
    /// Under `--no-cache` nothing is appended.
    pub fn log_api_call(
        &mut self,
        request: cost::Request<'_>,
        json: Option<&serde_json::Value>,
        cache_hit: bool,
        username: Option<&str>,
    ) -> cost::CostEstimate {
        let endpoint = normalize_endpoint(request.url);
        let null = serde_json::Value::Null;
        let json = json.unwrap_or(&null);
        if endpoint == "/2/users/me"
            && let Some(id) = json
                .get("data")
                .and_then(|d| d.get("id"))
                .and_then(|id| id.as_str())
        {
            self.caller_id = Some(id.to_string());
        }
        let priced = cost::Request {
            caller_id: request.caller_id.or(self.caller_id.as_deref()),
            ..request
        };
        let estimate = cost::estimate_raw_cost(&priced, json);
        if self.cache_opts.no_store {
            return estimate;
        }
        let (object_type, object_count) = if estimate.write.is_some() {
            ("write", 1)
        } else if estimate.users_read > 0 && estimate.posts_read == 0 {
            ("user", estimate.users_read)
        } else if estimate.posts_read > 0 {
            ("post", estimate.posts_read + estimate.users_read)
        } else {
            ("none", 0)
        };
        let now = chrono::Utc::now();
        let record = Record::Request(RequestRecord {
            v: ledger::FORMAT,
            ts: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            method: request.method.to_uppercase(),
            endpoint,
            object_type: object_type.to_string(),
            object_count: i64::from(object_count),
            estimated_cost: estimate.estimated_usd,
            cache_hit,
            app: self.account.app.clone(),
            username: self
                .username
                .as_deref()
                .or(username)
                .unwrap_or_default()
                .to_string(),
        });
        self.store.record(&record, now);
        estimate
    }
}
