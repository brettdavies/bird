//! Raw request layer: HTTP method + path (with param substitution), query/body, output.

use crate::cost;
use crate::db::{BirdClient, RequestContext};
use crate::output;
use crate::requirements::AuthType;
use crate::schema::resolve_path;
use std::collections::HashMap;

/// Perform a raw API request: resolve path, send via xurl transport, print JSON.
///
/// U2 (Plan 2) note: signature takes `&OutputConfig` and a stdout writer; the
/// body still calls `crate::out_println!` (U3 / R13 replaces those).
#[allow(clippy::too_many_arguments)]
pub fn run_raw(
    client: &mut BirdClient,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    method: &str,
    path: &str,
    params: &HashMap<String, String>,
    query: &[String],
    body: Option<&str>,
    pretty: bool,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = stdout;
    let path = resolve_path(path, params)?;
    let url = format!("https://api.x.com{}", path);
    let mut url_builder = url::Url::parse(&url).map_err(|e| e.to_string())?;
    for q in query {
        if let Some((k, v)) = q.split_once('=') {
            url_builder.query_pairs_mut().append_pair(k, v);
        }
    }
    let url = url_builder.to_string();

    let ctx = RequestContext {
        auth_type,
        username: None,
    };

    let method_upper = method.to_uppercase();
    let response = if method_upper == "GET" {
        let response = client.get(&url, &ctx)?;
        let estimate = cost::estimate_cost(
            response.json.as_ref().unwrap_or(&serde_json::Value::Null),
            &url,
            response.cache_hit,
        );
        cost::display_cost(cfg, &estimate);
        response
    } else {
        client.request(&method_upper, &url, &ctx, body)?
    };

    if !response.is_success() {
        return Err(format!(
            "{} {}: {}",
            method,
            response.status,
            output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }
    let json = match response.json {
        Some(j) => j,
        None => serde_json::Value::String(response.body),
    };
    if pretty {
        crate::out_println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        crate::out_println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}
