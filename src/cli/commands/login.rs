//! `bird login` — OAuth2 authentication.

use crate::config::ResolvedConfig;
use crate::db;
use crate::error::BirdError;
use crate::login::{self, HeadlessAuthArgs};
use crate::output::OutputConfig;

#[allow(clippy::too_many_arguments)]
pub fn run(
    client: &mut db::BirdClient,
    config: &ResolvedConfig,
    out: &OutputConfig,
    stdin: &mut dyn std::io::BufRead,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    headless: HeadlessAuthArgs,
) -> Result<(), BirdError> {
    let quiet = out.suppress_diag();
    let username = config.username.as_deref();

    if headless.no_browser {
        login::run_oauth2_authenticate_headless_embedded(out, stdout, stdin, config, username)?;
    } else {
        login::run_oauth2_authenticate_interactive_embedded(out, stdout, config, username)?;
    }

    if let Some(Ok(count)) = client.db_clear()
        && count > 0
        && !quiet
    {
        writeln!(
            stderr,
            "[store] Cleared {} stored entries after login.",
            count
        )
        .ok();
    }
    Ok(())
}
