//! Profile command: look up an X user by username, display JSON.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{RequestContext, CachedClient};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::output;
use crate::requirements::AuthType;
use reqwest::header::HeaderMap;

const USER_FIELDS: &str =
    "created_at,public_metrics,description,profile_image_url,location,verified,url";

/// Profile options bundled to avoid clippy::too_many_arguments.
pub struct ProfileOpts<'a> {
    pub username: &'a str,
    pub pretty: bool,
}

pub async fn run_profile(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    opts: ProfileOpts<'_>,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let username = validate_username(opts.username)?;

    let url = format!(
        "https://api.x.com/2/users/by/username/{}?user.fields={}",
        username, USER_FIELDS
    );

    let token = resolve_token_for_command(client.http(), config, "profile").await?;

    let response = match &token {
        CommandToken::Bearer(access) => {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", access).parse()?);
            let ctx = RequestContext {
                auth_type: &AuthType::OAuth2User,
                username: config.username.as_deref(),
            };
            client.get(&url, &ctx, headers).await?
        }
        CommandToken::OAuth1 => client.oauth1_request("GET", &url, config, None).await?,
    };

    if !response.status.is_success() {
        return Err(format!(
            "GET profile {}: {}",
            response.status,
            output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }

    let json = response.json.ok_or("invalid JSON in API response")?;

    // X API returns HTTP 200 with errors array for not-found users (not 404)
    if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
        if let Some(err) = errors.first() {
            let detail = err
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or("unknown error");
            return Err(format!("profile failed: {}", detail).into());
        }
    }

    let estimate = cost::estimate_cost(&json, &url, response.cache_hit);
    cost::display_cost(&estimate, use_color);

    if opts.pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }

    Ok(())
}

/// Validate and normalize username: strip leading '@', check 1-15 alphanumeric/underscore chars.
fn validate_username(username: &str) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
    let username = username.strip_prefix('@').unwrap_or(username);
    if username.is_empty() || username.len() > 15 {
        return Err(
            format!("username must be 1-15 characters, got {}", username.len()).into(),
        );
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("username contains invalid characters (only alphanumeric and underscore allowed)".into());
    }
    Ok(username)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_username_valid() {
        assert_eq!(validate_username("elonmusk").unwrap(), "elonmusk");
        assert_eq!(validate_username("a").unwrap(), "a");
        assert_eq!(validate_username("user_name_123").unwrap(), "user_name_123");
    }

    #[test]
    fn validate_username_strips_at() {
        assert_eq!(validate_username("@elonmusk").unwrap(), "elonmusk");
    }

    #[test]
    fn validate_username_empty() {
        assert!(validate_username("").is_err());
        assert!(validate_username("@").is_err());
    }

    #[test]
    fn validate_username_too_long() {
        assert!(validate_username("abcdefghijklmnop").is_err()); // 16 chars
    }

    #[test]
    fn validate_username_invalid_chars() {
        assert!(validate_username("user-name").is_err());
        assert!(validate_username("user.name").is_err());
        assert!(validate_username("user name").is_err());
    }
}
