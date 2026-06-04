//! Subprocess-bound smoke tests carved out from `cli_smoke.rs` per Plan 1 R25.
//!
//! Three carve-out classes live here:
//!
//! 1. **Clap exit class** — `version_flag` and `help_flag` verify the binary
//!    exits cleanly to the real process. In-process clap parse returns
//!    `Err(DisplayHelp/Version)` which the runner handles; the subprocess
//!    shape pins fidelity to the real-process behavior.
//!
//! 2. **`BIRD_XURL_PATH`-touching class** — three tests probe missing-binary
//!    error paths by pointing `BIRD_XURL_PATH` at a nonexistent file. Even
//!    with [`bird::transport::reset_xurl_path_for_tests`], in-process test
//!    ordering becomes load-bearing because the `OnceLock` cache poisons
//!    across tests. Defer to R22 (Transport trait redesign) for in-process
//!    migration.
//!
//! 3. **Stdout/stderr content-asserting class** — any test that asserts on
//!    captured stdout/stderr content stays forked until Plan 2 U11 routes
//!    the `out_println!` / `out_print!` / `diag!` macros through injected
//!    writers. Plan 1's library-style runner exposes the writer surface,
//!    but the macros still target global handles.
//!
//! Migration target: when Plan 2 U11 lands, the content-asserting class
//! collapses back into the library-style `cli_smoke.rs`. The clap-exit and
//! `BIRD_XURL_PATH` classes remain here as deliberate fidelity holdouts.

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

// --- Content-asserting class ----------------------------------------------

#[test]
fn watchlist_add_and_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Add alice
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "add", "alice"])
        .assert()
        .success();
    // List should contain alice
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn watchlist_add_remove_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "add", "alice"])
        .assert()
        .success();
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "remove", "alice", "--force"])
        .assert()
        .success();
    // List should be empty (no "alice")
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice").not());
}

#[test]
fn username_invalid_chars_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--username", "'; DROP TABLE", "doctor"])
        .env("NO_COLOR", "1")
        .assert()
        .failure()
        .code(78)
        .stderr(predicate::str::contains("--username"));
}

// --- Completions tests ---

#[test]
fn completions_bash_exits_zero() {
    bird()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn completions_zsh_contains_function_name() {
    bird()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_bird"));
}

#[test]
fn completions_fish_exits_zero() {
    bird()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn completions_powershell_exits_zero() {
    bird()
        .args(["completions", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn completions_elvish_exits_zero() {
    bird()
        .args(["completions", "elvish"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn completions_bash_contains_subcommand_names() {
    let output = bird().args(["completions", "bash"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("me"),
        "bash completions should contain 'me' subcommand"
    );
    assert!(
        stdout.contains("bookmarks"),
        "bash completions should contain 'bookmarks' subcommand"
    );
    assert!(
        stdout.contains("completions"),
        "bash completions should contain 'completions' subcommand"
    );
}

#[test]
fn completions_bash_output_is_substantial() {
    let output = bird().args(["completions", "bash"]).output().unwrap();
    assert!(
        output.stdout.len() > 1024,
        "bash completions should be >1KB for 28+ subcommands, got {} bytes",
        output.stdout.len()
    );
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

// --- Usage flag tests ---

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

// --- Quiet flag tests ---

#[test]
fn quiet_flag_with_help() {
    bird()
        .args(["--quiet", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("usage:")));
}

#[test]
fn quiet_flag_accepted_by_clap() {
    // --quiet with completions should succeed (no xurl needed)
    bird()
        .args(["--quiet", "completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn quiet_short_flag_accepted() {
    bird()
        .args(["-q", "completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

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

#[test]
fn quiet_flag_suppresses_watchlist_empty_hint() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--quiet", "watchlist", "list"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn quiet_flag_suppresses_watchlist_add_confirmation() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--quiet", "watchlist", "add", "alice"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn quiet_flag_suppresses_watchlist_remove_message() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--quiet", "watchlist", "remove", "alice", "--force"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

// --- U4: write-op guards (--force/--yes/--dry-run) ----------------------

#[test]
fn delete_without_force_or_tty_is_usage_error() {
    // Non-TTY invocation of a destructive command without --force MUST refuse.
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["delete", "/2/tweets/123", "--output", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["error"], "requires-confirmation");
    assert_eq!(json["kind"], "usage");
}

#[test]
fn delete_dry_run_emits_envelope_and_skips_request() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["delete", "/2/tweets/123", "--dry-run", "--output", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
    assert_eq!(json["data"]["would"]["method"], "DELETE");
}

#[test]
fn tweet_dry_run_includes_body() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["tweet", "hi there", "--dry-run", "--output", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
    assert_eq!(json["data"]["would"]["body"]["text"], "hi there");
}

#[test]
fn cache_clear_dry_run_does_not_clear() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["cache", "clear", "--dry-run", "--output", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
}

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

// --- JSON error output tests ---

#[test]
fn output_json_config_error_schema() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "--username", "'; DROP TABLE", "doctor"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
    assert_eq!(json["exit_code"], 78);
    assert!(json["error"].as_str().is_some());
    assert!(json.get("command").is_none());
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

#[test]
fn output_json_suppresses_diagnostics() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "json", "watchlist", "list"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn output_text_explicit_shows_text_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--output", "text", "--username", "'; DROP TABLE", "doctor"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("config failed:"),
        "Text mode should show human-readable errors, got: {}",
        stderr
    );
}

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

#[test]
fn non_tty_defaults_to_json_errors() {
    // In test environment stderr is not a TTY, so auto-detection should pick JSON
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--username", "'; DROP TABLE", "doctor"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should be parseable as JSON (auto-detected non-TTY -> json)
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
}

// --- Headless login tests ---

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

/// `bird --examples -o text` prints the curated block and exits zero.
#[test]
fn examples_flag_prints_block() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--examples", "--output", "text"])
        .output()
        .unwrap();
    assert!(output.status.success(), "--examples must exit zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Examples:"));
    assert!(stdout.contains("bird me"));
    assert!(stdout.contains("--output json"));
}

/// `bird --examples --output json` emits a JSON envelope listing commands.
#[test]
fn examples_flag_json_envelope() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = with_temp_home(&mut bird(), tmp.path())
        .args(["--examples", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("--examples --output json must emit JSON");
    let data = val.get("data").expect("envelope must have data");
    let arr = data.as_array().expect("data must be an array");
    assert!(!arr.is_empty(), "examples data array must be non-empty");
    assert!(
        arr.iter()
            .any(|v| v.as_str().is_some_and(|s| s.starts_with("bird "))),
        "every example should start with `bird `"
    );
    assert!(val.get("meta").is_some(), "envelope must have meta");
}
