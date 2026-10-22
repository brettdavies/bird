//! `bird spend` — the local ledger of estimated API cost, with X's actual
//! figures beside it under `--sync`.

use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::spend;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    since: Option<String>,
    sync: bool,
    pretty: bool,
) -> Result<(), BirdError> {
    spend::run_spend(client, out, stdout, stderr, since.as_deref(), !sync, pretty)
        .map_err(|e| BirdError::from_source("spend", e))?;
    Ok(())
}
