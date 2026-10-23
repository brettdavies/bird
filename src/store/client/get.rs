//! Read path: a GET answered from its same-day response file, or sent and
//! observed into the store.

use super::{ApiResponse, BirdClient, RequestContext};
use crate::cost;

impl BirdClient {
    /// GET `url`. Unless `--refresh`, a same-day response stored for the
    /// same request and account answers it; under `--cache-only` nothing is
    /// sent and a miss is bird's `cache-miss`. A sent response is stored and
    /// archived, and every sent or answered request is priced into the
    /// ledger.
    pub fn get(
        &mut self,
        url: &str,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        if self.cache_opts.no_store {
            let response = self.xurl_get(url, ctx)?;
            self.log_api_call(
                cost::Request::get(url),
                response.json.as_ref(),
                false,
                ctx.username,
            );
            return Ok(response);
        }

        let now = chrono::Utc::now();
        let account = self.account_for(ctx);
        let read_store = !self.cache_opts.refresh || self.cache_opts.cache_only;
        if read_store && let Some(hit) = self.store.answer("GET", url, account.as_ref(), now) {
            let response = ApiResponse {
                status: hit.status,
                cached_body: None,
                cache_hit: true,
                json: Some(hit.body),
            };
            self.log_api_call(
                cost::Request::get(url),
                response.json.as_ref(),
                true,
                ctx.username,
            );
            return Ok(response);
        }
        if self.cache_opts.cache_only {
            return Err(Box::new(crate::error::CacheMiss));
        }

        let response = self.xurl_get(url, ctx)?;
        if let Some(ref body) = response.json {
            self.store
                .observe("GET", url, account.as_ref(), response.status, body, now);
        }
        self.log_api_call(
            cost::Request::get(url),
            response.json.as_ref(),
            false,
            ctx.username,
        );
        Ok(response)
    }

    /// GET via the embedded client. xdk-rs returns an error for a non-2xx
    /// status, so a response here is a success.
    fn xurl_get(
        &self,
        url: &str,
        ctx: &RequestContext<'_>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let json = self.xurl_send_raw_url("GET", url, "", ctx)?;
        Ok(ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: Some(json),
        })
    }
}
