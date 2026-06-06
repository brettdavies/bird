//! `bird profile` — look up a user profile by username.

use crate::cli::auth_scheme::AuthType;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::profile;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    username: String,
    pretty: bool,
) -> Result<(), BirdError> {
    let auth_type = AuthType::OAuth2User;
    profile::run_profile(
        client,
        out,
        stdout,
        stderr,
        profile::ProfileOpts {
            username: &username,
            pretty,
        },
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("profile", e))?;
    Ok(())
}
