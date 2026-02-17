//! Raw request layer: HTTP method + path (with param substitution), query/body, auth, output.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{CachedClient, RequestContext};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::output;
use crate::requirements::AuthType;
use crate::schema::resolve_path;
use reqwest::header::HeaderMap;
use std::collections::HashMap;

/// Perform a raw API request: resolve path, get token (OAuth2, bearer, or OAuth 1.0a), send, print JSON.
#[allow(clippy::too_many_arguments)]
pub async fn run_raw(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    method: &str,
    path: &str,
    params: &HashMap<String, String>,
    query: &[String],
    body: Option<&str>,
    pretty: bool,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = resolve_path(path, params)?;
    let url = format!("https://api.x.com{}", path);
    let mut url_builder = url::Url::parse(&url).map_err(|e| e.to_string())?;
    for q in query {
        if let Some((k, v)) = q.split_once('=') {
            url_builder.query_pairs_mut().append_pair(k, v);
        }
    }
    let url = url_builder.to_string();

    let method_upper = method.to_uppercase();
    let command_name = method.to_lowercase();

    let token = resolve_token_for_command(client.http(), config, &command_name).await?;

    let response = match token {
        CommandToken::Bearer(access) => {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", access).parse()?);

            let ctx = RequestContext {
                auth_type: &AuthType::OAuth2User,
                username: config.username.as_deref(),
            };
            if method_upper == "GET" {
                let response = client.get(&url, &ctx, headers).await?;
                let estimate = cost::estimate_cost(
                    response.json.as_ref().unwrap_or(&serde_json::Value::Null),
                    &url,
                    response.cache_hit,
                );
                cost::display_cost(&estimate, use_color);
                response
            } else {
                let reqwest_method = match method_upper.as_str() {
                    "POST" => reqwest::Method::POST,
                    "PUT" => reqwest::Method::PUT,
                    "DELETE" => reqwest::Method::DELETE,
                    _ => return Err(format!("unsupported method: {}", method).into()),
                };
                if body.is_some() {
                    headers.insert("Content-Type", "application/json".parse()?);
                }
                client
                    .request(reqwest_method, &url, &ctx, headers, body.map(String::from))
                    .await?
            }
        }
        CommandToken::OAuth1 => {
            client
                .oauth1_request(&method_upper, &url, config, body)
                .await?
        }
    };

    if !response.status.is_success() {
        return Err(format!(
            "{} {}: {}",
            method,
            response.status,
            output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }
    // Use pre-parsed JSON; fall back to wrapping raw body as a JSON string
    let json = match response.json {
        Some(j) => j,
        None => serde_json::Value::String(response.body),
    };
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}
