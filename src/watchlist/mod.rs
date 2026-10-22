//! Watchlist command: manage and check a curated list of X users.
//! Config-driven (config.toml), uses toml_edit for formatting-preserving writes.

mod check;
mod store;

pub use check::{CheckOpts, run_watchlist_check};

use crate::config::ResolvedConfig;
use crate::schema;

/// `bird watchlist list` — display the current watchlist as JSON.
///
/// `limit` clamps the number of returned entries; `cursor` is a 0-based offset
/// encoded as a numeric string (the local watchlist is small enough that an
/// integer offset is a more natural cursor than a token).
pub fn run_watchlist_list(
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    pretty: bool,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quiet = cfg.suppress_diag();
    let config_path = config.config_dir.join("config.toml");
    let entries = store::load_watchlist(&config_path)?;

    if entries.is_empty() && !quiet {
        writeln!(
            stderr,
            "Watchlist is empty. Add users with: bird watchlist add <username>"
        )
        .ok();
    }

    let offset = cursor.and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let total = entries.len();
    let cap = limit.map(|l| l as usize).unwrap_or(total);
    let end = offset.saturating_add(cap).min(total);
    let window: Vec<&String> = entries.iter().skip(offset).take(cap).collect();
    let next_cursor = if end < total {
        Some(end.to_string())
    } else {
        None
    };

    // `watchlist list` writes a single bounded JSON document, so it needs no
    // buffering; only the streaming handler (`run_watchlist_check`) buffers.
    let mut meta = serde_json::Map::new();
    if let Some(cursor) = next_cursor {
        meta.insert("next_cursor".into(), serde_json::Value::String(cursor));
    }
    let payload = serde_json::json!({ "data": window, "meta": meta });
    cfg.print_ok_pretty(stdout, payload, pretty)?;
    Ok(())
}

/// Report a watchlist change: an ok message on stdout under JSON, a line on
/// stderr in text mode unless quiet.
fn report(
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    message: &str,
) -> std::io::Result<()> {
    if cfg.format.is_json() {
        cfg.print_ok_message(stdout, message)
    } else {
        if !cfg.suppress_diag() {
            writeln!(stderr, "{message}").ok();
        }
        Ok(())
    }
}

/// `bird watchlist add <username>` — add a user to the watchlist (idempotent).
pub fn run_watchlist_add(
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quiet = cfg.suppress_diag();
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    store::add_to_watchlist(&config_path, clean, stderr, quiet)?;
    report(
        cfg,
        stdout,
        stderr,
        &format!("Added @{} to watchlist.", clean),
    )?;
    Ok(())
}

/// `bird watchlist remove <username>` — remove a user from the watchlist (idempotent).
pub fn run_watchlist_remove(
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    let removed = store::remove_from_watchlist(&config_path, clean)?;
    let message = if removed {
        format!("Removed @{} from watchlist.", clean)
    } else {
        format!("@{} was not in the watchlist.", clean)
    };
    report(cfg, stdout, stderr, &message)?;
    Ok(())
}
