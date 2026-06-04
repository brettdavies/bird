//! Subprocess-bound smoke tests — the R25 carve-outs.
//!
//! Two carve-out classes survive Plan 2 U11:
//!
//! 1. **Clap exit class** — `--version` and top-level / subcommand `--help`
//!    routes go through clap's [`clap::Error::print()`] path, which writes
//!    directly to the real process stdout. The runner's writer-injection
//!    cannot intercept this without rewriting clap; the subprocess shape
//!    pins fidelity to the real-process behavior. Library-style coverage of
//!    the JSON-wrapped help envelope lives in `tests/cli_smoke.rs` and
//!    `tests/envelope_consistency.rs`.
//!
//! 2. **`BIRD_XURL_PATH`-touching class** — three tests probe missing-binary
//!    error paths by pointing `BIRD_XURL_PATH` at a nonexistent file. Even
//!    with [`bird::transport::reset_xurl_path_for_tests`], in-process test
//!    ordering becomes load-bearing because the `OnceLock` cache poisons
//!    across tests. Deferred to a Transport-redesign follow-up.
//!
//! 3. **`BIRD_QUIET` env-var class** — two tests verify the env-var path
//!    rather than the `--quiet` flag. Plan 1's [`bird::config::EnvOverrides`]
//!    snapshot does not propagate `BIRD_QUIET` to clap (clap reads it via
//!    `env = "BIRD_QUIET"` on the flag itself, from the real process env),
//!    so a subprocess is the cleanest way to exercise the env path without
//!    leaking env state across in-process tests. `--quiet` flag coverage
//!    lives in `tests/cli_smoke.rs`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

fn bird() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("bird")
}

/// Set HOME and XDG_CONFIG_HOME to isolate config from the CI environment.
/// Without this, XDG_CONFIG_HOME (if set on the runner) overrides HOME,
/// causing parallel tests to share one config file — a race condition.
fn with_temp_home<'a>(cmd: &'a mut Command, tmp: &Path) -> &'a mut Command {
    cmd.env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp.join(".config"))
}

// --- Clap-exit class -------------------------------------------------------

#[test]
fn version_flag() {
    bird()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bird"));
}

#[test]
fn help_flag() {
    bird()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("usage:")));
}

// --- BIRD_XURL_PATH-touching carve-out (R25) -----------------------------

#[test]
fn completions_works_without_xurl() {
    bird()
        .args(["completions", "bash"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_12345")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// --- Subcommand `--help` (clap-exit) class --------------------------------

#[test]
fn usage_help_shows_local_flag() {
    bird()
        .args(["usage", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--local"));
}

#[test]
fn usage_help_does_not_show_sync_flag() {
    let output = bird().args(["usage", "--help"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("--sync"),
        "usage --help should not contain --sync, got: {}",
        stdout
    );
}

// --- BIRD_XURL_PATH-touching carve-out (R25) -----------------------------

#[test]
fn usage_local_flag_accepted_by_clap() {
    // --local should be accepted by clap (exits later due to missing xurl, but not exit 2)
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["usage", "--local"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_12345")
        .output()
        .unwrap();
    // Should NOT be exit 2 (clap parse error) — any other exit is fine
    assert_ne!(
        output.status.code(),
        Some(2),
        "--local should be accepted by clap"
    );
}

// --- Quiet flag + help (clap-exit) ----------------------------------------

#[test]
fn quiet_flag_with_help() {
    bird()
        .args(["--quiet", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("usage:")));
}

// --- BIRD_QUIET env-var carve-out (R25) -----------------------------------

#[test]
fn bird_quiet_env_var_activates_quiet() {
    // BIRD_QUIET=1 should suppress stderr diagnostics
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "list"])
        .env("BIRD_QUIET", "1")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn bird_quiet_env_var_zero_does_not_activate() {
    // BIRD_QUIET=0 should NOT suppress stderr (FalseyValueParser)
    // --output text forces text mode in non-TTY test environment
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "text", "watchlist", "list"])
        .env("BIRD_QUIET", "0")
        .assert()
        .success()
        .stderr(predicate::str::contains("Watchlist is empty"));
}

// --- Top-level `--help` (clap-exit) ---------------------------------------

#[test]
fn global_limit_and_cursor_flags_present() {
    // Top-level help advertises --limit and --cursor; anc's behavioral audit
    // requires these in the global flag surface.
    let output = bird().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--limit"),
        "global --limit missing from help"
    );
    assert!(
        stdout.contains("--cursor"),
        "global --cursor missing from help"
    );
}

// --- BIRD_XURL_PATH-touching carve-out (R25) -----------------------------

#[test]
fn output_json_command_error_schema() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "me"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_12345")
        .output()
        .unwrap();

    // xurl not found => config error (exit 78)
    assert_eq!(output.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
    assert_eq!(json["exit_code"], 78);
}

// --- BIRD_OUTPUT env-var (real process env) -------------------------------

#[test]
fn bird_output_env_var_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--username", "'; DROP TABLE", "doctor"])
        .env("BIRD_OUTPUT", "json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
}

// --- Subcommand `--help` (clap-exit) class --------------------------------

#[test]
fn login_help_advertises_no_browser_flag() {
    let assert = bird().args(["login", "--help"]).assert().success();
    let out = assert.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--no-browser"),
        "expected `--no-browser` in `bird login --help`, got: {stdout}",
    );
}

/// `bird --help` carries a top-level `Examples:` block with at least one
/// text + `--output json` paired invocation.
#[test]
fn top_level_help_has_paired_examples() {
    let output = bird().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Examples:"),
        "top-level --help missing 'Examples:' section"
    );
    assert!(
        stdout.contains("--output json"),
        "top-level --help missing a `--output json` example"
    );
}
