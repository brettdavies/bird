//! Write path: POST/PUT/DELETE pass-through. Stays thin until write-side
//! cache invalidation (deleted-tweet, blocked-user propagation) lands as a
//! future tenant.

use super::{ApiResponse, BirdClient, RequestContext};

impl BirdClient {
    /// POST/PUT/DELETE — pass-through via xurl, no entity store interaction.
    pub fn request(
        &mut self,
        method: &str,
        url: &str,
        ctx: &RequestContext<'_>,
        body: Option<&str>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let json = self.xurl_send_raw_url(method, url, body.unwrap_or(""), ctx)?;
        self.log_api_call(url, method, Some(&json), false, ctx.username);
        Ok(ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: Some(json),
        })
    }
}
