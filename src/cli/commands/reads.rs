//! `bird me` — the authenticated user, read through `raw::run_raw`.

use crate::cli::auth_scheme::AuthType;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::raw;
use crate::store;
use std::collections::HashMap;

pub fn run_me(
    client: &mut store::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    pretty: bool,
) -> Result<(), BirdError> {
    let params = HashMap::new();
    let auth_type = AuthType::OAuth2User;
    raw::run_raw(
        client,
        out,
        stdout,
        stderr,
        "GET",
        "/2/users/me",
        &params,
        &[],
        &[],
        None,
        pretty,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("me", e))?;
    Ok(())
}
