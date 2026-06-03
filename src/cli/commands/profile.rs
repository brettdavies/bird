//! `bird profile` — look up a user profile by username.

use crate::cli::dispatch::default_auth_type;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::profile;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    username: String,
    pretty: bool,
) -> Result<(), BirdError> {
    let use_color = out.use_color;
    let quiet = out.suppress_diag();
    let auth_type = default_auth_type("profile");
    profile::run_profile(
        client,
        profile::ProfileOpts {
            username: &username,
            pretty,
        },
        use_color,
        quiet,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("profile", e))?;
    Ok(())
}
