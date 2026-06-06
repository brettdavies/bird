//! `bird login` — OAuth2 authentication.

use crate::db;
use crate::error::BirdError;
use crate::login::{self, HeadlessAuthArgs};
use crate::output::OutputConfig;
#[cfg(not(feature = "embedded-xurl"))]
use crate::transport;

#[allow(clippy::too_many_arguments)]
pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    headless: HeadlessAuthArgs,
    username: Option<&str>,
    app: Option<&str>,
) -> Result<(), BirdError> {
    let quiet = out.suppress_diag();

    #[cfg(not(feature = "embedded-xurl"))]
    {
        let _ = app;
        let xurl_path = client.xurl_path().ok_or_else(|| {
            BirdError::config(format!("xurl not found. {}", transport::XURL_INSTALL_HINT))
        })?;
        if headless.no_browser {
            login::run_oauth2_authenticate_headless(out, stdout, xurl_path, username)
                .map_err(|e| BirdError::from_source("login", e))?;
        } else {
            transport::xurl_passthrough(&["auth", "oauth2"], xurl_path)
                .map_err(|e| BirdError::from_source("login", e))?;
        }
    }

    #[cfg(feature = "embedded-xurl")]
    {
        if headless.no_browser {
            login::run_oauth2_authenticate_headless_embedded(out, stdout, username, app)
                .map_err(|e| BirdError::from_source("login", e))?;
        } else {
            login::run_oauth2_authenticate_interactive_embedded(out, stdout, username, app)
                .map_err(|e| BirdError::from_source("login", e))?;
        }
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
