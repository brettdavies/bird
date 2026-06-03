//! `bird profile` — look up a user profile by username.

use crate::cli::dispatch::default_auth_type;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::profile;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    username: String,
    pretty: bool,
) -> Result<(), BirdError> {
    let auth_type = default_auth_type("profile");
    profile::run_profile(
        client,
        out,
        stdout,
        profile::ProfileOpts {
            username: &username,
            pretty,
        },
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("profile", e))?;
    Ok(())
}
