//! `bird usage` — view API usage and costs.

use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::usage;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    since: Option<String>,
    local: bool,
    pretty: bool,
) -> Result<(), BirdError> {
    usage::run_usage(client, out, stdout, stderr, since.as_deref(), local, pretty)
        .map_err(|e| BirdError::from_source("usage", e))?;
    Ok(())
}
