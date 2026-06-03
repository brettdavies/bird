//! `bird cache clear` and `bird cache stats` — local entity store management.

use crate::cli::CacheAction;
use crate::cli::dispatch::{GuardOutcome, require_confirmation};
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use std::io::Write;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    action: CacheAction,
    no_interactive: bool,
) -> Result<(), BirdError> {
    let quiet = out.suppress_diag();
    match action {
        CacheAction::Clear { guard } => {
            let target = client
                .db_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<store>".to_string());
            match require_confirmation(
                "clear",
                "LOCAL",
                &target,
                None,
                guard,
                out,
                no_interactive,
                stdout,
                stderr,
                None,
            )? {
                GuardOutcome::DryRun => return Ok(()),
                GuardOutcome::Proceed => {}
            }
            match client.db_clear() {
                Some(Ok(count)) => {
                    let stats = client.db_stats().and_then(|r| r.ok());
                    let size_str =
                        stats.map_or("0.0".to_string(), |s| format!("{:.1}", s.size_mb()));
                    if !quiet {
                        writeln!(
                            stderr,
                            "Cleared {} stored entities ({} MB).",
                            count, size_str
                        )
                        .ok();
                    }
                }
                Some(Err(e)) => {
                    return Err(BirdError::general(
                        "cache",
                        format!("failed to clear store: {}", e).into(),
                    ));
                }
                None => {
                    if !quiet {
                        writeln!(stderr, "Store is not available.").ok();
                    }
                }
            }
        }
        CacheAction::Stats { pretty } => match client.db_stats() {
            Some(Ok(stats)) => {
                let path = client
                    .db_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let data = serde_json::json!({
                    "path": path,
                    "size_mb": (stats.size_mb() * 10.0).round() / 10.0,
                    "max_size_mb": stats.max_size_mb() as u64,
                    "tweets": stats.tweet_count,
                    "users": stats.user_count,
                    "raw_responses": stats.raw_response_count,
                    "healthy": stats.healthy(),
                });
                if pretty {
                    writeln!(stdout, "Store: {}", path).map_err(cache_io_err)?;
                    writeln!(
                        stdout,
                        "Size:  {:.1} MB / {:.0} MB limit",
                        stats.size_mb(),
                        stats.max_size_mb()
                    )
                    .map_err(cache_io_err)?;
                    writeln!(stdout, "Tweets: {}", stats.tweet_count).map_err(cache_io_err)?;
                    writeln!(stdout, "Users:  {}", stats.user_count).map_err(cache_io_err)?;
                    writeln!(stdout, "Raw:    {}", stats.raw_response_count)
                        .map_err(cache_io_err)?;
                } else if out.is_raw_text() {
                    // --raw text: one key=value per line, pipe-safe.
                    writeln!(stdout, "path={}", path).map_err(cache_io_err)?;
                    writeln!(stdout, "size_mb={:.1}", stats.size_mb()).map_err(cache_io_err)?;
                    writeln!(stdout, "max_size_mb={:.0}", stats.max_size_mb())
                        .map_err(cache_io_err)?;
                    writeln!(stdout, "tweets={}", stats.tweet_count).map_err(cache_io_err)?;
                    writeln!(stdout, "users={}", stats.user_count).map_err(cache_io_err)?;
                    writeln!(stdout, "raw_responses={}", stats.raw_response_count)
                        .map_err(cache_io_err)?;
                    writeln!(stdout, "healthy={}", stats.healthy()).map_err(cache_io_err)?;
                } else {
                    let env = serde_json::json!({"data": data, "meta": {}});
                    out.print_envelope(stdout, &env).map_err(cache_io_err)?;
                }
            }
            Some(Err(e)) => {
                return Err(BirdError::general(
                    "cache",
                    format!("failed to read store stats: {}", e).into(),
                ));
            }
            None => {
                if !pretty && !out.is_raw_text() {
                    let env = serde_json::json!({
                        "data": {"healthy": false},
                        "meta": {"status": "store-unavailable"},
                    });
                    out.print_envelope(stdout, &env).map_err(cache_io_err)?;
                } else if !quiet {
                    writeln!(stderr, "Store is not available.").ok();
                }
            }
        },
    }
    Ok(())
}

fn cache_io_err(e: std::io::Error) -> BirdError {
    BirdError::general("cache", Box::<dyn std::error::Error + Send + Sync>::from(e))
}
