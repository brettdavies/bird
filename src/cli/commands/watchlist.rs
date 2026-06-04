//! `bird watchlist fetch` — check recent activity for all watched users.
//!
//! `Add`/`Remove`/`List` are pre-dispatched in `fn main` (they don't need xurl
//! and don't reach the dispatcher's `run` function).

use crate::cli::dispatch::{ListFlags, clamp_limit, default_auth_type};
use crate::config::ResolvedConfig;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::watchlist;

pub fn run_fetch(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    config: &ResolvedConfig,
    pretty: bool,
    list_flags: &ListFlags,
) -> Result<(), BirdError> {
    let use_color = out.use_color;
    let quiet = out.suppress_diag();
    let (limit, _) = clamp_limit(list_flags.limit, 100, 1000);
    let auth_type = default_auth_type("watchlist_check");
    watchlist::run_watchlist_check(
        client,
        config,
        watchlist::CheckOpts {
            pretty,
            limit,
            cursor: list_flags.cursor.as_deref(),
        },
        use_color,
        quiet,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("watchlist", e))?;
    Ok(())
}
