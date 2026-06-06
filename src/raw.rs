//! Raw request layer: HTTP method + path (with param substitution), query/body, output.

use crate::cost;
use crate::db::{BirdClient, RequestContext};
use crate::output;
use crate::requirements::AuthType;
#[cfg(not(feature = "embedded-xurl"))]
use crate::schema::resolve_path;
use std::collections::HashMap;

/// Perform a raw API request: resolve path, send via xurl transport, print JSON.
///
/// Signature takes `&OutputConfig` and injected stdout/stderr writers
/// (Plan 2 U2/U6). `stderr` receives the cost-display line emitted by
/// `cost::display_cost` for GET requests.
#[allow(clippy::too_many_arguments)]
pub fn run_raw(
    client: &mut BirdClient,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    method: &str,
    path: &str,
    params: &HashMap<String, String>,
    query: &[String],
    body: Option<&str>,
    pretty: bool,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ctx = RequestContext {
        auth_type,
        username: None,
    };
    let method_upper = method.to_uppercase();

    #[cfg(not(feature = "embedded-xurl"))]
    let mut response = {
        let path = resolve_path(path, params)?;
        let url = format!("https://api.x.com{}", path);
        let mut url_builder = url::Url::parse(&url).map_err(|e| e.to_string())?;
        for q in query {
            if let Some((k, v)) = q.split_once('=') {
                url_builder.query_pairs_mut().append_pair(k, v);
            }
        }
        let url = url_builder.to_string();

        if method_upper == "GET" {
            let response = client.get(&url, &ctx)?;
            let estimate = cost::estimate_cost(
                response.json.as_ref().unwrap_or(&serde_json::Value::Null),
                &url,
                response.cache_hit,
            );
            cost::display_cost(cfg, stderr, &estimate);
            response
        } else {
            client.request(&method_upper, &url, &ctx, body)?
        }
    };

    // Embedded arm: bird hands the template + params + query to xurl, which
    // substitutes path params and runs the `auth_matrix` lookup against the
    // template (rather than against an already-rendered URL bird produces).
    // The bird-side entity-store decomposition that `client.get` performs is
    // intentionally skipped here — `bird raw` is the escape hatch, and the
    // typed handlers (U8) own the decomposition path.
    #[cfg(feature = "embedded-xurl")]
    let mut response = {
        let query_pairs: Vec<(String, String)> = query
            .iter()
            .filter_map(|q| {
                q.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .collect();
        let response = client.raw_template_request(
            &method_upper,
            path,
            params.clone(),
            query_pairs,
            body,
            &ctx,
        )?;
        if method_upper == "GET" {
            let estimate = cost::estimate_cost(
                response.json.as_ref().unwrap_or(&serde_json::Value::Null),
                path,
                response.cache_hit,
            );
            cost::display_cost(cfg, stderr, &estimate);
        }
        response
    };

    if !response.is_success() {
        return Err(format!(
            "{} {}: {}",
            method,
            response.status,
            output::sanitize_for_stderr(&response.body(), 200)
        )
        .into());
    }
    // `xurl_get` always sets `json: Some(_)` on success today; the fallback
    // preserves any raw body string the response carries (e.g.
    // raw_responses cache hits where the stored bytes don't parse as JSON).
    let json = match response.json.take() {
        Some(jv) => jv,
        None => serde_json::Value::String(response.into_body()),
    };
    if pretty {
        writeln!(stdout, "{}", serde_json::to_string_pretty(&json)?)?;
    } else {
        writeln!(stdout, "{}", serde_json::to_string(&json)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::client::ApiResponse;

    /// Mirrors the `response.json.take()` fallback at the top of `run_raw`:
    /// when `json` is None but `cached_body` carries the API bytes (a
    /// raw_responses cache hit with a non-JSON BLOB payload), the fallback
    /// reaches `into_body()` and must preserve the stored bytes verbatim.
    #[test]
    fn into_body_fallback_preserves_raw_bytes() {
        let payload = "not valid json: just bytes";
        let mut response = ApiResponse::for_test_raw_body(payload);
        assert!(response.json.is_none(), "fixture must omit json");
        let json = match response.json.take() {
            Some(jv) => jv,
            None => serde_json::Value::String(response.into_body()),
        };
        assert_eq!(json, serde_json::Value::String(payload.to_string()));
    }
}
