//! Library-style smoke tests.
//!
//! Every test that asserts only on exit code or on filesystem side effects
//! runs in-process via [`common::run_in_process`]. Tests that assert on
//! captured stdout/stderr content stay forked in
//! [`tests/cli_smoke_subprocess.rs`] until Plan 2 U11 strengthens the
//! in-process suite with stdout-content assertions against the
//! runner-injected writers.
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
    let (exit, _stdout, _stderr) = common::run_in_process(&["bird", "watchlist", "list"], &env);
    assert_eq!(exit, 0);
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
