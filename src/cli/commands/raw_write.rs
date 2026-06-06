//! `bird post`, `bird put`, `bird delete` — raw write requests via `raw::run_raw`.

use crate::cli::WriteGuard;
use crate::cli::dispatch::{
    GuardOutcome, build_dry_run_url, default_auth_type, parse_param_vec, require_confirmation,
};
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::raw;

#[allow(clippy::too_many_arguments)]
pub fn run_post(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    path: String,
    param: Vec<String>,
    query: Vec<String>,
    body: Option<String>,
    header: Vec<String>,
    pretty: bool,
    guard: WriteGuard,
    no_interactive: bool,
) -> Result<(), BirdError> {
    let params = parse_param_vec(&param);
    let target = build_dry_run_url(&path, &params, &query)
        .unwrap_or_else(|| format!("https://api.x.com{}", path));
    let body_json = body.as_deref().and_then(|s| serde_json::from_str(s).ok());
    match require_confirmation(
        "POST",
        "POST",
        &target,
        body_json.as_ref(),
        guard,
        out,
        no_interactive,
        stdout,
        stderr,
        None,
    )? {
        GuardOutcome::DryRun => return Ok(()),
        GuardOutcome::Proceed => {}
    }
    let auth_type = default_auth_type("post");
    raw::run_raw(
        client,
        out,
        stdout,
        stderr,
        "POST",
        &path,
        &params,
        &query,
        &header,
        body.as_deref(),
        pretty,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("post", e))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_put(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    path: String,
    param: Vec<String>,
    query: Vec<String>,
    body: Option<String>,
    header: Vec<String>,
    pretty: bool,
    guard: WriteGuard,
    no_interactive: bool,
) -> Result<(), BirdError> {
    let params = parse_param_vec(&param);
    let target = build_dry_run_url(&path, &params, &query)
        .unwrap_or_else(|| format!("https://api.x.com{}", path));
    let body_json = body.as_deref().and_then(|s| serde_json::from_str(s).ok());
    match require_confirmation(
        "PUT",
        "PUT",
        &target,
        body_json.as_ref(),
        guard,
        out,
        no_interactive,
        stdout,
        stderr,
        None,
    )? {
        GuardOutcome::DryRun => return Ok(()),
        GuardOutcome::Proceed => {}
    }
    let auth_type = default_auth_type("put");
    raw::run_raw(
        client,
        out,
        stdout,
        stderr,
        "PUT",
        &path,
        &params,
        &query,
        &header,
        body.as_deref(),
        pretty,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("put", e))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_delete(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    path: String,
    param: Vec<String>,
    query: Vec<String>,
    header: Vec<String>,
    pretty: bool,
    guard: WriteGuard,
    no_interactive: bool,
) -> Result<(), BirdError> {
    let params = parse_param_vec(&param);
    let target = build_dry_run_url(&path, &params, &query)
        .unwrap_or_else(|| format!("https://api.x.com{}", path));
    match require_confirmation(
        "delete",
        "DELETE",
        &target,
        None,
        guard,
        out,
        no_interactive,
        stdout,
        stderr,
        None,
    )? {
        GuardOutcome::DryRun => return Ok(()),
        GuardOutcome::Proceed => {}
    }
    let auth_type = default_auth_type("delete");
    raw::run_raw(
        client, out, stdout, stderr, "DELETE", &path, &params, &query, &header, None, pretty,
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("delete", e))?;
    Ok(())
}
