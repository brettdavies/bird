//! `bird me` and `bird get` — raw GET reads via `raw::run_raw`.

use crate::cli::dispatch::{ListFlags, clamp_limit, default_auth_type, parse_param_vec};
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::raw;
use std::collections::HashMap;

pub fn run_me(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    pretty: bool,
) -> Result<(), BirdError> {
    let params = HashMap::new();
    let auth_type = default_auth_type("me");
    raw::run_raw(
        client,
        out,
        stdout,
        "GET",
        "/2/users/me",
        &params,
        &[],
        None,
        pretty,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("me", e))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_get(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    path: String,
    param: Vec<String>,
    query: Vec<String>,
    pretty: bool,
    list_flags: &ListFlags,
) -> Result<(), BirdError> {
    let params = parse_param_vec(&param);
    let mut query = query;
    if let Some(ref tok) = list_flags.cursor {
        query.push(format!("pagination_token={}", tok));
    }
    if let Some(n) = list_flags.limit {
        let (clamped, _) = clamp_limit(Some(n), 100, 1000);
        query.push(format!("max_results={}", clamped));
    }
    let auth_type = default_auth_type("get");
    raw::run_raw(
        client, out, stdout, "GET", &path, &params, &query, None, pretty, &auth_type,
    )
    .map_err(|e| BirdError::from_source("get", e))?;
    Ok(())
}
