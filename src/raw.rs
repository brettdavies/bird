//! Raw request layer: HTTP method + path (with param substitution), query/body, auth, output.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{RequestContext, CachedClient};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::requirements::AuthType;
use crate::schema::resolve_path;
use reqwest::header::HeaderMap;
use reqwest_oauth1::OAuthClientProvider;
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

    let (_auth_type, status, text) = match token {
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
                    &serde_json::from_str(&response.body).unwrap_or(serde_json::Value::Null),
                    &url,
                    response.cache_hit,
                );
                cost::display_cost(&estimate, use_color);
                (AuthType::OAuth2User, response.status, response.body)
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
                let response = client
                    .request(reqwest_method, &url, &ctx, headers, body.map(String::from))
                    .await?;
                (AuthType::OAuth2User, response.status, response.body)
            }
        }
        CommandToken::OAuth1 => {
            // OAuth1 uses reqwest_oauth1 which needs the raw reqwest::Client.
            // Cannot go through CachedClient.get() because oauth1 signs the request internally.
            let ck = config.oauth1_consumer_key.as_ref().unwrap();
            let cs = config.oauth1_consumer_secret.as_ref().unwrap();
            let at = config.oauth1_access_token.as_ref().unwrap();
            let ats = config.oauth1_access_token_secret.as_ref().unwrap();
            let secrets = reqwest_oauth1::Secrets::new(ck.as_str(), cs.as_str())
                .token(at.as_str(), ats.as_str());
            let mut req = match method_upper.as_str() {
                "GET" => client.http().clone().oauth1(secrets).get(&url),
                "POST" => client.http().clone().oauth1(secrets).post(&url),
                "PUT" => client.http().clone().oauth1(secrets).put(&url),
                "DELETE" => client.http().clone().oauth1(secrets).delete(&url),
                _ => return Err(format!("unsupported method: {}", method).into()),
            };
            if let Some(b) = body {
                req = req
                    .header("Content-Type", "application/json")
                    .body(b.to_string());
            }
            let res = req.send().await?;
            let status = res.status();
            let text = res.text().await?;
            client.log_api_call(&url, &method_upper, &text, false, config.username.as_deref());
            (AuthType::OAuth1, status, text)
        }
    };

    if !status.is_success() {
        return Err(format!("{} {}: {}", method, status, text).into());
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}
