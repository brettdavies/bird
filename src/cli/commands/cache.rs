//! `bird cache clear` and `bird cache stats` — local entity store management.

use crate::cli::CacheAction;
use crate::cli::dispatch::{GuardOutcome, require_confirmation};
use crate::db;
use crate::diag;
use crate::error::BirdError;
use crate::out_println;
use crate::output::{self, OutputConfig};

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
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
                &mut std::io::stderr().lock(),
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
                    diag!(
                        quiet,
                        "Cleared {} stored entities ({} MB).",
                        count,
                        size_str
                    );
                }
                Some(Err(e)) => {
                    return Err(BirdError::general(
                        "cache",
                        format!("failed to clear store: {}", e).into(),
                    ));
                }
                None => {
                    diag!(quiet, "Store is not available.");
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
                    out_println!("Store: {}", path);
                    out_println!(
                        "Size:  {:.1} MB / {:.0} MB limit",
                        stats.size_mb(),
                        stats.max_size_mb()
                    );
                    out_println!("Tweets: {}", stats.tweet_count);
                    out_println!("Users:  {}", stats.user_count);
                    out_println!("Raw:    {}", stats.raw_response_count);
                } else if out.is_raw_text() {
                    // --raw text: one key=value per line, pipe-safe.
                    out_println!("path={}", path);
                    out_println!("size_mb={:.1}", stats.size_mb());
                    out_println!("max_size_mb={:.0}", stats.max_size_mb());
                    out_println!("tweets={}", stats.tweet_count);
                    out_println!("users={}", stats.user_count);
                    out_println!("raw_responses={}", stats.raw_response_count);
                    out_println!("healthy={}", stats.healthy());
                } else {
                    let meta = serde_json::json!({});
                    let line = output::success_envelope_string(&data, &meta).map_err(|e| {
                        BirdError::general(
                            "cache",
                            Box::<dyn std::error::Error + Send + Sync>::from(e),
                        )
                    })?;
                    out_println!("{}", line);
                }
            }
            Some(Err(e)) => {
                return Err(BirdError::general(
                    "cache",
                    format!("failed to read store stats: {}", e).into(),
                ));
            }
            None => {
                let data = serde_json::json!({"healthy": false});
                let meta = serde_json::json!({"status": "store-unavailable"});
                if !pretty && !out.is_raw_text() {
                    let line = output::success_envelope_string(&data, &meta).map_err(|e| {
                        BirdError::general(
                            "cache",
                            Box::<dyn std::error::Error + Send + Sync>::from(e),
                        )
                    })?;
                    out_println!("{}", line);
                } else {
                    diag!(quiet, "Store is not available.");
                }
            }
        },
    }
    Ok(())
}
