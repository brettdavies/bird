use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

#[allow(deprecated)]
fn bird() -> Command {
    Command::cargo_bin("bird").unwrap()
}

/// Set HOME and XDG_CONFIG_HOME to isolate config from the CI environment.
/// Without this, XDG_CONFIG_HOME (if set on the runner) overrides HOME,
/// causing parallel tests to share one config file — a race condition.
fn with_temp_home<'a>(cmd: &'a mut Command, tmp: &Path) -> &'a mut Command {
    cmd.env("HOME", tmp)
        .env("XDG_CONFIG_HOME", tmp.join(".config"))
}

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

#[test]
fn no_args_shows_usage() {
    bird().assert().failure().code(2);
}

#[test]
fn watchlist_list_empty_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["watchlist", "list"])
        .assert()
        .success();
}

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
        .args(["watchlist", "remove", "alice"])
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

#[test]
fn username_at_prefix_normalized() {
    // @validuser should be accepted (normalized to validuser).
    // Doctor runs successfully — the username value is valid after stripping @.
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["--username", "@validuser", "doctor"])
        .env("NO_COLOR", "1")
        .assert()
        .success();
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
fn completions_invalid_shell_exits_two() {
    bird()
        .args(["completions", "invalid-shell"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn completions_no_argument_exits_two() {
    bird().args(["completions"]).assert().failure().code(2);
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

#[test]
fn completions_works_without_xurl() {
    bird()
        .args(["completions", "bash"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_12345")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn completions_does_not_create_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    with_temp_home(&mut bird(), tmp.path())
        .args(["completions", "bash"])
        .assert()
        .success();
    // Completions should not create any config directory
    assert!(
        !tmp.path().join(".config/bird").exists(),
        "completions should not create config directory"
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
        .args(["--quiet", "watchlist", "remove", "alice"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_flag_exits_two() {
    bird().arg("--invalid-flag").assert().failure().code(2);
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
    assert_eq!(json["code"], 78);
    assert!(json["error"].as_str().is_some());
    assert!(json.get("command").is_none());
}

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
    assert_eq!(json["code"], 78);
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
