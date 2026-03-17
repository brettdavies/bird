//! Profile command: look up an X user by username, display JSON.

use crate::cost;
use crate::db::{BirdClient, RequestContext};
use crate::fields;
use crate::output;
use crate::requirements::AuthType;
use crate::schema;

/// Profile options bundled to avoid clippy::too_many_arguments.
pub struct ProfileOpts<'a> {
    pub username: &'a str,
    pub pretty: bool,
}

pub fn run_profile(
    client: &mut BirdClient,
    opts: ProfileOpts<'_>,
    use_color: bool,
    quiet: bool,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let username = schema::validate_username(opts.username)?;

    let url = {
        let mut u = url::Url::parse(&format!(
            "https://api.x.com/2/users/by/username/{}",
            username
        ))
        .unwrap();
        {
            let mut pairs = u.query_pairs_mut();
            for (key, value) in fields::user_query_params() {
                pairs.append_pair(key, value);
            }
        }
        u.to_string()
    };

    let ctx = RequestContext {
        auth_type,
        username: None,
    };
    let response = client.get(&url, &ctx)?;

    if !response.is_success() {
        return Err(format!(
            "GET profile {}: {}",
            response.status,
            output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }

    let json = response.json.ok_or("invalid JSON in API response")?;

    // X API returns HTTP 200 with errors array for not-found users (not 404)
    if let Some(errors) = json.get("errors").and_then(|e| e.as_array())
        && let Some(err) = errors.first()
    {
        let detail = err
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("unknown error");
        return Err(format!("profile failed: {}", detail).into());
    }

    let estimate = cost::estimate_cost(&json, &url, response.cache_hit);
    cost::display_cost(&estimate, use_color, quiet);

    if opts.pretty {
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", serde_json::to_string(&json)?);
    }

    Ok(())
}
