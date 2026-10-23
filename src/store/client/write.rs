//! Write path: POST/PUT/DELETE sent through the embedded client. Sending one
//! removes the account's stored responses (see `BirdClient::after_send`).

use super::{ApiResponse, BirdClient, RequestContext};

impl BirdClient {
    /// POST/PUT/DELETE through the embedded client, priced into the ledger.
    pub fn request(
        &mut self,
        method: &str,
        url: &str,
        ctx: &RequestContext<'_>,
        body: Option<&str>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let json = self.xurl_send_raw_url(method, url, body.unwrap_or(""), ctx)?;
        let request = crate::cost::Request {
            method,
            url,
            body,
            caller_id: None,
        };
        self.log_api_call(request, Some(&json), false, ctx.username);
        Ok(ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: Some(json),
        })
    }
}
