//! Curated bookmarks command: GET /2/users/{id}/bookmarks with pagination, max_results=100.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::config::ResolvedConfig;

/// Fetch all bookmarks for the authenticated user (id from /2/users/me), paginating with max_results=100.
pub async fn run_bookmarks(
    config: &ResolvedConfig,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = resolve_token_for_command(config, "bookmarks").await?;
    let access = match token {
        CommandToken::Bearer(t) => t,
        CommandToken::OAuth1 => unreachable!("bookmarks accepts OAuth2 user only per spec"),
    };
    let client = reqwest::Client::new();

    let me_res = client
        .get("https://api.x.com/2/users/me")
        .header("Authorization", format!("Bearer {}", access))
        .send()
        .await?;
    let status = me_res.status();
    let me_text = me_res.text().await?;
    if !status.is_success() {
        return Err(format!("GET /2/users/me failed: {}", me_text).into());
    }
    let me_json: serde_json::Value = serde_json::from_str(&me_text)?;
    let user_id = me_json
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("no data.id in /2/users/me response")?;

    let mut all_data: Vec<serde_json::Value> = Vec::new();
    let mut pagination_token: Option<String> = None;

    loop {
        let mut url = format!(
            "https://api.x.com/2/users/{}/bookmarks?max_results=100",
            user_id
        );
        if let Some(ref pt) = pagination_token {
            url.push_str("&pagination_token=");
            url.push_str(pt);
        }
        let res = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access))
            .send()
            .await?;
        let status = res.status();
        let text = res.text().await?;
        if !status.is_success() {
            return Err(format!("GET bookmarks failed: {}", text).into());
        }
        let page: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(data) = page.get("data").and_then(|d| d.as_array()) {
            for item in data {
                all_data.push(item.clone());
            }
        }
        pagination_token = page
            .get("meta")
            .and_then(|m| m.get("next_token"))
            .and_then(|t| t.as_str())
            .map(String::from);
        if pagination_token.is_none() {
            break;
        }
    }

    let out = serde_json::json!({ "data": all_data });
    if pretty {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
