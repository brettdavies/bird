//! `bird search` — search recent tweets.

use crate::cli::auth_scheme::AuthType;
use crate::cli::dispatch::ListFlags;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::search;

#[allow(clippy::too_many_arguments)]
pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    query: String,
    pretty: bool,
    sort: String,
    min_likes: Option<u64>,
    max_results: Option<u32>,
    pages: Option<u32>,
    list_flags: &ListFlags,
) -> Result<(), BirdError> {
    let auth_type = AuthType::OAuth2User;
    // `--limit` is the canonical agent-facing flag; `--max-results` is kept
    // as the per-page Twitter-API knob. When both are set, `--limit` wins.
    let resolved_max = list_flags.limit.or(max_results);
    let opts = search::SearchOpts {
        query: &query,
        pretty,
        sort: &sort,
        min_likes,
        max_results: resolved_max.unwrap_or(100).clamp(10, 100),
        pages: pages.unwrap_or(1).clamp(1, 10),
        cursor: list_flags.cursor.as_deref(),
    };
    search::run_search(client, out, stdout, stderr, opts, &auth_type)
        .map_err(|e| BirdError::from_source("search", e))?;
    Ok(())
}
