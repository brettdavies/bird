//! Bird's seam over the xdk-rs client.
//!
//! bird renders every request itself (a full URL, or a path template for
//! `bird raw`) and sends it through one generic entry point. The trait exists
//! so tests substitute `MockXurlClient` at the bird/xdk boundary while
//! production requests flow through [`EmbeddedClient`].

use std::future::Future;
use std::path::Path;

use serde_json::Value;
use xdk::api::{Client, RequestOptions};
use xdk::auth::Auth;
use xdk::config::Config;

use crate::shutdown::{Cancelled, Stop};

#[cfg(test)]
pub mod mock;

/// The request entry point bird needs from xdk-rs. Production impl is
/// [`EmbeddedClient`]; tests substitute `MockXurlClient`.
pub trait XurlClient {
    /// Sends one request and returns the response body as X sent it. Raw
    /// requests skip xdk's typed-response vocabulary normalization, so the
    /// keys match the field parameters bird put on the request.
    fn send_request(&mut self, options: &RequestOptions) -> xdk::Result<Value>;

    /// Why the client cannot send anything, or `None` when it can.
    fn unavailable_reason(&self) -> Option<&str> {
        None
    }
}

/// Opens the xdk-rs config and token store for one invocation from what the
/// runner captured: the environment snapshot, the store at `token_store`, and
/// `app` when `--app` names one. A named store reads nothing from the process,
/// so a test that passes an empty environment and a temporary store stays
/// isolated; with none, xdk opens its default store the way `xr` does.
pub fn open_store(
    env: &xdk::config::EnvOverrides,
    token_store: Option<&Path>,
    app: Option<&str>,
) -> (Config, Auth) {
    let mut cfg = Config::from_overrides(env);
    if let Some(app) = app {
        cfg.app_name = app.to_string();
    }
    let auth = match token_store {
        Some(path) => Auth::new_with_store_path_and_overrides(&cfg, path, env),
        None => Auth::new(&cfg),
    };
    (cfg, auth)
}

/// Where the OAuth2 pending state for `token_store` lives: beside a named
/// store, or beside xdk's default store.
pub fn pending_path(token_store: Option<&Path>) -> xdk::Result<std::path::PathBuf> {
    match token_store {
        Some(path) => Ok(xdk::auth::pending::pending_path_for_store(path)),
        None => xdk::auth::pending::default_pending_path(),
    }
}

/// An `xdk::api::Client` plus the current-thread runtime that drives its
/// async calls. bird is synchronous, so each call blocks on the runtime; the
/// client's connection pool lives on that runtime and is reused across calls.
pub struct EmbeddedClient {
    client: Client,
    runtime: tokio::runtime::Runtime,
}

impl EmbeddedClient {
    /// Builds a client over `auth`'s token store, with every non-streaming
    /// request bounded by `timeout_secs`. A store with no usable credential
    /// still builds; the first request that needs one fails with xdk's
    /// auth-required error.
    pub fn new(cfg: &Config, auth: Auth, timeout_secs: u64) -> xdk::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| xdk::Error::Internal(format!("cannot start the async runtime: {e}")))?;
        let client = {
            let _context = runtime.enter();
            Client::with_timeout(cfg, auth, timeout_secs)?
        };
        Ok(Self { client, runtime })
    }

    /// The underlying client, for the calls bird makes outside
    /// [`XurlClient::send_request`] (sign-in).
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Runs `future` to completion on this client's runtime, or drops it when
    /// the process receives SIGTERM or SIGINT first.
    pub fn block_on<F: Future>(&self, future: F) -> Result<F::Output, Cancelled> {
        Stop::process().block_on(&self.runtime, future)
    }
}

impl XurlClient for EmbeddedClient {
    fn send_request(&mut self, options: &RequestOptions) -> xdk::Result<Value> {
        self.block_on(self.client.send_request(options))
            .unwrap_or_else(|cancelled| Err(cancelled.into()))
    }
}

/// `BirdClient` holds its `XurlClient` in a `std::sync::Mutex` and must stay
/// shareable across threads, so the production impl must be `Send + Sync`.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EmbeddedClient>();
};

/// `XurlClient` used when [`EmbeddedClient::new`] fails at startup.
/// Returns the construction error on every call, so commands that never send
/// a request keep working and the first one that does reports why it cannot.
pub struct ConstructionStub {
    error: String,
}

impl ConstructionStub {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

impl XurlClient for ConstructionStub {
    fn send_request(&mut self, _options: &RequestOptions) -> xdk::Result<Value> {
        Err(xdk::Error::Internal(format!(
            "X API client unavailable: {}",
            self.error
        )))
    }

    fn unavailable_reason(&self) -> Option<&str> {
        Some(&self.error)
    }
}
