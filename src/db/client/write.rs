//! Write path: POST/PUT/DELETE pass-through. Stays thin until write-side
//! cache invalidation (deleted-tweet, blocked-user propagation) lands as a
//! future tenant.

#[cfg(not(feature = "embedded-xurl"))]
use crate::requirements;

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
        #[cfg(not(feature = "embedded-xurl"))]
        {
            let mut args: Vec<String> = vec!["-X".into(), method.to_uppercase()];
            if let Some(flag) = requirements::auth_flag(ctx.auth_type) {
                args.extend_from_slice(&["--auth".into(), flag.into()]);
            }
            if let Some(ref username) = self.username {
                args.extend_from_slice(&["-u".into(), username.clone()]);
            }
            if let Some(b) = body {
                args.extend_from_slice(&["-d".into(), b.into()]);
            }
            args.push(url.into());

            let json_value = self.transport.request(&args)?;
            self.log_api_call(url, method, Some(&json_value), false, ctx.username);
            Ok(ApiResponse {
                status: 200,
                cached_body: None,
                cache_hit: false,
                json: Some(json_value),
            })
        }
        #[cfg(feature = "embedded-xurl")]
        {
            let _ = (method, url, ctx, body);
            Err("embedded transport stub — handler migration lands in PR2".into())
        }
    }
}
