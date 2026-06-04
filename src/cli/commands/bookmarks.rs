//! `bird bookmarks` — list bookmarks for the authenticated user.

use crate::bookmarks;
use crate::cli::dispatch::{ListFlags, clamp_limit};
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    pretty: bool,
    list_flags: &ListFlags,
) -> Result<(), BirdError> {
    let (limit, _) = clamp_limit(list_flags.limit, 100, 1000);
    bookmarks::run_bookmarks(
        client,
        out,
        stdout,
        stderr,
        bookmarks::BookmarkOpts {
            pretty,
            limit,
            cursor: list_flags.cursor.as_deref(),
        },
    )
    .map_err(|e| BirdError::from_source("bookmarks", e))?;
    Ok(())
}
