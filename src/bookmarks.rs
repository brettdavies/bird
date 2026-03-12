//! Curated bookmarks command: GET /2/users/{id}/bookmarks with pagination, max_results=100.

use crate::cost;
use crate::db::{BirdClient, BookmarkRow, RequestContext};
use crate::fields;
use crate::output;
use crate::requirements::AuthType;

/// Fetch bookmarks for the authenticated user, streaming each page to stdout as it arrives.
pub fn run_bookmarks(
    client: &mut BirdClient,
    pretty: bool,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Bookmarks require OAuth2 user context
    let auth_type = AuthType::OAuth2User;
    let ctx = RequestContext {
        auth_type: &auth_type,
        username: None,
    };

    // Fetch user ID via /2/users/me (goes through entity store)
    let me_response = client.get("https://api.x.com/2/users/me", &ctx)?;
    if !me_response.is_success() {
        return Err(format!(
            "GET /2/users/me failed: {}",
            output::sanitize_for_stderr(&me_response.body, 200)
        )
        .into());
    }
    let me_json = me_response.json.ok_or("invalid JSON from /2/users/me")?;
    let user_id = me_json
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("no data.id in /2/users/me response")?;

    let me_estimate = cost::estimate_cost(
        &me_json,
        "https://api.x.com/2/users/me",
        me_response.cache_hit,
    );
    cost::display_cost(&me_estimate, use_color);

    // Extract username from /users/me for bookmark relationship storage
    let me_username = me_json
        .get("data")
        .and_then(|d| d.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    let mut pagination_token: Option<String> = None;
    let mut first_item = true;
    let mut bookmark_rows: Vec<BookmarkRow> = Vec::new();
    let mut position: i64 = 0;

    // Open the JSON array wrapper
    if pretty {
        println!("{{\n  \"data\": [");
    } else {
        print!("{{\"data\":[");
    }

    loop {
        let url = {
            let mut u =
                url::Url::parse(&format!("https://api.x.com/2/users/{}/bookmarks", user_id))
                    .unwrap();
            {
                let mut pairs = u.query_pairs_mut();
                pairs.append_pair("max_results", "100");
                for (key, value) in fields::tweet_query_params() {
                    pairs.append_pair(key, value);
                }
                if let Some(ref pt) = pagination_token {
                    pairs.append_pair("pagination_token", pt);
                }
            }
            u.to_string()
        };

        let response = client.get(&url, &ctx)?;
        if !response.is_success() {
            return Err(format!(
                "GET bookmarks failed: {}",
                output::sanitize_for_stderr(&response.body, 200)
            )
            .into());
        }

        let page = response.json.ok_or("invalid JSON from bookmarks")?;
        let page_estimate = cost::estimate_cost(&page, &url, response.cache_hit);
        cost::display_cost(&page_estimate, use_color);

        if let Some(data) = page.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if !first_item {
                    if pretty {
                        println!(",");
                    } else {
                        print!(",");
                    }
                }
                first_item = false;
                if pretty {
                    let s = serde_json::to_string_pretty(item)?;
                    for line in s.lines() {
                        println!("    {}", line);
                    }
                } else {
                    print!("{}", serde_json::to_string(item)?);
                }
                // Accumulate bookmark relationships for storage
                if let Some(tweet_id) = item.get("id").and_then(|v| v.as_str()) {
                    bookmark_rows.push(BookmarkRow {
                        account_username: me_username.clone(),
                        tweet_id: tweet_id.to_string(),
                        position,
                        refreshed_at: crate::db::unix_now(),
                    });
                    position += 1;
                }
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

    // Store bookmark relationships in entity store
    if !me_username.is_empty() && !bookmark_rows.is_empty() {
        if let Some(db) = client.db() {
            if let Err(e) = db.replace_bookmarks(&me_username, &bookmark_rows) {
                eprintln!("[store] warning: bookmark storage failed: {e}");
            }
        }
    }

    // Close the JSON array wrapper
    if pretty {
        println!("\n  ]\n}}");
    } else {
        println!("]}}");
    }
    Ok(())
}
