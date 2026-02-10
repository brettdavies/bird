//! Raw request layer: HTTP method + path (with param substitution), query/body, auth, output.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::config::ResolvedConfig;
use crate::schema::resolve_path;
use reqwest_oauth1::OAuthClientProvider;
use std::collections::HashMap;

/// Perform a raw API request: resolve path, get token (OAuth2, bearer, or OAuth 1.0a), send, print JSON.
pub async fn run_raw(
    client: &reqwest::Client,
    config: &ResolvedConfig,
    method: &str,
    path: &str,
    params: &HashMap<String, String>,
    query: &[String],
    body: Option<&str>,
    pretty: bool,
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

    let token = resolve_token_for_command(client, config, &command_name).await?;

    let (status, text) = match token {
        CommandToken::Bearer(access) => {
            let mut req = match method_upper.as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                _ => return Err(format!("unsupported method: {}", method).into()),
            };
            req = req.header("Authorization", format!("Bearer {}", access));
            if let Some(b) = body {
                req = req.header("Content-Type", "application/json").body(b.to_string());
            }
            let res = req.send().await?;
            (res.status(), res.text().await?)
        }
        CommandToken::OAuth1 => {
            let ck = config.oauth1_consumer_key.as_ref().unwrap();
            let cs = config.oauth1_consumer_secret.as_ref().unwrap();
            let at = config.oauth1_access_token.as_ref().unwrap();
            let ats = config.oauth1_access_token_secret.as_ref().unwrap();
            let secrets = reqwest_oauth1::Secrets::new(ck.as_str(), cs.as_str()).token(at.as_str(), ats.as_str());
            let mut req = match method_upper.as_str() {
                "GET" => client.clone().oauth1(secrets).get(&url),
                "POST" => client.clone().oauth1(secrets).post(&url),
                "PUT" => client.clone().oauth1(secrets).put(&url),
                "DELETE" => client.clone().oauth1(secrets).delete(&url),
                _ => return Err(format!("unsupported method: {}", method).into()),
            };
            if let Some(b) = body {
                req = req.header("Content-Type", "application/json").body(b.to_string());
            }
            let res = req.send().await?;
            (res.status(), res.text().await?)
        }
    };

    if !status.is_success() {
        return Err(format!("{} {}: {}", method, status, text).into());
    }
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text));
    if pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }
    Ok(())
}
