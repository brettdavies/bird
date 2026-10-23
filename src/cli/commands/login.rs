//! `bird login` — OAuth2 authentication. Stored responses are kept per
//! account, so a sign-in leaves the store alone.

use crate::config::ResolvedConfig;
use crate::error::BirdError;
use crate::login::{self, HeadlessAuthArgs};
use crate::output::OutputConfig;

pub fn run(
    config: &ResolvedConfig,
    out: &OutputConfig,
    stdin: &mut dyn std::io::BufRead,
    stdout: &mut dyn std::io::Write,
    headless: HeadlessAuthArgs,
) -> Result<(), BirdError> {
    let username = config.username.as_deref();
    if headless.no_browser {
        login::run_oauth2_authenticate_headless_embedded(out, stdout, stdin, config, username)?;
    } else {
        login::run_oauth2_authenticate_interactive_embedded(out, stdout, config, username)?;
    }
    Ok(())
}
