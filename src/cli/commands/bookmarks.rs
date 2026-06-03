//! `bird bookmarks` — list bookmarks for the authenticated user.

use crate::bookmarks;
use crate::cli::dispatch::{ListFlags, clamp_limit};
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    pretty: bool,
    list_flags: &ListFlags,
) -> Result<(), BirdError> {
    let use_color = out.use_color;
    let quiet = out.suppress_diag();
    let (limit, _) = clamp_limit(list_flags.limit, 100, 1000);
    bookmarks::run_bookmarks(
        client,
        bookmarks::BookmarkOpts {
            pretty,
            limit,
            cursor: list_flags.cursor.as_deref(),
        },
        use_color,
        quiet,
    )
    .map_err(|e| BirdError::from_source("bookmarks", e))?;
    Ok(())
}
