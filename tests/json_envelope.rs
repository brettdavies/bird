//! Library-style integration tests for the JSON error envelope and the
//! `--json` / `--jsonl` aliases.
//!
//! The envelope is `xr`'s: `{"status":"error","reason","exit_code","message"}`
//! plus a `next_step` when a recovery step exists. Clap parse failures route
//! through the same formatter via `Cli::try_parse()`.

mod common;

fn parse_envelope(stderr: &str) -> serde_json::Value {
    common::stderr_envelope(stderr)
}

#[test]
fn bad_flag_under_output_json_emits_envelope() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "--bogus-flag"], &env);
    assert_eq!(exit, 2, "usage errors exit 2");
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["status"], "error");
    assert_eq!(envv["reason"], "invalid-args");
    assert_eq!(envv["exit_code"], 2, "exit_code field should be 2");
    assert!(envv["message"].as_str().is_some(), "message field present");
    assert_eq!(envv["next_step"]["action"], "show-help");
    assert_eq!(envv["next_step"]["command"], "bird --help");
}

#[test]
fn bad_flag_under_json_alias_emits_envelope() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) = common::run_in_process(&["bird", "--json", "--bogus-flag"], &env);
    assert_eq!(exit, 2);
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["reason"], "invalid-args");
    assert_eq!(envv["exit_code"], 2);
}

#[test]
fn bad_flag_under_jsonl_alias_emits_envelope() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--jsonl", "--bogus-flag"], &env);
    assert_eq!(exit, 2);
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["reason"], "invalid-args");
    assert_eq!(envv["exit_code"], 2);
}

#[test]
fn config_error_emits_envelope() {
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
    assert_eq!(exit, 78, "config errors exit 78");
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["reason"], "config-error");
    assert_eq!(envv["exit_code"], 78);
    assert!(envv["message"].as_str().is_some());
}

#[test]
fn missing_required_argument_emits_envelope() {
    // `profile` requires a username positional arg.
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "profile"], &env);
    assert_eq!(exit, 2);
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["reason"], "invalid-args");
    assert_eq!(envv["exit_code"], 2);
    assert_eq!(envv["next_step"]["command"], "bird profile --help");
}

#[test]
fn unknown_command_echoes_the_word_and_suggests() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "thred", "1"], &env);
    assert_eq!(exit, 2);
    let envv = parse_envelope(&stderr);
    assert_eq!(envv["reason"], "unknown-command");
    assert_eq!(envv["command"], "thred");
    assert_eq!(envv["suggestion"], "thread");
    assert_eq!(envv["next_step"]["action"], "show-help");
}

#[test]
fn json_alias_matches_output_json_for_help() {
    // Help is plain text under every output mode, so `--json --help` and
    // `--output json --help` print the same bytes.
    let env_alias = common::TestEnv::new();
    let (exit_alias, stdout_alias, _) =
        common::run_in_process(&["bird", "--json", "--help"], &env_alias);
    assert_eq!(exit_alias, 0);

    let env_long = common::TestEnv::new();
    let (exit_long, stdout_long, _) =
        common::run_in_process(&["bird", "--output", "json", "--help"], &env_long);
    assert_eq!(exit_long, 0);

    assert_eq!(
        stdout_alias, stdout_long,
        "--json --help must produce the same stdout as --output json --help"
    );
    assert!(stdout_alias.contains("Usage: bird"));
}

#[test]
fn timeout_flag_accepted() {
    // --timeout is global; should be accepted on any subcommand.
    let env = common::TestEnv::new();
    let (exit, stdout, _) =
        common::run_in_process(&["bird", "--timeout", "5", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn verbose_flag_accepted() {
    // -v should be accepted globally.
    let env = common::TestEnv::new();
    let (exit_v, stdout_v, _) =
        common::run_in_process(&["bird", "-v", "completions", "bash"], &env);
    assert_eq!(exit_v, 0);
    assert!(!stdout_v.is_empty());
    let env2 = common::TestEnv::new();
    let (exit_vv, stdout_vv, _) =
        common::run_in_process(&["bird", "-vv", "completions", "bash"], &env2);
    assert_eq!(exit_vv, 0);
    assert!(!stdout_vv.is_empty());
}

#[test]
fn color_never_accepted() {
    for color in ["never", "always", "auto"] {
        let env = common::TestEnv::new();
        let (exit, stdout, _) =
            common::run_in_process(&["bird", "--color", color, "completions", "bash"], &env);
        assert_eq!(exit, 0, "--color {} should be accepted", color);
        assert!(!stdout.is_empty());
    }
}

#[test]
fn raw_flag_accepted() {
    // --raw should not fail parsing on any subcommand.
    let env = common::TestEnv::new();
    let (exit, stdout, _) = common::run_in_process(&["bird", "--raw", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn plain_alias_still_accepted() {
    // Deprecated --plain should still parse (it's an alias for --color never).
    let env = common::TestEnv::new();
    let (exit, stdout, _) =
        common::run_in_process(&["bird", "--plain", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn no_color_alias_still_accepted() {
    let env = common::TestEnv::new();
    let (exit, stdout, _) =
        common::run_in_process(&["bird", "--no-color", "completions", "bash"], &env);
    assert_eq!(exit, 0);
    assert!(!stdout.is_empty());
}

#[test]
fn envelope_error_id_is_kebab_case_for_unknown_arg() {
    let env = common::TestEnv::new();
    let (_exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "--bogus-flag"], &env);
    let envv = parse_envelope(&stderr);
    let id = envv["reason"].as_str().expect("test: reason is string");
    assert!(
        id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
        "reason should be kebab-case: {}",
        id
    );
}
