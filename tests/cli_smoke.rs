//! Library-style smoke tests.
//!
//! Every test runs in-process via [`common::run_in_process`]; both the exit
//! code and the captured stdout/stderr content are asserted where the
//! runner's writer-injection makes them observable.
//!
//! The transport layer holds the resolved xurl path on the
//! [`bird::transport::XurlTransport`] instance constructed per call, so
//! `xurl_path`-touching tests are safe to run in-process. The remaining
//! subprocess holdouts in [`tests/cli_smoke_subprocess.rs`] cover clap's
//! `e.print()` help/version path and env-var-only flag paths.
//!
//! The `every_subcommand_help_has_example` and
//! `nested_subcommand_help_has_example` tests call clap's
//! [`clap::Command::write_help`] directly on the parsed [`bird::cli::Cli`]
//! command tree — no runner, no subprocess.

use clap::CommandFactory;

mod common;

// --- Exit-only migrations -------------------------------------------------

#[test]
fn no_args_shows_usage() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird"], &env);
    assert_eq!(exit, 2);
}

#[test]
fn watchlist_list_empty_config() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "watchlist", "list"], &env);
    assert_eq!(exit, 0);
    // Empty config → empty JSON array on stdout (default output mode in a
    // non-TTY test environment is JSON per the runner's auto-detect path).
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn username_at_prefix_normalized() {
    // @validuser should be accepted (normalized to validuser).
    // Doctor runs successfully — the username value is valid after stripping @.
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "--username", "@validuser", "doctor"], &env);
    assert_eq!(exit, 0);
}

#[test]
fn completions_invalid_shell_exits_two() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) =
        common::run_in_process(&["bird", "completions", "invalid-shell"], &env);
    assert_eq!(exit, 2);
}

#[test]
fn completions_no_argument_exits_two() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "completions"], &env);
    assert_eq!(exit, 2);
}

#[test]
fn completions_does_not_create_config() {
    let env = common::TestEnv::new();
    let config_dir = env.paths.config_dir.clone();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    // Completions should not populate the config directory with state files.
    // `TestEnv::new()` pre-creates the directory itself, so check for the
    // bird config.toml that would be written if the loader ran.
    assert!(
        !config_dir.join("config.toml").exists(),
        "completions should not create config.toml"
    );
}

#[test]
fn usage_sync_flag_rejected() {
    // --sync should be rejected by clap (unknown flag)
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "usage", "--sync"], &env);
    assert_eq!(exit, 2);
}

#[test]
fn invalid_flag_exits_two() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "--invalid-flag"], &env);
    assert_eq!(exit, 2);
}

#[test]
fn watchlist_remove_yes_alias_proceeds() {
    let env = common::TestEnv::new();
    let (exit_add, _, _) = common::run_in_process(&["bird", "watchlist", "add", "alice"], &env);
    assert_eq!(exit_add, 0);
    let (exit_remove, _, _) =
        common::run_in_process(&["bird", "watchlist", "remove", "alice", "--yes"], &env);
    assert_eq!(exit_remove, 0);
}

#[test]
fn login_no_browser_parses() {
    // With closed stdin and an isolated HOME, the command must reach the headless
    // path and exit non-zero (xurl absent or rejects empty redirect URL) — not a
    // clap usage error.
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "login", "--no-browser"], &env);
    assert_ne!(exit, 2, "clap usage error: --no-browser not recognized");
}

#[test]
fn login_headless_alias_parses() {
    let env = common::TestEnv::new();
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "login", "--headless"], &env);
    assert_ne!(exit, 2, "clap usage error: --headless alias not recognized");
}

// --- Content-asserting migrations from cli_smoke_subprocess.rs (Plan 2 U11) -

#[test]
fn watchlist_add_and_list() {
    let env = common::TestEnv::new();
    let (exit_add, _, _) = common::run_in_process(&["bird", "watchlist", "add", "alice"], &env);
    assert_eq!(exit_add, 0);
    let (exit_list, stdout, _) = common::run_in_process(&["bird", "watchlist", "list"], &env);
    assert_eq!(exit_list, 0);
    assert!(
        stdout.contains("alice"),
        "watchlist list stdout should contain alice, got: {:?}",
        stdout
    );
}

#[test]
fn watchlist_add_remove_list() {
    let env = common::TestEnv::new();
    let (exit_add, _, _) = common::run_in_process(&["bird", "watchlist", "add", "alice"], &env);
    assert_eq!(exit_add, 0);
    let (exit_rm, _, _) =
        common::run_in_process(&["bird", "watchlist", "remove", "alice", "--force"], &env);
    assert_eq!(exit_rm, 0);
    let (exit_list, stdout, _) = common::run_in_process(&["bird", "watchlist", "list"], &env);
    assert_eq!(exit_list, 0);
    assert!(
        !stdout.contains("alice"),
        "watchlist list should not contain alice after remove, got: {:?}",
        stdout
    );
}

#[test]
fn username_invalid_chars_rejected() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "--output",
            "text",
            "--username",
            "'; DROP TABLE",
            "doctor",
        ],
        &env,
    );
    assert_eq!(exit, 78);
    assert!(
        stderr.contains("--username"),
        "stderr should mention --username, got: {:?}",
        stderr
    );
}

// --- Completions content assertions ---------------------------------------

#[test]
fn completions_bash_exits_zero() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty(), "completions bash must emit stdout");
}

#[test]
fn completions_zsh_contains_function_name() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "zsh"], &env);
    assert_eq!(exit, 0);
    assert!(
        stdout.contains("_bird"),
        "zsh completions should mention `_bird` function name"
    );
}

#[test]
fn completions_fish_exits_zero() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "fish"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty(), "completions fish must emit stdout");
}

#[test]
fn completions_powershell_exits_zero() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "completions", "powershell"], &env);
    assert_eq!(exit, 0);
    assert!(
        !stdout.is_empty(),
        "completions powershell must emit stdout"
    );
}

#[test]
fn completions_elvish_exits_zero() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "elvish"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty(), "completions elvish must emit stdout");
}

#[test]
fn completions_bash_contains_subcommand_names() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
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
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(
        stdout.len() > 1024,
        "bash completions should be >1KB for 28+ subcommands, got {} bytes",
        stdout.len()
    );
}

// --- Quiet-flag content assertions (env-var variants stay subprocess) -----

#[test]
fn quiet_flag_suppresses_watchlist_empty_hint() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--quiet", "watchlist", "list"], &env);
    assert_eq!(exit, 0);
    assert!(
        stderr.is_empty(),
        "--quiet must suppress the watchlist empty hint, got stderr: {:?}",
        stderr
    );
}

#[test]
fn quiet_flag_suppresses_watchlist_add_confirmation() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--quiet", "watchlist", "add", "alice"], &env);
    assert_eq!(exit, 0);
    assert!(
        stderr.is_empty(),
        "--quiet must suppress the watchlist add confirmation, got stderr: {:?}",
        stderr
    );
}

#[test]
fn quiet_flag_suppresses_watchlist_remove_message() {
    let env = common::TestEnv::new();
    let (_, _, _) = common::run_in_process(&["bird", "watchlist", "add", "alice"], &env);
    let (exit, _stdout, stderr) = common::run_in_process(
        &["bird", "--quiet", "watchlist", "remove", "alice", "--force"],
        &env,
    );
    assert_eq!(exit, 0);
    assert!(
        stderr.is_empty(),
        "--quiet must suppress the watchlist remove message, got stderr: {:?}",
        stderr
    );
}

#[test]
fn quiet_flag_accepted_by_clap() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "--quiet", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn quiet_short_flag_accepted() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "-q", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

// --- Write-op guards (--force / --yes / --dry-run) ------------------------

#[test]
fn delete_without_force_or_tty_is_usage_error() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &["bird", "delete", "/2/tweets/123", "--output", "json"],
        &env,
    );
    assert_eq!(exit, 2);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["error"], "requires-confirmation");
    assert_eq!(json["kind"], "usage");
}

#[test]
fn delete_dry_run_emits_envelope_and_skips_request() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(
        &[
            "bird",
            "delete",
            "/2/tweets/123",
            "--dry-run",
            "--output",
            "json",
        ],
        &env,
    );
    assert_eq!(exit, 0);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
    assert_eq!(json["data"]["would"]["method"], "DELETE");
}

#[test]
fn tweet_dry_run_includes_body() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(
        &["bird", "tweet", "hi there", "--dry-run", "--output", "json"],
        &env,
    );
    assert_eq!(exit, 0);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
    assert_eq!(json["data"]["would"]["body"]["text"], "hi there");
}

#[test]
fn cache_clear_dry_run_does_not_clear() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(
        &["bird", "cache", "clear", "--dry-run", "--output", "json"],
        &env,
    );
    assert_eq!(exit, 0);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["data"]["dry_run"], true);
}

// --- JSON error output schemas --------------------------------------------

#[test]
fn output_json_config_error_schema() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "--output",
            "json",
            "--username",
            "'; DROP TABLE",
            "doctor",
        ],
        &env,
    );
    assert_eq!(exit, 78);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
    assert_eq!(json["exit_code"], 78);
    assert!(json["error"].as_str().is_some());
    assert!(json.get("command").is_none());
}

#[test]
fn output_json_suppresses_diagnostics() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "watchlist", "list"], &env);
    assert_eq!(exit, 0);
    assert!(
        stderr.is_empty(),
        "--output json must suppress diagnostics, got stderr: {:?}",
        stderr
    );
}

#[test]
fn output_text_explicit_shows_text_errors() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "--output",
            "text",
            "--username",
            "'; DROP TABLE",
            "doctor",
        ],
        &env,
    );
    assert_eq!(exit, 78);
    assert!(
        stderr.contains("config failed:"),
        "text mode should show human-readable errors, got: {:?}",
        stderr
    );
}

#[test]
fn non_tty_defaults_to_json_errors() {
    // Cargo test harness runs non-TTY, so the runner's auto-detect must pick JSON.
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--username", "'; DROP TABLE", "doctor"], &env);
    assert_eq!(exit, 78);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
}

// --- `--examples` block ---------------------------------------------------

#[test]
fn examples_flag_prints_block() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "--examples", "--output", "text"], &env);
    assert_eq!(exit, 0, "--examples must exit zero");
    assert!(stdout.contains("Examples:"));
    assert!(stdout.contains("bird me"));
    assert!(stdout.contains("--output json"));
}

#[test]
fn examples_flag_json_envelope() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "--examples", "--output", "json"], &env);
    assert_eq!(exit, 0);
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

/// `bird --examples --output json` smoke-checks that captured stdout carries
/// content — guards against any future regression that re-introduces a real
/// stdout bypass for runner short-circuits.
#[test]
fn run_in_process_captures_stdout_for_examples() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "--examples"], &env);
    assert_eq!(exit, 0);
    assert!(
        stdout.contains("bird"),
        "stdout should contain bird example invocations: {:?}",
        stdout
    );
}

// --- Structural migrations via clap::Command::write_help ------------------

/// Every subcommand's `--help` must include an example invocation line
/// (matches anc's p3-must-subcommand-examples detection rules).
///
/// Migrated to library-style by walking `Cli::command().get_subcommands()` and
/// calling `clap::Command::write_help` directly — bypasses both the runner and
/// the subprocess path; clap renders the same bytes either way.
#[test]
fn every_subcommand_help_has_example() {
    let subcommands = [
        "login",
        "me",
        "get",
        "post",
        "put",
        "bookmarks",
        "profile",
        "search",
        "thread",
        "delete",
        "watchlist",
        "usage",
        "tweet",
        "reply",
        "like",
        "unlike",
        "repost",
        "unrepost",
        "follow",
        "unfollow",
        "dm",
        "block",
        "unblock",
        "mute",
        "unmute",
        "doctor",
        "cache",
        "completions",
        "skill",
    ];
    let mut cli = bird::cli::Cli::command();
    for sub_name in subcommands {
        let sub = cli
            .find_subcommand_mut(sub_name)
            .unwrap_or_else(|| panic!("subcommand `{sub_name}` missing from Cli"));
        let mut buf: Vec<u8> = Vec::new();
        sub.write_help(&mut buf)
            .unwrap_or_else(|e| panic!("write_help({sub_name}): {e}"));
        let stdout = String::from_utf8_lossy(&buf);
        let has_marker = stdout.contains("Examples:")
            || stdout.contains("EXAMPLES")
            || stdout
                .lines()
                .any(|l| l.trim_start().starts_with("bird ") || l.trim_start().starts_with("$ "));
        assert!(
            has_marker,
            "`bird {sub_name} --help` missing example marker; got:\n{stdout}"
        );
    }
}

/// Nested subcommands also need their own example blocks (anc walks each).
#[test]
fn nested_subcommand_help_has_example() {
    let nested = [
        ("watchlist", "check"),
        ("watchlist", "add"),
        ("watchlist", "remove"),
        ("watchlist", "list"),
        ("cache", "clear"),
        ("cache", "stats"),
    ];
    let mut cli = bird::cli::Cli::command();
    for (outer, inner) in nested {
        let outer_cmd = cli
            .find_subcommand_mut(outer)
            .unwrap_or_else(|| panic!("outer subcommand `{outer}` missing"));
        let inner_cmd = outer_cmd
            .find_subcommand_mut(inner)
            .unwrap_or_else(|| panic!("nested subcommand `{outer} {inner}` missing"));
        let mut buf: Vec<u8> = Vec::new();
        inner_cmd
            .write_help(&mut buf)
            .unwrap_or_else(|e| panic!("write_help({outer} {inner}): {e}"));
        let stdout = String::from_utf8_lossy(&buf);
        let has_marker = stdout.contains("Examples:")
            || stdout.contains("EXAMPLES")
            || stdout
                .lines()
                .any(|l| l.trim_start().starts_with("bird ") || l.trim_start().starts_with("$ "));
        assert!(
            has_marker,
            "`bird {outer} {inner} --help` missing example marker"
        );
    }
}

// --- xurl-path-touching migrations (R22 — transport state, no global cache) -

/// `bird completions <shell>` short-circuits before any xurl resolution.
/// Pointing the snapshot at a non-existent path proves completions never even
/// try to spawn xurl.
#[test]
fn completions_works_without_xurl() {
    let env = common::TestEnv::new()
        .with_xurl_path(std::path::PathBuf::from("/tmp/nonexistent_xurl_12345"));
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty(), "completions bash must emit stdout");
}

/// `bird usage --local` must be accepted by clap even when xurl is missing
/// (the local-only path bypasses xurl). Asserts clap doesn't reject the flag
/// with exit 2 (the unrecognised-arg code).
#[test]
fn usage_local_flag_accepted_by_clap() {
    let env = common::TestEnv::new()
        .with_xurl_path(std::path::PathBuf::from("/tmp/nonexistent_xurl_12345"));
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "usage", "--local"], &env);
    assert_ne!(exit, 2, "--local must be accepted by clap");
}

/// xurl-missing config error must serialize as the canonical error envelope
/// when `--output json` is set. R22 keeps this assertion in-process because
/// the resolution result is per-transport state, not a global cache.
#[test]
fn output_json_command_error_schema_xurl_missing() {
    let env = common::TestEnv::new()
        .with_xurl_path(std::path::PathBuf::from("/tmp/nonexistent_xurl_12345"));
    let (exit, _stdout, stderr) = common::run_in_process(&["bird", "--output", "json", "me"], &env);
    assert_eq!(exit, 78);
    let json: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(json["kind"], "config");
    assert_eq!(json["exit_code"], 78);
}
