#![cfg(all(unix, not(feature = "embedded-xurl")))]

//! Exit-code contract for the subprocess xurl transport.
//!
//! Locks the 78 / 77 / 1 codes bird emits today so the cutover to the
//! embedded xurl transport in PR2/PR3 cannot silently shift any documented
//! exit code. R4 in
//! `docs/plans/2026-06-05-001-refactor-embed-xurl-crate-plan.md` calls this
//! out as the contract anchor — codes 3 / 4 / 5 (rate-limited / not-found /
//! network, inherited from `xurl::error::exit_code_for_error`) land in U7's
//! commit alongside the new error-mapping path; this file covers what bird
//! ships today on default features.
//!
//! Cfg: `unix` (mock-xurl scripts use `/bin/sh`) AND
//! `not(feature = "embedded-xurl")` (PR1 stubs the embedded path so its
//! handlers do not produce the documented codes).

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Write a `/bin/sh` script that bird can spawn as `xurl`, marked executable.
fn write_mock_xurl(dir: &Path, body: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = dir.join(format!("mock_xurl_{pid}_{id}"));
    fs::write(&path, body).expect("write mock xurl");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod mock xurl");
    path
}

// --- 78 (config) ------------------------------------------------------------

/// Invalid `--username` value rejected at config-resolution time.
#[test]
fn invalid_username_exits_78() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--username", "'; DROP TABLE", "doctor"], &env);
    assert_eq!(exit, 78, "invalid --username must surface as config error");
    assert!(
        !stderr.trim().is_empty(),
        "stderr must carry a config-error envelope or message",
    );
}

/// Missing xurl binary on a network-bound command surfaces as a config error
/// because bird treats "no transport" as a startup misconfiguration, not a
/// runtime command failure.
#[test]
fn missing_xurl_binary_exits_78() {
    let env = common::TestEnv::new()
        .with_xurl_path(PathBuf::from("/tmp/nonexistent_xurl_for_exit_code_test"));
    let (exit, _stdout, stderr) = common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(exit, 78);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).expect("json error envelope");
    assert_eq!(json["kind"], "config");
    assert_eq!(json["exit_code"], 78);
}

/// Invalid `--output` value is rejected by clap. Clap's exit class lives in
/// `BirdError::Usage` → exit 2 (`EX_USAGE`), NOT 78 — pin that here so future
/// changes don't conflate the two.
#[test]
fn invalid_output_format_exits_2_not_78() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "--output", "xml", "me"], &env);
    assert_eq!(
        exit, 2,
        "clap usage errors must exit 2 — bird's 78 (config) is reserved for \
         post-clap config-resolution failures",
    );
}

// --- 77 (auth) --------------------------------------------------------------

/// Mock xurl returns a 401 auth-error envelope; bird's classifier maps the
/// 401/403 cases to `XurlError::Auth` → `BirdError::Auth` → exit 77.
#[test]
fn auth_error_from_xurl_exits_77() {
    let tmp = tempfile::TempDir::new().expect("tmpdir");
    let xurl = write_mock_xurl(
        tmp.path(),
        "#!/bin/sh\n\
         if [ \"$1\" = \"version\" ]; then\n  printf 'xurl 2.0.0\\n'\n  exit 0\nfi\n\
         printf '%s' '{\"title\":\"Unauthorized\",\"status\":401,\"detail\":\"Invalid token\"}'\n\
         exit 1\n",
    );

    let env = common::TestEnv::new().with_xurl_path(xurl);
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(exit, 77, "401 from xurl must surface as auth (exit 77)");
}

/// Mock xurl returns a 403 forbidden envelope; bird's classifier treats it as
/// an auth error too, sharing the 77 exit code with 401.
#[test]
fn forbidden_from_xurl_exits_77() {
    let tmp = tempfile::TempDir::new().expect("tmpdir");
    let xurl = write_mock_xurl(
        tmp.path(),
        "#!/bin/sh\n\
         if [ \"$1\" = \"version\" ]; then\n  printf 'xurl 2.0.0\\n'\n  exit 0\nfi\n\
         printf '%s' '{\"title\":\"Forbidden\",\"status\":403,\"detail\":\"no permission\"}'\n\
         exit 1\n",
    );

    let env = common::TestEnv::new().with_xurl_path(xurl);
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(exit, 77, "403 from xurl must surface as auth (exit 77)");
}

// --- 1 (general) ------------------------------------------------------------

/// Mock xurl returns a non-401/403 API error; bird routes through
/// `XurlError::Api` → `BirdError::General` → exit 1.
#[test]
fn server_error_from_xurl_exits_1() {
    let tmp = tempfile::TempDir::new().expect("tmpdir");
    let xurl = write_mock_xurl(
        tmp.path(),
        "#!/bin/sh\n\
         if [ \"$1\" = \"version\" ]; then\n  printf 'xurl 2.0.0\\n'\n  exit 0\nfi\n\
         printf '%s' '{\"title\":\"InternalError\",\"status\":500,\"detail\":\"boom\"}'\n\
         exit 1\n",
    );

    let env = common::TestEnv::new().with_xurl_path(xurl);
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(exit, 1, "5xx from xurl must surface as general (exit 1)");
}

/// Mock xurl exits non-zero with no JSON body — process-level transport
/// failure routes through `XurlError::Process` → `BirdError::General` → exit
/// 1.
#[test]
fn xurl_process_failure_exits_1() {
    let tmp = tempfile::TempDir::new().expect("tmpdir");
    let xurl = write_mock_xurl(
        tmp.path(),
        "#!/bin/sh\n\
         if [ \"$1\" = \"version\" ]; then\n  printf 'xurl 2.0.0\\n'\n  exit 0\nfi\n\
         printf 'segfault: boom\\n' 1>&2\n\
         exit 139\n",
    );

    let env = common::TestEnv::new().with_xurl_path(xurl);
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(
        exit, 1,
        "xurl process death must surface as general (exit 1)"
    );
}
