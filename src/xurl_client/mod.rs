//! Bird's seam over the xdk-rs client.
//!
//! bird renders every request itself (a full URL, or a path template for
//! `bird raw`) and sends it through one generic entry point. The trait exists
//! so tests substitute `MockXurlClient` at the bird/xdk boundary while
//! production requests flow through [`EmbeddedClient`].

use std::future::Future;

use serde_json::Value;
use xdk::api::{Client, RequestOptions};
use xdk::auth::Auth;
use xdk::config::Config;

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

/// An `xdk::api::Client` plus the current-thread runtime that drives its
/// async calls. bird is synchronous, so each call blocks on the runtime; the
/// client's connection pool lives on that runtime and is reused across calls.
pub struct EmbeddedClient {
    client: Client,
    runtime: tokio::runtime::Runtime,
}

impl EmbeddedClient {
    /// Builds a client over the `~/.xurl` token store for `cfg`'s app, with
    /// every non-streaming request bounded by `timeout_secs`. A store with no
    /// usable credential still builds; the first request that needs one fails
    /// with xdk's auth-required error.
    pub fn from_config(cfg: &Config, timeout_secs: u64) -> xdk::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| xdk::Error::Internal(format!("cannot start the async runtime: {e}")))?;
        let client = {
            let _context = runtime.enter();
            Client::with_timeout(cfg, Auth::new(cfg), timeout_secs)?
        };
        Ok(Self { client, runtime })
    }

    /// The underlying client, for the calls bird makes outside
    /// [`XurlClient::send_request`] (sign-in).
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Runs `future` to completion on this client's runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

impl XurlClient for EmbeddedClient {
    fn send_request(&mut self, options: &RequestOptions) -> xdk::Result<Value> {
        self.runtime.block_on(self.client.send_request(options))
    }
}

/// `BirdClient` holds its `XurlClient` in a `std::sync::Mutex` and must stay
/// shareable across threads, so the production impl must be `Send + Sync`.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EmbeddedClient>();
};

/// `XurlClient` used when [`EmbeddedClient::from_config`] fails at startup.
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
