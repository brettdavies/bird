use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn bird() -> Command {
    Command::cargo_bin("bird").unwrap()
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
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success();
}

#[test]
fn watchlist_add_and_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Add alice
    bird()
        .args(["watchlist", "add", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    // List should contain alice
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn watchlist_add_remove_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["watchlist", "add", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    bird()
        .args(["watchlist", "remove", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    // List should be empty (no "alice")
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("alice").not());
}

#[test]
fn username_invalid_chars_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["--username", "'; DROP TABLE", "doctor"])
        .env("HOME", tmp.path())
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
    bird()
        .args(["--username", "@validuser", "doctor"])
        .env("HOME", tmp.path())
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
    bird()
        .args(["completions", "bash"])
        .env("HOME", tmp.path())
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
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .env("BIRD_QUIET", "1")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn bird_quiet_env_var_zero_does_not_activate() {
    // BIRD_QUIET=0 should NOT suppress stderr (FalseyValueParser)
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .env("BIRD_QUIET", "0")
        .assert()
        .success()
        .stderr(predicate::str::contains("Watchlist is empty"));
}

#[test]
fn quiet_flag_suppresses_watchlist_empty_hint() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["--quiet", "watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn quiet_flag_suppresses_watchlist_add_confirmation() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["--quiet", "watchlist", "add", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn quiet_flag_suppresses_watchlist_remove_message() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["--quiet", "watchlist", "remove", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn invalid_flag_exits_two() {
    bird().arg("--invalid-flag").assert().failure().code(2);
}
