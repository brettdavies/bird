//! `BirdClient`'s embedded-xurl seam: the `bird raw` template-shaped entry
//! point, the lock-acquire helper that typed methods (U8/U9) will share, and
//! the local AuthType → wire-string mapping. PR3's cutover removes the cfg
//! gates; this module survives as the only transport surface.

// Mirrors xurl-rs's own `#[allow(clippy::result_large_err)]` on `XurlError`
// (192 bytes). Closures threaded through `with_xurl` return `XurlResult<…>`
// directly; boxing at every bird seam would pay an allocation per error path
// for no downstream gain.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;

use xurl::api::{RequestOptions, RequestTarget};

use super::{ApiResponse, BirdClient, RequestContext};
use crate::requirements::AuthType;
use crate::xurl_client::XurlClient;

/// Map bird's `AuthType` enum to xurl-rs's wire-string vocabulary
/// (`"app"`/`"oauth1"`/`"oauth2"`, or empty for xurl's auto-detect path).
/// xurl-rs treats `OAuth2User` and the empty string identically — both
/// resolve to OAuth2 via `auth_matrix` — so `OAuth2User` maps to empty here.
/// `AuthType::None` also maps to empty; the caller is responsible for
/// setting `RequestOptions.no_auth = true` when the bird-side `AuthType` is
/// `None`. U12 surfaces a full `--auth` flag against the same wire
/// vocabulary, at which point this helper moves to a shared spot.
pub(super) fn auth_type_to_xurl_wire(at: &AuthType) -> String {
    match at {
        AuthType::OAuth2User => String::new(),
        AuthType::OAuth1 => "oauth1".to_string(),
        AuthType::Bearer => "app".to_string(),
        AuthType::None => String::new(),
    }
}

impl BirdClient {
    /// Acquire the embedded `xurl` mutex and dispatch through it. Centralizes
    /// the lock acquire + poison-panic policy so every typed call site stays
    /// a one-liner. When xurl v3.0.0 swaps to `tokio::sync::Mutex` (per
    /// KTD-1), only this helper changes.
    pub(crate) fn with_xurl<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut dyn XurlClient) -> R,
    {
        let mut guard = self.xurl.lock().expect("BirdClient.xurl mutex poisoned");
        f(&mut **guard)
    }

    /// `bird raw` embedded seam: dispatch a request through the typed xurl
    /// client using a `RequestTarget::Template`. xurl owns path substitution
    /// and the `auth_matrix::supported_auth(method, template)` lookup
    /// atomically — bird passes the template, not a rendered URL.
    ///
    /// Used by `src/raw.rs::run_raw` under `embedded-xurl`. The subprocess
    /// arm continues to call `BirdClient::get`/`request` with a rendered URL.
    /// PR3's U15 deletes the subprocess arm; this seam becomes the only path.
    pub fn raw_template_request(
        &mut self,
        method: &str,
        path_template: &str,
        path_params: HashMap<String, String>,
        query: Vec<(String, String)>,
        body: Option<&str>,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
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

        let json = self
            .with_xurl(|x| x.send_request(&opts))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::client::CacheOpts;
    use crate::xurl_client::mock::MockXurlClient;
    use std::io;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use xurl::api::{ApiResponse as XurlApiResponse, Tweet};

    /// Build a no-store `BirdClient` whose embedded transport is the supplied
    /// mock. Returns the constructed client alongside a shared handle the
    /// caller can use to inspect the recorded call log after dispatching.
    fn client_with_mock(mock: MockXurlClient) -> (BirdClient, MockXurlClient) {
        let handle = mock.clone_handle();
        let client = BirdClient::new(
            Box::new(mock),
            Path::new("/nonexistent/u6-embedded-test"),
            CacheOpts {
                no_store: true,
                ..CacheOpts::default()
            },
            0,
            None,
            true,
            Arc::new(Mutex::new(io::sink())),
        );
        (client, handle)
    }

    fn queue_one_tweet(mock: &MockXurlClient) {
        let queued = XurlApiResponse {
            data: vec![Tweet {
                id: "abc".to_string(),
                ..Tweet::default()
            }],
            ..XurlApiResponse::<Vec<Tweet>>::default()
        };
        mock.push_response("send_request", queued);
    }

    #[test]
    fn raw_template_request_dispatches_typed_payload() {
        let mock = MockXurlClient::new();
        queue_one_tweet(&mock);
        let (mut client, _handle) = client_with_mock(mock);

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
        // typed payload survived. `RequestTarget` argument capture is asserted
        // in the dedicated tests below.
        let json = response.json.expect("queued json must surface");
        assert_eq!(json["data"][0]["id"], "abc");
    }

    #[test]
    fn auth_type_wire_mapping() {
        assert_eq!(auth_type_to_xurl_wire(&AuthType::OAuth2User), "");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::OAuth1), "oauth1");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::Bearer), "app");
        assert_eq!(auth_type_to_xurl_wire(&AuthType::None), "");
    }

    /// `bird raw GET /2/users/me` — no path params, no query. The mock must
    /// see `RequestTarget::Template { path: "/2/users/me", path_params: {},
    /// query: [] }`, never a rendered URL.
    #[test]
    fn no_params_no_query_reaches_xurl_as_template() {
        let mock = MockXurlClient::new();
        queue_one_tweet(&mock);
        let (mut client, handle) = client_with_mock(mock);

        let ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: None,
        };
        client
            .raw_template_request("GET", "/2/users/me", HashMap::new(), Vec::new(), None, &ctx)
            .expect("dispatch");

        let calls = handle.calls();
        assert_eq!(calls.len(), 1);
        let target = &calls[0].args["target"];
        assert_eq!(target["type"], "template");
        assert_eq!(target["path"], "/2/users/me");
        assert_eq!(target["path_params"], serde_json::json!({}));
        assert_eq!(target["query"], serde_json::json!([]));
    }

    /// `bird raw GET /2/users/{id}/bookmarks -p id=12345` — path params must
    /// arrive as a map keyed by the template segment, not a rendered URL.
    #[test]
    fn path_params_reach_xurl_unrendered() {
        let mock = MockXurlClient::new();
        queue_one_tweet(&mock);
        let (mut client, handle) = client_with_mock(mock);

        let ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: None,
        };
        let mut params = HashMap::new();
        params.insert("id".to_string(), "12345".to_string());
        client
            .raw_template_request(
                "GET",
                "/2/users/{id}/bookmarks",
                params,
                Vec::new(),
                None,
                &ctx,
            )
            .expect("dispatch");

        let calls = handle.calls();
        let target = &calls[0].args["target"];
        assert_eq!(target["type"], "template");
        assert_eq!(
            target["path"], "/2/users/{id}/bookmarks",
            "the template must reach xurl unrendered so auth_matrix can key on it",
        );
        assert_eq!(target["path_params"]["id"], "12345");
    }

    /// `bird raw GET /2/tweets/search/recent -q query=hello -q max_results=10`
    /// — query pairs must arrive in declaration order.
    #[test]
    fn query_pairs_preserve_order() {
        let mock = MockXurlClient::new();
        queue_one_tweet(&mock);
        let (mut client, handle) = client_with_mock(mock);

        let ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: None,
        };
        let query = vec![
            ("query".to_string(), "hello".to_string()),
            ("max_results".to_string(), "10".to_string()),
        ];
        client
            .raw_template_request(
                "GET",
                "/2/tweets/search/recent",
                HashMap::new(),
                query,
                None,
                &ctx,
            )
            .expect("dispatch");

        let calls = handle.calls();
        let target = &calls[0].args["target"];
        assert_eq!(target["query"][0], serde_json::json!(["query", "hello"]));
        assert_eq!(target["query"][1], serde_json::json!(["max_results", "10"]),);
    }

    /// `bird raw GET /2/users/{id}/likes -p id=foo;bar` — bird's deleted
    /// `validate_param_value` would have rejected the semicolon. Embedded
    /// now hands the value straight to xurl, whose `InvalidPathParam`
    /// rejects only `/`, `?`, `#`, `%`. The dispatch passes the semicolon
    /// through unchanged; whether the request ultimately succeeds is xurl's
    /// call, not bird's.
    #[test]
    fn embedded_no_longer_rejects_semicolon_in_path_params() {
        let mock = MockXurlClient::new();
        queue_one_tweet(&mock);
        let (mut client, handle) = client_with_mock(mock);

        let ctx = RequestContext {
            auth_type: &AuthType::OAuth2User,
            username: None,
        };
        let mut params = HashMap::new();
        params.insert("id".to_string(), "foo;bar".to_string());
        client
            .raw_template_request("GET", "/2/users/{id}/likes", params, Vec::new(), None, &ctx)
            .expect("dispatch must succeed; bird no longer pre-rejects this value");

        let calls = handle.calls();
        assert_eq!(calls[0].args["target"]["path_params"]["id"], "foo;bar");
    }
}
