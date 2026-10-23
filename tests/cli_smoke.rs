//! Library-style smoke tests.
//!
//! Every test runs in-process via [`common::run_in_process`]; both the exit
//! code and the captured stdout/stderr content are asserted where the
//! runner's writer-injection makes them observable.
//!
//! The `every_subcommand_help_has_example` and
//! `nested_subcommand_help_has_example` tests call clap's
//! [`clap::Command::write_help`] directly on the parsed [`bird::cli::Cli`]
//! command tree — no runner.

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
    // Empty config: an ok document with an empty list (the default output
    // mode in a non-TTY test environment is JSON).
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(
        doc,
        serde_json::json!({"status": "ok", "data": [], "meta": {}})
    );
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

/// `--no-browser` and its `--headless` alias reach the headless sign-in path.
/// Against the test's empty token store and environment it refuses the way
/// `xr auth oauth2` does, with `client-credentials-missing` and exit 2, before
/// any authorize URL is built: no pending state lands beside the store, and
/// stdin is never read.
#[test]
fn login_headless_refuses_an_app_without_client_credentials() {
    for flag in ["--no-browser", "--headless"] {
        let env = common::TestEnv::new();
        let (exit, _stdout, stderr) = common::run_in_process_with_stdin(
            &["bird", "login", flag, "--output", "json"],
            &env,
            &mut common::PanicOnRead,
        );
        assert_eq!(exit, 2, "{flag}: stderr {stderr:?}");
        let envelope = common::stderr_envelope(&stderr);
        assert_eq!(envelope["reason"], "client-credentials-missing", "{flag}");
        let store = env.paths.token_store.as_deref().expect("test: named store");
        let pending = xdk::auth::pending::pending_path_for_store(store);
        assert!(!pending.exists(), "{flag}: wrote {}", pending.display());
    }
}

/// `bird doctor` reads the token store the runner was handed, not the
/// machine's `~/.xurl`: an app seeded into the test's store is the only app
/// the report lists.
#[test]
fn doctor_reads_the_injected_token_store() {
    let env = common::TestEnv::new();
    let store_path = env
        .paths
        .token_store
        .as_deref()
        .and_then(|path| path.to_str())
        .expect("test: named utf-8 store path");
    xdk::store::TokenStore::new_with_path(store_path)
        .add_app("fixture-app", "fixture-client-id", "fixture-client-secret")
        .expect("test: seed the token store");

    let (exit, stdout, stderr) =
        common::run_in_process(&["bird", "doctor", "--output", "json"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let report: serde_json::Value = serde_json::from_str(stdout.trim()).expect("doctor JSON");
    let apps: Vec<&str> = report["auth"]["apps"]
        .as_object()
        .expect("auth.apps object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(apps, ["fixture-app"]);
    assert_eq!(report["auth"]["apps"]["fixture-app"]["client_id_set"], true);
    assert_eq!(report["auth"]["env"]["client_id_set"], false);
}

/// An app with client credentials and no token: a user-context command fails
/// offline with `auth-required` and a `sign-in` step naming `bird login`,
/// chosen from the store the run loaded.
#[test]
fn a_missing_token_points_at_bird_login() {
    let env = common::TestEnv::new();
    let store_path = env
        .paths
        .token_store
        .as_deref()
        .and_then(|path| path.to_str())
        .expect("test: named utf-8 store path");
    xdk::store::TokenStore::new_with_path(store_path)
        .add_app("fixture-app", "fixture-client-id", "fixture-client-secret")
        .expect("test: seed the token store");

    let (exit, _stdout, stderr) = common::run_in_process(
        &["bird", "--app", "fixture-app", "me", "--output", "json"],
        &env,
    );
    assert_eq!(exit, 77, "stderr {stderr:?}");
    let envelope = common::stderr_envelope(&stderr);
    assert_eq!(envelope["reason"], "auth-required");
    assert_eq!(envelope["next_step"]["action"], "sign-in");
    assert_eq!(envelope["next_step"]["command"], "bird login --no-browser");

    let (exit, _stdout, stderr) = common::run_in_process(
        &["bird", "--app", "fixture-app", "me", "--output", "text"],
        &env,
    );
    assert_eq!(exit, 77);
    assert!(
        stderr.contains("Sign in first. Run: bird login"),
        "text hint, got {stderr:?}"
    );
}

#[test]
fn cache_only_miss_is_a_cache_miss() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "--cache-only",
            "profile",
            "jack",
            "--output",
            "json",
        ],
        &env,
    );
    assert_eq!(exit, 1, "stderr {stderr:?}");
    let envelope = common::stderr_envelope(&stderr);
    assert_eq!(envelope["reason"], "cache-miss");
    assert_eq!(envelope["next_step"]["action"], "fix-input");
    assert_eq!(
        envelope["next_step"]["template"],
        "bird profile jack --output json"
    );
    let message = envelope["message"].as_str().expect("message");
    for words in [
        "--cache-only",
        "GET /2/users/by/username/jack",
        "today (UTC)",
        "this account",
        "only the UTC day it was written",
    ] {
        assert!(message.contains(words), "{words:?} missing: {message}");
    }
    assert!(!message.contains("entity"), "{message}");
}

#[test]
fn cache_only_miss_text_names_the_command_to_send() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(
        &["bird", "--output", "text", "--cache-only", "search", "rust"],
        &env,
    );
    assert_eq!(exit, 1, "stderr {stderr:?}");
    assert!(stderr.contains("GET /2/tweets/search/recent"), "{stderr}");
    assert!(
        stderr.contains("bird --output text search rust"),
        "{stderr}"
    );
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
fn a_write_without_force_or_tty_requires_confirmation() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "like", "123", "--output", "json"], &env);
    assert_eq!(exit, 1);
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["status"], "error");
    assert_eq!(json["reason"], "confirmation-required");
    assert_eq!(json["next_step"]["action"], "confirm");
}

#[test]
fn a_write_dry_run_emits_envelope_and_skips_request() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(
        &["bird", "like", "123", "--dry-run", "--output", "json"],
        &env,
    );
    assert_eq!(exit, 0);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["status"], "dry_run");
    assert_eq!(json["would_succeed"], true);
    assert_eq!(json["exit_code"], 0);
    assert_eq!(json["command"], "like");
    assert_eq!(json["method"], "POST");
    assert_eq!(json["confirmation_required"], true);
}

/// The raw HTTP verbs are gone: each is an unknown command before any request.
#[test]
fn raw_http_verbs_are_unknown_commands() {
    let env = common::TestEnv::new();
    let runs: &[&[&str]] = &[
        &["bird", "get", "/2/users/me", "--output", "json"],
        &["bird", "put", "/x", "--output", "json"],
        &[
            "bird",
            "post",
            "/2/tweets",
            "--body",
            "{}",
            "--output",
            "json",
        ],
        &["bird", "delete", "1585341984679469056", "--output", "json"],
    ];
    for argv in runs {
        let (exit, stdout, stderr) = common::run_in_process(argv, &env);
        assert_eq!(exit, 2, "{argv:?}");
        assert!(stdout.is_empty(), "{argv:?}: {stdout:?}");
        let json = common::stderr_envelope(&stderr);
        assert_eq!(json["reason"], "unknown-command", "{argv:?}");
        assert_eq!(json["command"], argv[1], "{argv:?}");
    }
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
    assert_eq!(json["status"], "dry_run");
    assert_eq!(json["body"]["text"], "hi there");
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
    assert_eq!(json["status"], "dry_run");
    assert_eq!(json["command"], "clear");
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
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["status"], "error");
    assert_eq!(json["reason"], "config-error");
    assert_eq!(json["exit_code"], 78);
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
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["reason"], "config-error");
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
        "bookmarks",
        "profile",
        "search",
        "thread",
        "watchlist",
        "spend",
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
        ("watchlist", "fetch"),
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

// --- Local-only path smokes (no API call required) ------------------------

/// `bird completions <shell>` short-circuits before any xurl construction.
#[test]
fn completions_works_without_xurl() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty(), "completions bash must emit stdout");
}

/// `bird spend` reads the local ledger with no request; `bird usage` names no
/// command until stack 3 composes `xr`'s.
#[test]
fn spend_reads_the_local_ledger_and_usage_is_gone() {
    let env = common::TestEnv::new();
    let (exit, stdout, stderr) =
        common::run_in_process(&["bird", "spend", "--output", "json"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("spend JSON");
    assert_eq!(doc["status"], "ok");
    assert_eq!(doc["sync_status"], "skipped");

    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "usage", "--output", "json"], &env);
    assert_eq!(exit, 2);
    assert_eq!(
        common::stderr_envelope(&stderr)["reason"],
        "unknown-command"
    );
}

// --- Skill install destinations ------------------------------------------

/// `~` destinations expand against the `HOME` the runner was handed, so an
/// in-process run never resolves into the real home directory.
#[test]
fn skill_install_expands_against_the_injected_home() {
    let mut env = common::TestEnv::new();
    let home = tempfile::TempDir::new().expect("test: home tempdir");
    env.env.xdk.home = Some(home.path().display().to_string());

    let (exit, stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "skill",
            "install",
            "claude_code",
            "--dry-run",
            "--output",
            "json",
        ],
        &env,
    );

    assert_eq!(exit, 0, "stderr {stderr:?}");
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("install envelope JSON");
    let expected = home.path().join(".claude/skills/bird");
    assert_eq!(envelope["install_dir"], expected.display().to_string());
}

/// With no `HOME` in the injected environment the installer reports
/// `home-not-set`, even though the test process itself has one.
#[test]
fn skill_install_ignores_the_process_home() {
    let env = common::TestEnv::new();

    let (exit, stdout, _) = common::run_in_process(
        &[
            "bird",
            "skill",
            "install",
            "claude_code",
            "--dry-run",
            "--output",
            "json",
        ],
        &env,
    );

    assert_eq!(exit, 1, "stdout {stdout:?}");
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("install envelope JSON");
    assert_eq!(envelope["reason"], "home-not-set");
}

/// The envelope `bird skill <verb> <host> --dry-run --output json` prints
/// against `env`, with its exit code.
fn skill_dry_run(env: &common::TestEnv, verb: &str, host: &str) -> (u8, serde_json::Value) {
    let (exit, stdout, stderr) = common::run_in_process(
        &["bird", "skill", verb, host, "--dry-run", "--output", "json"],
        env,
    );
    let envelope = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("skill {verb} envelope JSON: {e}: {stdout:?} {stderr:?}"));
    (exit, envelope)
}

/// Sets `var` to `value` among the host directory variables `env` captured.
fn with_skill_dir(env: &mut common::TestEnv, var: &str, value: &std::path::Path) {
    env.env
        .skill_dirs
        .insert(var.to_string(), value.display().to_string());
}

#[test]
fn claude_config_dir_relocates_the_claude_code_destination() {
    let mut env = common::TestEnv::new();
    let config = tempfile::TempDir::new().expect("test: config tempdir");
    with_skill_dir(&mut env, "CLAUDE_CONFIG_DIR", config.path());

    for verb in ["install", "update"] {
        let (exit, envelope) = skill_dry_run(&env, verb, "claude_code");
        assert_eq!(exit, 0, "{verb}: {envelope}");
        assert_eq!(
            envelope["install_dir"],
            config.path().join("skills/bird").display().to_string(),
            "{verb}"
        );
    }
}

#[test]
fn bird_skill_home_stands_in_for_home_in_the_codex_destination() {
    let mut env = common::TestEnv::new();
    let skill_home = tempfile::TempDir::new().expect("test: skill home tempdir");
    env.env.skill_home = Some(skill_home.path().display().to_string());

    let (exit, envelope) = skill_dry_run(&env, "install", "codex");
    assert_eq!(exit, 0, "{envelope}");
    assert_eq!(
        envelope["install_dir"],
        skill_home
            .path()
            .join(".agents/skills/bird")
            .display()
            .to_string()
    );
}

#[test]
fn xdg_config_home_moves_the_opencode_destination() {
    let mut env = common::TestEnv::new();
    let config = tempfile::TempDir::new().expect("test: XDG tempdir");
    with_skill_dir(&mut env, "XDG_CONFIG_HOME", config.path());

    let (exit, envelope) = skill_dry_run(&env, "install", "opencode");
    assert_eq!(exit, 0, "{envelope}");
    assert_eq!(
        envelope["install_dir"],
        config
            .path()
            .join("opencode/skills/bird")
            .display()
            .to_string()
    );
}

/// For opencode, its own config-dir variable wins over `BIRD_SKILL_HOME`,
/// which wins over `XDG_CONFIG_HOME`, which wins over `HOME`.
#[test]
fn opencode_destination_variables_resolve_in_xr_order() {
    let home = tempfile::TempDir::new().expect("test: home tempdir");
    let skill_home = tempfile::TempDir::new().expect("test: skill home tempdir");
    let xdg = tempfile::TempDir::new().expect("test: XDG tempdir");
    let opencode = tempfile::TempDir::new().expect("test: opencode tempdir");

    let mut env = common::TestEnv::new();
    env.env.xdk.home = Some(home.path().display().to_string());
    env.env.skill_home = Some(skill_home.path().display().to_string());
    with_skill_dir(&mut env, "XDG_CONFIG_HOME", xdg.path());
    with_skill_dir(&mut env, "OPENCODE_CONFIG_DIR", opencode.path());

    let planned =
        |env: &common::TestEnv| skill_dry_run(env, "install", "opencode").1["install_dir"].clone();

    assert_eq!(
        planned(&env),
        opencode.path().join("skills/bird").display().to_string()
    );
    env.env.skill_dirs.remove("OPENCODE_CONFIG_DIR");
    assert_eq!(
        planned(&env),
        skill_home
            .path()
            .join(".config/opencode/skills/bird")
            .display()
            .to_string()
    );
    env.env.skill_home = None;
    assert_eq!(
        planned(&env),
        xdg.path()
            .join("opencode/skills/bird")
            .display()
            .to_string()
    );
    env.env.skill_dirs.remove("XDG_CONFIG_HOME");
    assert_eq!(
        planned(&env),
        home.path()
            .join(".config/opencode/skills/bird")
            .display()
            .to_string()
    );
}

#[test]
fn a_codex_install_reports_the_legacy_copy() {
    let mut env = common::TestEnv::new();
    let home = tempfile::TempDir::new().expect("test: home tempdir");
    env.env.xdk.home = Some(home.path().display().to_string());
    let legacy = home.path().join(".codex/skills/bird");
    std::fs::create_dir_all(&legacy).expect("test: create the legacy copy");

    for verb in ["install", "update"] {
        let (exit, envelope) = skill_dry_run(&env, verb, "codex");
        assert_eq!(exit, 0, "{verb}: {envelope}");
        assert_eq!(
            envelope["install_dir"],
            home.path()
                .join(".agents/skills/bird")
                .display()
                .to_string(),
            "{verb}"
        );
        assert_eq!(
            envelope["legacy_install_dir"],
            legacy.display().to_string(),
            "{verb}"
        );
    }
    assert!(legacy.exists(), "a dry run leaves the legacy copy in place");
}

/// `skill update --all` refreshes a host holding a copy, at its destination
/// or its legacy location, and reports every other host as skipped, all in one
/// envelope.
#[test]
fn update_all_refreshes_installed_hosts_and_skips_the_rest() {
    let mut env = common::TestEnv::new();
    let home = tempfile::TempDir::new().expect("test: home tempdir");
    env.env.xdk.home = Some(home.path().display().to_string());
    for copy in [".claude/skills/bird", ".codex/skills/bird"] {
        std::fs::create_dir_all(home.path().join(copy)).expect("test: create a copy");
    }

    let (exit, stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "skill",
            "update",
            "--all",
            "--dry-run",
            "--output",
            "json",
        ],
        &env,
    );

    assert_eq!(exit, 0, "stdout {stdout:?} stderr {stderr:?}");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("one update envelope: {e}: {stdout:?}"));
    assert_eq!(envelope["action"], "skill-update");
    assert_eq!(envelope["status"], "dry_run");
    assert_eq!(envelope["exit_code"], 0);
    let installations = envelope["installations"]
        .as_array()
        .expect("installations array");
    assert_eq!(installations.len(), bird::skill_install::KNOWN_HOSTS.len());
    for installation in installations {
        let host = installation["host"].as_str().expect("host string");
        if matches!(host, "claude_code" | "codex") {
            assert_eq!(installation["status"], "dry_run", "{installation}");
            assert_eq!(installation["would_succeed"], true, "{installation}");
        } else {
            assert_eq!(installation["status"], "skipped", "{installation}");
            assert_eq!(installation["reason"], "not-installed", "{installation}");
            assert_eq!(installation["exit_code"], 0, "{installation}");
            assert_eq!(installation["destination_status"], "absent");
            assert_eq!(installation["next_step"]["action"], "run-command");
            assert_eq!(
                installation["next_step"]["command"],
                format!("bird skill install {host}")
            );
        }
    }
}

/// Text mode names each skipped host's destination without calling it an
/// error.
#[test]
fn update_all_text_reports_skipped_hosts() {
    let mut env = common::TestEnv::new();
    let home = tempfile::TempDir::new().expect("test: home tempdir");
    env.env.xdk.home = Some(home.path().display().to_string());

    let (exit, stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "skill",
            "update",
            "--all",
            "--dry-run",
            "--output",
            "text",
        ],
        &env,
    );

    assert_eq!(exit, 0, "stderr {stderr:?}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        bird::skill_install::KNOWN_HOSTS.len(),
        "{stdout}"
    );
    let cursor = home.path().join(".cursor/skills/bird");
    assert!(
        lines.contains(&format!("skipped: not-installed: {}", cursor.display()).as_str()),
        "{stdout}"
    );
    assert!(!stdout.contains("error"), "{stdout}");
}

/// With no skill variable and no `HOME` in the injected environment every
/// host reports `home-not-set`, whatever the test process itself has set.
#[test]
fn no_skill_variable_and_no_home_is_home_not_set_for_every_host() {
    let env = common::TestEnv::new();
    let (exit, stdout, _) = common::run_in_process(
        &[
            "bird",
            "skill",
            "install",
            "--all",
            "--dry-run",
            "--output",
            "json",
        ],
        &env,
    );

    assert_eq!(exit, 1, "stdout {stdout:?}");
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("install envelope JSON");
    let installations = envelope["installations"]
        .as_array()
        .expect("installations array");
    assert_eq!(installations.len(), bird::skill_install::KNOWN_HOSTS.len());
    for installation in installations {
        assert_eq!(installation["reason"], "home-not-set", "{installation}");
        assert_eq!(installation["next_step"]["action"], "fix-input");
    }
}

// --- File store ---------------------------------------------------------------

/// A `--dry-run` write sends nothing, so it leaves the account's stored
/// responses in place.
#[test]
fn a_dry_run_clears_nothing() {
    let env = common::TestEnv::new();
    let responses = env.paths.cache_dir.join("bird").join("responses");
    let mut stored = Vec::new();
    for app in ["", "default"] {
        let account = bird::store::responses::Account {
            app: app.into(),
            user: "alice".into(),
        };
        let dir = responses.join(bird::store::responses::account_key(&account));
        std::fs::create_dir_all(&dir).expect("account dir");
        let file = dir.join(format!("{}.json", "0".repeat(64)));
        std::fs::write(&file, b"{}").expect("response file");
        stored.push(file);
    }
    let (exit, stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "-u",
            "alice",
            "--output",
            "json",
            "like",
            "1849",
            "--dry-run",
        ],
        &env,
    );
    assert_eq!(exit, 0, "stdout {stdout:?} stderr {stderr:?}");
    for file in stored {
        assert!(file.exists(), "{} was removed", file.display());
    }
}

/// Seeds one note, one ledger line, and one response file; returns their
/// paths.
fn seed_store(
    env: &common::TestEnv,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let data = env.paths.data_dir.join("bird");
    let note = data.join("archive").join("posts").join("1849.md");
    let ledger = data.join("spend.jsonl");
    let response = env
        .paths
        .cache_dir
        .join("bird")
        .join("responses")
        .join("0123456789abcdef")
        .join(format!("{}.json", "a".repeat(64)));
    for (path, bytes) in [
        (&note, &b"---\ntype: \"x-post\"\n---\n"[..]),
        (&ledger, &b"{}\n"[..]),
        (&response, &b"{}"[..]),
    ] {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, bytes).expect("seed");
    }
    (note, ledger, response)
}

#[test]
fn clear_keeps_the_archive_and_ledger() {
    let env = common::TestEnv::new();
    let (note, ledger, response) = seed_store(&env);
    let (exit, stdout, stderr) = common::run_in_process(
        &["bird", "--output", "json", "cache", "clear", "--force"],
        &env,
    );
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("ok document");
    let message = doc["message"].as_str().expect("message");
    assert!(message.contains("Cleared 1 stored responses"), "{message}");
    assert!(!response.exists(), "the response file must go");
    assert!(note.exists(), "the archive must stay");
    assert!(ledger.exists(), "the ledger must stay");
}

#[test]
fn clear_without_force_asks_first() {
    let env = common::TestEnv::new();
    let (_, _, response) = seed_store(&env);
    let (exit, _stdout, stderr) = common::run_in_process(
        &[
            "bird",
            "--no-interactive",
            "--output",
            "json",
            "cache",
            "clear",
        ],
        &env,
    );
    assert_eq!(exit, 1);
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["reason"], "confirmation-required");
    assert!(response.exists(), "nothing is removed without confirmation");
}

#[test]
fn stats_report_the_three_locations() {
    let env = common::TestEnv::new();
    seed_store(&env);
    let (exit, stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "cache", "stats"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("stats");
    assert_eq!(doc["status"], "ok");
    assert_eq!(doc["archive"]["posts"], 1);
    assert_eq!(doc["archive"]["source"], "default");
    assert_eq!(doc["responses"]["files"], 1);
    assert_eq!(doc["ledger"]["exists"], true);
    assert_eq!(doc["healthy"], true);
    for retired in ["max_size_mb", "raw_responses", "schema_version"] {
        assert!(doc.get(retired).is_none(), "{retired} must be gone: {doc}");
    }
}

/// An unhealthy store names each location bird cannot write, the directory
/// the check found not writable, and the fix for it.
#[cfg(unix)]
#[test]
fn unhealthy_stats_name_each_failing_location_and_its_fix() {
    use std::os::unix::fs::PermissionsExt;
    let env = common::TestEnv::new();
    let bird_data = env.paths.data_dir.join("bird");
    std::fs::create_dir_all(&bird_data).expect("mkdir");
    std::fs::set_permissions(&bird_data, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    let run = |args: &[&str]| common::run_in_process(args, &env);
    let (exit, stats, stderr) = run(&["bird", "--output", "text", "cache", "stats"]);
    let (_, doctor, _) = run(&["bird", "--output", "text", "doctor"]);
    std::fs::set_permissions(&bird_data, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    assert_eq!(exit, 0, "stderr {stderr:?}");

    let parent = bird_data.display().to_string();
    let archive = bird_data.join("archive").display().to_string();
    let ledger = format!("ledger {parent}");
    for (name, text) in [("cache stats", &stats), ("doctor", &doctor)] {
        assert!(text.contains("unhealthy"), "{name}: {text}");
        let line = |needle: &str| {
            text.lines()
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{name}: no line with {needle:?}: {text}"))
        };
        let archive_line = line(&format!("archive {archive}"));
        assert!(archive_line.contains(&parent), "{archive_line}");
        assert!(archive_line.contains("BIRD_ARCHIVE_DIR"), "{archive_line}");
        assert!(line(&ledger).contains("permission"), "{text}");
        assert!(
            !text.contains("responses /"),
            "{name}: responses is writable: {text}"
        );
    }
}

fn object_keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().expect("object").keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn doctor_and_stats_share_the_cache_shape() {
    let env = common::TestEnv::new();
    let (_, stats, _) =
        common::run_in_process(&["bird", "--output", "json", "cache", "stats"], &env);
    let (_, doctor, _) = common::run_in_process(&["bird", "--output", "json", "doctor"], &env);
    let mut stats: serde_json::Value = serde_json::from_str(stats.trim()).expect("stats");
    let doctor: serde_json::Value = serde_json::from_str(doctor.trim()).expect("doctor");
    stats.as_object_mut().expect("object").remove("status");
    assert_eq!(object_keys(&doctor["cache"]), object_keys(&stats));
    for location in ["archive", "responses", "ledger"] {
        assert_eq!(
            object_keys(&doctor["cache"][location]),
            object_keys(&stats[location]),
            "{location}"
        );
    }
}

#[test]
fn doctor_names_xdk() {
    let env = common::TestEnv::new();
    let (exit, stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "doctor"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("doctor");
    assert!(doc.get("xdk").is_some(), "{doc}");
    assert!(doc.get("linked_xdk_version").is_some(), "{doc}");
    assert!(doc.get("xurl").is_none(), "{doc}");
    assert!(doc.get("linked_xurl_version").is_none(), "{doc}");
    assert!(!stdout.contains("bird.db"), "{stdout}");
}

/// A scope word doctor does not report on is refused, so an agent that
/// mistypes a command name does not read the full report as success.
#[test]
fn doctor_refuses_a_command_it_does_not_report() {
    let env = common::TestEnv::new();
    let (exit, stdout, stderr) = common::run_in_process(
        &["bird", "--output", "json", "doctor", "nosuchcommand"],
        &env,
    );
    assert_eq!(exit, 2, "stderr {stderr:?}");
    assert!(stdout.is_empty(), "no report on stdout: {stdout}");
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["reason"], "unknown-command");
    assert_eq!(json["exit_code"], 2);
    assert_eq!(json["command"], "nosuchcommand");
    assert_eq!(json["next_step"]["action"], "show-help");
    assert_eq!(json["next_step"]["command"], "bird doctor --help");
    let message = json["message"].as_str().expect("message");
    let (_, reported, _) = common::run_in_process(&["bird", "--output", "json", "doctor"], &env);
    let reported: serde_json::Value = serde_json::from_str(reported.trim()).expect("doctor");
    for name in reported["commands"].as_object().expect("commands").keys() {
        assert!(
            message.contains(name.as_str()),
            "{name} unlisted: {message}"
        );
    }
}

#[test]
fn doctor_scoped_to_a_reported_command_succeeds() {
    let env = common::TestEnv::new();
    let (exit, stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "doctor", "me"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("doctor");
    let commands = doc["commands"].as_object().expect("commands");
    assert_eq!(commands.keys().collect::<Vec<_>>(), ["me"]);
}

/// bird's own reports print as text in text mode, the same text `--pretty`
/// prints, and as one JSON document under `--output json`.
#[test]
fn reports_print_text_in_text_mode() {
    let env = common::TestEnv::new();
    let cases: [(&[&str], &str); 3] = [
        (&["doctor"], "X API client"),
        (&["cache", "stats"], "Archive:"),
        (&["spend"], "API Usage ("),
    ];
    for (args, heading) in cases {
        let run = |output: &str, pretty: bool| {
            let mut argv = vec!["bird", "--color", "never", "--output", output];
            argv.extend_from_slice(args);
            if pretty {
                argv.push("--pretty");
            }
            common::run_in_process(&argv, &env)
        };
        let (exit, text, stderr) = run("text", false);
        assert_eq!(exit, 0, "{args:?} stderr {stderr:?}");
        assert!(text.starts_with(heading), "{args:?} text mode: {text}");
        assert!(!text.trim_start().starts_with('{'), "{args:?}: {text}");

        let (_, pretty, _) = run("text", true);
        assert_eq!(
            text, pretty,
            "{args:?}: text mode prints what --pretty prints"
        );

        let (exit, json, stderr) = run("json", false);
        assert_eq!(exit, 0, "{args:?} stderr {stderr:?}");
        let doc: serde_json::Value = serde_json::from_str(json.trim()).expect("one document");
        assert_eq!(doc["status"], "ok", "{args:?}: {doc}");
    }
}

// --- The 0.2.0 store ------------------------------------------------------------

fn seed_legacy(env: &common::TestEnv, names: &[&str]) {
    for name in names {
        std::fs::write(env.paths.config_dir.join(name), b"SQLite format 3").expect("seed");
    }
}

fn removal_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| l.starts_with("[store] removed the 0.2.0 store at "))
        .collect()
}

#[test]
fn the_first_run_removes_bird_db_and_its_sidecars() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db", "bird.db-wal", "bird.db-shm"]);
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    for name in ["bird.db", "bird.db-wal", "bird.db-shm"] {
        assert!(!env.paths.config_dir.join(name).exists(), "{name} remains");
    }
    let lines = removal_lines(&stderr);
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(lines[0].ends_with("bird.db"), "{stderr}");
}

#[test]
fn a_0_2_0_config_dir_is_left_with_no_database() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db", "bird.db-wal", "bird.db-shm", "cache.db"]);
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    assert_eq!(exit, 0, "stderr {stderr:?}");
    for name in ["bird.db", "bird.db-wal", "bird.db-shm", "cache.db"] {
        assert!(!env.paths.config_dir.join(name).exists(), "{name} remains");
    }
    assert_eq!(
        removal_lines(&stderr).len(),
        2,
        "one line per store: {stderr}"
    );
}

#[test]
fn the_second_run_says_nothing() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db"]);
    common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    assert_eq!(exit, 0);
    assert!(removal_lines(&stderr).is_empty(), "{stderr}");
}

#[test]
fn removal_ignores_cache_flags() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db"]);
    common::run_in_process(&["bird", "--no-cache", "--output", "text", "doctor"], &env);
    assert!(!env.paths.config_dir.join("bird.db").exists());
}

#[test]
fn quiet_removal_prints_nothing() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db"]);
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "-q", "--output", "text", "doctor"], &env);
    assert_eq!(exit, 0);
    assert!(!env.paths.config_dir.join("bird.db").exists());
    assert!(removal_lines(&stderr).is_empty(), "{stderr}");
}

#[test]
fn an_older_cache_db_is_removed_too() {
    let env = common::TestEnv::new();
    seed_legacy(&env, &["cache.db"]);
    let (_, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    assert!(!env.paths.config_dir.join("cache.db").exists());
    let lines = removal_lines(&stderr);
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(lines[0].ends_with("cache.db"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn a_failed_removal_leaves_the_exit_code_alone() {
    use std::os::unix::fs::PermissionsExt;
    let env = common::TestEnv::new();
    seed_legacy(&env, &["bird.db"]);
    let dir = &env.paths.config_dir;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "text", "doctor"], &env);
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).expect("chmod back");
    assert_eq!(exit, 0, "stderr {stderr:?}");
    assert!(dir.join("bird.db").exists());
    assert!(removal_lines(&stderr).is_empty(), "{stderr}");
}

// --- Store locations --------------------------------------------------------

/// A relative `archive_dir` would scatter notes across working directories,
/// so it is refused naming both settings that move the archive.
#[test]
fn relative_archive_dir_is_a_config_error() {
    let mut env = common::TestEnv::new();
    env.env.archive_dir = Some("notes/x".into());
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "doctor"], &env);
    assert_eq!(exit, 78, "stderr {stderr:?}");
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["reason"], "config-error");
    assert_eq!(json["command"], "notes/x");
    assert_eq!(json["next_step"]["action"], "fix-input");
    let message = json["message"].as_str().expect("message");
    assert!(message.contains("archive_dir"), "{message}");
    assert!(message.contains("BIRD_ARCHIVE_DIR"), "{message}");
}

/// A `config.toml` that does not parse is refused rather than replaced by the
/// default config, which would send notes to the default folder silently.
#[test]
fn malformed_config_toml_is_a_config_error() {
    let env = common::TestEnv::new();
    std::fs::write(
        env.paths.config_dir.join("config.toml"),
        "archive_dir = \"/v\n",
    )
    .expect("write config");
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "doctor"], &env);
    assert_eq!(exit, 78, "stderr {stderr:?}");
    let json = common::stderr_envelope(&stderr);
    assert_eq!(json["reason"], "config-error");
    let message = json["message"].as_str().expect("message");
    assert!(message.contains("config.toml"), "{message}");
}
