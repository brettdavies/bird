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

    // `watchlist list` writes a single bounded JSON document — no buffering
    // needed. Per U4, BufWriter adoption stays bounded to the streaming
    // handler (`run_watchlist_check`); per-call list output writes directly
    // to the injected writer.
    if next_cursor.is_some() {
        let payload = serde_json::json!({
            "data": window,
            "meta": { "next_cursor": next_cursor },
        });
        if pretty {
            writeln!(stdout, "{}", serde_json::to_string_pretty(&payload)?)?;
        } else {
            writeln!(stdout, "{}", serde_json::to_string(&payload)?)?;
        }
    } else if pretty {
        writeln!(stdout, "{}", serde_json::to_string_pretty(&window)?)?;
    } else {
        writeln!(stdout, "{}", serde_json::to_string(&window)?)?;
    }
    Ok(())
}

/// `bird watchlist add <username>` — add a user to the watchlist (idempotent).
/// Silent on success; no stdout writer.
pub fn run_watchlist_add(
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stderr: &mut dyn std::io::Write,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quiet = cfg.suppress_diag();
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    store::add_to_watchlist(&config_path, clean, stderr, quiet)?;
    if !quiet {
        writeln!(stderr, "Added @{} to watchlist.", clean).ok();
    }
    Ok(())
}

/// `bird watchlist remove <username>` — remove a user from the watchlist (idempotent).
/// Silent on success; no stdout writer.
pub fn run_watchlist_remove(
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stderr: &mut dyn std::io::Write,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quiet = cfg.suppress_diag();
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    let removed = store::remove_from_watchlist(&config_path, clean)?;
    if !quiet {
        if removed {
            writeln!(stderr, "Removed @{} from watchlist.", clean).ok();
        } else {
            writeln!(stderr, "@{} was not in the watchlist.", clean).ok();
        }
    }
    Ok(())
}
