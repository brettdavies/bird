//! `bird spend` — the local ledger of estimated API cost, with X's actual
//! figures beside it under `--sync`.

use crate::config::ResolvedConfig;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::spend;
use crate::store::ledger::Ledger;

#[allow(clippy::too_many_arguments)]
pub fn run(
    client: &mut db::BirdClient,
    config: &ResolvedConfig,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    since: Option<String>,
    sync: bool,
    pretty: bool,
) -> Result<(), BirdError> {
    let ledger = (!client.db_disabled()).then(|| Ledger::new(&config.store));
    spend::run_spend(
        client,
        ledger.as_ref(),
        out,
        stdout,
        stderr,
        since.as_deref(),
        !sync,
        pretty,
    )
    .map_err(|e| BirdError::from_source("spend", e))?;
    Ok(())
}
