//! `bird watchlist check` — stream NDJSON activity records for watched users.

use super::store::load_watchlist;
use crate::cli::auth_scheme::AuthType;
use crate::config::ResolvedConfig;
use crate::db::{BirdClient, RequestContext};
use crate::fields;

/// Options for `bird watchlist check`.
pub struct CheckOpts<'a> {
    pub pretty: bool,
    /// Maximum number of accounts to check this run.
    pub limit: u32,
    /// 0-based offset cursor into the watchlist for resumable checks.
    pub cursor: Option<&'a str>,
}

/// `bird watchlist check` — check recent activity for all watched users.
/// Streams NDJSON (one JSON object per line) per user as they complete.
///
/// The injected stdout writer is wrapped in a local `BufWriter` to amortize
/// the per-line lock/syscall cost across the stream. Flushed at end and on
/// every record boundary so partial output survives mid-stream errors.
pub fn run_watchlist_check(
    client: &mut BirdClient,
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    opts: CheckOpts<'_>,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    let quiet = cfg.suppress_diag();
    let pretty = opts.pretty;
    let config_path = config.config_dir.join("config.toml");
    let entries = load_watchlist(&config_path)?;

    if entries.is_empty() {
        if !quiet {
            writeln!(
                stderr,
                "Watchlist is empty. Add users with: bird watchlist add <username>"
            )
            .ok();
        }
        return Ok(());
    }

    let ctx = RequestContext {
        auth_type,
        username: None,
    };

    let mut writer = std::io::BufWriter::new(stdout);

    let offset = opts
        .cursor
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let cap = opts.limit as usize;
    let bounded: Vec<(usize, &String)> = entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(cap.max(1))
        .collect();
    let total = bounded.len();
    for (i, (_, username)) in bounded.iter().enumerate() {
        if !quiet {
            writeln!(
                stderr,
                "[watchlist] checking @{} ({}/{})...",
                username,
                i + 1,
                total
            )
            .ok();
        }

        let query = format!("from:{} -is:retweet", username);
        let search_url = build_check_url(&query);

        let activity = match execute_check(client, &ctx, &search_url, stderr, cfg) {
            Ok((tweet_count, latest_tweet, cache_hit)) => AccountActivity {
                username: (*username).clone(),
                recent_tweets: tweet_count,
                latest_tweet,
                cache_hit,
            },
            Err(e) => {
                if !quiet {
                    writeln!(stderr, "[watchlist] error checking @{}: {}", username, e).ok();
                }
                AccountActivity {
                    username: (*username).clone(),
                    recent_tweets: 0,
                    latest_tweet: None,
                    cache_hit: false,
                }
            }
        };

        if pretty {
            serde_json::to_writer_pretty(&mut writer, &activity)?;
        } else {
            serde_json::to_writer(&mut writer, &activity)?;
        }
        writeln!(writer)?;
        writer.flush()?;
    }
    writer.flush()?;
    Ok(())
}

fn build_check_url(query: &str) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent")
        .expect("invariant: constant search endpoint URL is well-formed");
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("query", query);
        pairs.append_pair("max_results", "10");
        for (key, value) in fields::tweet_query_params() {
            pairs.append_pair(key, value);
        }
    }
    url.to_string()
}

fn execute_check(
    client: &mut BirdClient,
    ctx: &RequestContext<'_>,
    url: &str,
    stderr: &mut dyn std::io::Write,
    cfg: &crate::output::OutputConfig,
) -> Result<(u64, Option<LatestTweet>, bool), Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url, ctx)?;

    if !response.is_success() {
        return Err(format!(
            "GET search {}: {}",
            response.status,
            crate::output::sanitize_for_stderr(&response.body(), 200)
        )
        .into());
    }

    let json = response.json.ok_or("invalid JSON from search")?;

    // Cost display per account
    let estimate = crate::cost::estimate_cost(&json, url, response.cache_hit);
    crate::cost::display_cost(cfg, stderr, &estimate);

    let tweet_count = json
        .get("meta")
        .and_then(|m| m.get("result_count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    let latest_tweet = extract_latest_tweet(&json);

    Ok((tweet_count, latest_tweet, response.cache_hit))
}

fn extract_latest_tweet(body: &serde_json::Value) -> Option<LatestTweet> {
    let data = body.get("data")?.as_array()?;
    let tweet = data.first()?;
    Some(LatestTweet {
        id: tweet.get("id")?.as_str()?.to_string(),
        text: tweet.get("text")?.as_str()?.to_string(),
        created_at: tweet
            .get("created_at")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string(),
        likes: tweet
            .get("public_metrics")
            .and_then(|m| m.get("like_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        retweets: tweet
            .get("public_metrics")
            .and_then(|m| m.get("retweet_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

#[derive(serde::Serialize)]
struct AccountActivity {
    username: String,
    recent_tweets: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_tweet: Option<LatestTweet>,
    cache_hit: bool,
}

#[derive(serde::Serialize)]
struct LatestTweet {
    id: String,
    text: String,
    created_at: String,
    likes: u64,
    retweets: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_latest_tweet_empty_data() {
        let body = serde_json::json!({"data": [], "meta": {"result_count": 0}});
        assert!(extract_latest_tweet(&body).is_none());
    }

    #[test]
    fn extract_latest_tweet_with_data() {
        let body = serde_json::json!({
            "data": [{
                "id": "123",
                "text": "hello world",
                "created_at": "2026-02-11T10:00:00.000Z",
                "public_metrics": {
                    "like_count": 42,
                    "retweet_count": 5
                }
            }]
        });
        let tweet = extract_latest_tweet(&body).expect("test");
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.text, "hello world");
        assert_eq!(tweet.likes, 42);
        assert_eq!(tweet.retweets, 5);
    }

    #[test]
    fn extract_latest_tweet_no_data_key() {
        let body = serde_json::json!({"meta": {"result_count": 0}});
        assert!(extract_latest_tweet(&body).is_none());
    }

    #[test]
    fn build_check_url_includes_query_and_limit() {
        let url = build_check_url("from:alice -is:retweet");
        assert!(url.starts_with("https://api.x.com/2/tweets/search/recent?"));
        assert!(url.contains("max_results=10"));
        assert!(url.contains("query=from%3Aalice"));
    }
}
