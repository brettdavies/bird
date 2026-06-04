//! `bird usage` — view API usage and costs.

use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::usage;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    since: Option<String>,
    local: bool,
    pretty: bool,
) -> Result<(), BirdError> {
    let quiet = out.suppress_diag();
    usage::run_usage(client, since.as_deref(), local, pretty, quiet)
        .map_err(|e| BirdError::from_source("usage", e))?;
    Ok(())
}
