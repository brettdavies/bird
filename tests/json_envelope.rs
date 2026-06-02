//! Integration tests for the anc JSON error envelope and `--json` / `--jsonl` aliases.
//!
//! The envelope shape is `{"error", "kind", "message", "exit_code"}` per KTD-1 of the
//! anc 100% push plan. Clap parse failures route through the same formatter via
//! `Cli::try_parse()`.

use assert_cmd::Command;
use std::path::Path;

fn bird() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("bird")
}

fn with_temp_home<'a>(cmd: &'a mut Command, tmp: &Path) -> &'a mut Command {
    cmd.env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp.join(".config"))
}

fn parse_envelope(stderr: &[u8]) -> serde_json::Value {
    let s = String::from_utf8_lossy(stderr);
    serde_json::from_str(s.trim()).unwrap_or_else(|e| {
        panic!(
            "stderr did not parse as JSON envelope: {} stderr={:?}",
            e, s
        )
    })
}

#[test]
fn bad_flag_under_output_json_emits_envelope() {
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "--bogus-flag"])
        .output()
        .expect("test: spawn bird");

    assert_eq!(output.status.code(), Some(2), "usage errors exit 2");
    let env = parse_envelope(&output.stderr);
    assert_eq!(env["kind"], "usage", "kind should be usage");
    assert_eq!(env["exit_code"], 2, "exit_code field should be 2");
    assert!(env["error"].as_str().is_some(), "error field present");
    assert!(env["message"].as_str().is_some(), "message field present");
}

#[test]
fn bad_flag_under_json_alias_emits_envelope() {
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--json", "--bogus-flag"])
        .output()
        .expect("test: spawn bird");

    assert_eq!(output.status.code(), Some(2));
    let env = parse_envelope(&output.stderr);
    assert_eq!(env["kind"], "usage");
    assert_eq!(env["exit_code"], 2);
}

#[test]
fn bad_flag_under_jsonl_alias_emits_envelope() {
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--jsonl", "--bogus-flag"])
        .output()
        .expect("test: spawn bird");

    assert_eq!(output.status.code(), Some(2));
    let env = parse_envelope(&output.stderr);
    assert_eq!(env["kind"], "usage");
    assert_eq!(env["exit_code"], 2);
}

#[test]
fn config_error_emits_envelope() {
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "--username", "'; DROP TABLE", "doctor"])
        .output()
        .expect("test: spawn bird");

    assert_eq!(output.status.code(), Some(78), "config errors exit 78");
    let env = parse_envelope(&output.stderr);
    assert_eq!(env["kind"], "config");
    assert_eq!(env["exit_code"], 78);
    assert!(env["error"].as_str().is_some());
    assert!(env["message"].as_str().is_some());
}

#[test]
fn missing_required_argument_emits_envelope() {
    // `profile` requires a username positional arg.
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "profile"])
        .output()
        .expect("test: spawn bird");

    assert_eq!(output.status.code(), Some(2));
    let env = parse_envelope(&output.stderr);
    assert_eq!(env["kind"], "usage");
    assert_eq!(env["exit_code"], 2);
}

#[test]
fn json_alias_matches_output_json_for_help() {
    // Both `--json --help` and `--output json --help` should succeed identically.
    bird().args(["--json", "--help"]).assert().success();
    bird()
        .args(["--output", "json", "--help"])
        .assert()
        .success();
}

#[test]
fn timeout_flag_accepted() {
    // --timeout is global; should be accepted on any subcommand.
    bird()
        .args(["--timeout", "5", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn verbose_flag_accepted() {
    // -v should be accepted globally.
    bird()
        .args(["-v", "completions", "bash"])
        .assert()
        .success();
    bird()
        .args(["-vv", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn color_never_accepted() {
    bird()
        .args(["--color", "never", "completions", "bash"])
        .assert()
        .success();
    bird()
        .args(["--color", "always", "completions", "bash"])
        .assert()
        .success();
    bird()
        .args(["--color", "auto", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn raw_flag_accepted() {
    // --raw should not fail parsing on any subcommand.
    bird()
        .args(["--raw", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn plain_alias_still_accepted() {
    // Deprecated --plain should still parse (it's an alias for --color never).
    bird()
        .args(["--plain", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn no_color_alias_still_accepted() {
    bird()
        .args(["--no-color", "completions", "bash"])
        .assert()
        .success();
}

#[test]
fn envelope_error_id_is_kebab_case_for_unknown_arg() {
    let tmp = tempfile::TempDir::new().expect("test: tempdir");
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "--bogus-flag"])
        .output()
        .expect("test: spawn bird");
    let env = parse_envelope(&output.stderr);
    let id = env["error"].as_str().expect("test: error id is string");
    assert!(
        id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
        "error id should be kebab-case: {}",
        id
    );
}
