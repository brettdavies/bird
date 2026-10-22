//! `bird watchlist fetch` — stream NDJSON activity records for watched users.

use super::store::load_watchlist;
use crate::cli::auth_scheme::AuthType;
use crate::config::ResolvedConfig;
use crate::db::{BirdClient, RequestContext};
use crate::fields;

/// Options for `bird watchlist fetch`.
pub struct CheckOpts<'a> {
    pub pretty: bool,
    /// Maximum number of accounts to check this run.
    pub limit: u32,
    /// 0-based offset cursor into the watchlist for resumable checks.
    pub cursor: Option<&'a str>,
}

/// `bird watchlist fetch` — check recent activity for all watched users.
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
        if cfg.format == crate::output::OutputFormat::Json {
            cfg.print_ok(stdout, serde_json::json!({ "data": [], "meta": {} }))?;
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
    // `--output json` prints one ok document; every other mode streams one
    // activity object per line as each user is checked.
    let one_document = cfg.format == crate::output::OutputFormat::Json;
    let mut collected: Vec<serde_json::Value> = Vec::new();
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
            Ok((post_count, latest_post, cache_hit)) => AccountActivity {
                username: (*username).clone(),
                recent_posts: post_count,
                latest_post,
                cache_hit,
            },
            Err(e) if crate::shutdown::Cancelled::find(e.as_ref()).is_some() => return Err(e),
            Err(e) => {
                if !quiet {
                    writeln!(stderr, "[watchlist] error checking @{}: {}", username, e).ok();
                }
                AccountActivity {
                    username: (*username).clone(),
                    recent_posts: 0,
                    latest_post: None,
                    cache_hit: false,
                }
            }
        };

        if one_document {
            collected.push(serde_json::to_value(&activity)?);
            continue;
        }
        if pretty {
            serde_json::to_writer_pretty(&mut writer, &activity)?;
        } else {
            serde_json::to_writer(&mut writer, &activity)?;
        }
        writeln!(writer)?;
        writer.flush()?;
    }
    if one_document {
        let payload = serde_json::json!({ "data": collected, "meta": {} });
        cfg.print_ok_pretty(&mut writer, payload, pretty)?;
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
        for (key, value) in fields::post_query_params() {
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
) -> Result<(u64, Option<LatestPost>, bool), Box<dyn std::error::Error + Send + Sync>> {
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

    let post_count = json
        .get("meta")
        .and_then(|m| m.get("result_count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    let latest_post = extract_latest_post(&json);

    Ok((post_count, latest_post, response.cache_hit))
}

fn extract_latest_post(body: &serde_json::Value) -> Option<LatestPost> {
    let data = body.get("data")?.as_array()?;
    let tweet = data.first()?;
    Some(LatestPost {
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
        reposts: tweet
            .get("public_metrics")
            .and_then(|m| m.get("repost_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

#[derive(serde::Serialize)]
struct AccountActivity {
    username: String,
    recent_posts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_post: Option<LatestPost>,
    cache_hit: bool,
}

#[derive(serde::Serialize)]
struct LatestPost {
    id: String,
    text: String,
    created_at: String,
    likes: u64,
    reposts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::CacheOpts;
    use crate::error::BirdError;
    use crate::output::{OutputConfig, OutputFormat};
    use crate::shutdown::Cancelled;
    use crate::xurl_client::mock::MockXurlClient;
    use std::sync::{Arc, Mutex};

    /// A signal during one account's check ends the run with that
    /// cancellation instead of logging it and requesting the next account.
    #[test]
    fn a_cancelled_check_ends_the_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("config.toml"),
            "watchlist = [\"alice\", \"bob\"]\n",
        )
        .expect("write config");
        let config = ResolvedConfig {
            username: None,
            config_dir: dir.path().to_path_buf(),
            cache_path: dir.path().join("bird.db"),
            cache_enabled: false,
            cache_max_size_mb: 10,
            token_store: Some(dir.path().join("auth.yml")),
            xdk_env: xdk::config::EnvOverrides::default(),
            app: None,
        };
        let mock = MockXurlClient::new();
        mock.push_err(
            "send_request",
            Cancelled {
                signal: libc::SIGTERM,
            }
            .into(),
        );
        let calls = mock.clone_handle();
        let mut client = BirdClient::new(
            Box::new(mock),
            &config.cache_path,
            CacheOpts {
                no_store: true,
                ..CacheOpts::default()
            },
            10,
            None,
            true,
            Arc::new(Mutex::new(std::io::sink())),
        );
        let out = OutputConfig {
            format: OutputFormat::Json,
            use_color: false,
            quiet: true,
            raw: false,
        };
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());

        let err = run_watchlist_check(
            &mut client,
            &config,
            &out,
            &mut stdout,
            &mut stderr,
            CheckOpts {
                pretty: false,
                limit: 100,
                cursor: None,
            },
            &AuthType::Bearer,
        )
        .expect_err("the cancellation ends the run");

        assert_eq!(BirdError::from_source("watchlist", err).exit_code(), 143);
        assert_eq!(calls.calls().len(), 1, "bob must not be requested");
        assert!(stdout.is_empty(), "no record for a cancelled check");
    }

    #[test]
    fn extract_latest_post_empty_data() {
        let body = serde_json::json!({"data": [], "meta": {"result_count": 0}});
        assert!(extract_latest_post(&body).is_none());
    }

    #[test]
    fn extract_latest_post_with_data() {
        let body = serde_json::json!({
            "data": [{
                "id": "123",
                "text": "hello world",
                "created_at": "2026-02-11T10:00:00.000Z",
                "public_metrics": {
                    "like_count": 42,
                    "repost_count": 5
                }
            }]
        });
        let tweet = extract_latest_post(&body).expect("test");
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.text, "hello world");
        assert_eq!(tweet.likes, 42);
        assert_eq!(tweet.reposts, 5);
    }

    #[test]
    fn extract_latest_post_no_data_key() {
        let body = serde_json::json!({"meta": {"result_count": 0}});
        assert!(extract_latest_post(&body).is_none());
    }

    #[test]
    fn build_check_url_includes_query_and_limit() {
        let url = build_check_url("from:alice -is:retweet");
        assert!(url.starts_with("https://api.x.com/2/tweets/search/recent?"));
        assert!(url.contains("max_results=10"));
        assert!(url.contains("query=from%3Aalice"));
    }
}
