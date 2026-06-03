//! `bird login` — OAuth2 authentication.

use crate::db;
use crate::diag;
use crate::error::BirdError;
use crate::login::{self, HeadlessAuthArgs};
use crate::output::OutputConfig;
use crate::transport;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    headless: HeadlessAuthArgs,
    username: Option<&str>,
) -> Result<(), BirdError> {
    let quiet = out.suppress_diag();
    if headless.no_browser {
        login::run_oauth2_authenticate_headless(out, stdout, username)
            .map_err(|e| BirdError::from_source("login", e))?;
    } else {
        transport::xurl_passthrough(&["auth", "oauth2"])
            .map_err(|e| BirdError::from_source("login", e))?;
    }
    if let Some(Ok(count)) = client.db_clear()
        && count > 0
    {
        diag!(
            quiet,
            "[store] Cleared {} stored entries after login.",
            count
        );
    }
    Ok(())
}
