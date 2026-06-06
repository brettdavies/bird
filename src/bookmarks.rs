//! Curated bookmarks command: GET /2/users/{id}/bookmarks with pagination, max_results=100.

use crate::cli::auth_scheme::AuthType;
use crate::cost;
use crate::db::{BirdClient, BookmarkRow, RequestContext};
use crate::fields;
use crate::output;

/// Options bundle for `bird bookmarks` (keeps the public signature stable as
/// flags are added).
pub struct BookmarkOpts<'a> {
    pub pretty: bool,
    /// Maximum results per upstream page (X API caps `max_results` at 100).
    pub limit: u32,
    /// Optional pagination cursor that maps to X API `pagination_token`.
    pub cursor: Option<&'a str>,
}

/// Fetch bookmarks for the authenticated user, streaming each page to stdout as it arrives.
///
/// The injected stdout writer is wrapped in a local `BufWriter` to amortize
/// the per-record lock/syscall cost across the JSON stream. Flushed at end.
/// Mid-stream errors (transport, JSON, write) propagate `?`; the BufWriter
/// is dropped on unwind, flushing already-buffered bytes in debug builds.
/// In release builds bird uses `panic = "abort"`, so mid-stream aborts may
/// truncate the buffer — an accepted property of streaming output.
pub fn run_bookmarks(
    client: &mut BirdClient,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    opts: BookmarkOpts<'_>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(stdout);
    let quiet = cfg.suppress_diag();
    let pretty = opts.pretty;
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
            output::sanitize_for_stderr(&me_response.body(), 200)
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
    cost::display_cost(cfg, stderr, &me_estimate);

    // Extract username from /users/me for bookmark relationship storage
    let me_username = me_json
        .get("data")
        .and_then(|d| d.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    let mut pagination_token: Option<String> = opts.cursor.map(|s| s.to_string());
    let mut first_item = true;
    let mut bookmark_rows: Vec<BookmarkRow> = Vec::new();
    let mut position: i64 = 0;
    // X API caps `max_results` at 100; clamp here so callers can pass any value.
    let max_results = opts.limit.clamp(1, 100);
    // Stop after this many items so `--limit` bounds the response, not just
    // the per-page page size.
    let limit_cap: usize = opts.limit as usize;
    let mut emitted = 0usize;
    let mut next_cursor: Option<String> = None;

    // Open the JSON array wrapper
    if pretty {
        writeln!(out, "{{\n  \"data\": [")?;
    } else {
        write!(out, "{{\"data\":[")?;
    }

    loop {
        let url = {
            let mut u =
                url::Url::parse(&format!("https://api.x.com/2/users/{}/bookmarks", user_id))
                    .expect("invariant: bookmarks endpoint URL is well-formed");
            {
                let mut pairs = u.query_pairs_mut();
                pairs.append_pair("max_results", &max_results.to_string());
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
                output::sanitize_for_stderr(&response.body(), 200)
            )
            .into());
        }

        let page = response.json.ok_or("invalid JSON from bookmarks")?;
        let page_estimate = cost::estimate_cost(&page, &url, response.cache_hit);
        cost::display_cost(cfg, stderr, &page_estimate);

        if let Some(data) = page.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if limit_cap > 0 && emitted >= limit_cap {
                    break;
                }
                if !first_item {
                    if pretty {
                        writeln!(out, ",")?;
                    } else {
                        write!(out, ",")?;
                    }
                }
                first_item = false;
                if pretty {
                    let s = serde_json::to_string_pretty(item)?;
                    for line in s.lines() {
                        writeln!(out, "    {}", line)?;
                    }
                } else {
                    write!(out, "{}", serde_json::to_string(item)?)?;
                }
                emitted += 1;
                if let Some(tweet_id) = item.get("id").and_then(|v| v.as_str()) {
                    bookmark_rows.push(BookmarkRow {
                        username: me_username.clone(),
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
        if limit_cap > 0 && emitted >= limit_cap {
            next_cursor.clone_from(&pagination_token);
            break;
        }
        if pagination_token.is_none() {
            break;
        }
    }

    // Store bookmark relationships in entity store
    if !me_username.is_empty()
        && !bookmark_rows.is_empty()
        && let Some(db) = client.db()
        && let Err(e) = db.replace_bookmarks(&me_username, &bookmark_rows)
        && !quiet
    {
        writeln!(stderr, "[store] warning: bookmark storage failed: {e}").ok();
    }

    // Close the JSON array wrapper, appending a meta block when there is a
    // next cursor so agents can paginate without re-scanning.
    if let Some(ref tok) = next_cursor {
        if pretty {
            writeln!(out, "\n  ],")?;
            writeln!(out, "  \"meta\": {{ \"next_cursor\": {:?} }}", tok)?;
            writeln!(out, "}}")?;
        } else {
            write!(out, "],\"meta\":{{\"next_cursor\":")?;
            write!(out, "{}", serde_json::to_string(tok)?)?;
            writeln!(out, "}}}}")?;
        }
    } else if pretty {
        writeln!(out, "\n  ]\n}}")?;
    } else {
        writeln!(out, "]}}")?;
    }
    out.flush()?;
    Ok(())
}
