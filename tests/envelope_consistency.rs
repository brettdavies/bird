//! Envelope consistency tests (p2-should-consistent-envelope).
//!
//! Per anc's audit: non-payload keys present in the success envelope must also
//! appear in the error envelope. bird's error envelope is `xr`'s:
//! `{"status":"error","reason","exit_code","message"}` plus optional fields.

mod common;

#[test]
fn success_envelope_has_data_and_meta_keys() {
    // `--examples` under `--output json` emits a success envelope without
    // touching disk state, config, or sqlite.
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "--examples"], &env);
    assert_eq!(exit, 0);

    let json: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("test: examples envelope JSON parse");
    let obj = json
        .as_object()
        .expect("test: examples envelope top-level is object");
    assert!(obj.contains_key("data"), "success envelope must have data");
    assert!(obj.contains_key("meta"), "success envelope must have meta");
}

#[test]
fn error_envelope_has_required_keys() {
    let env = common::TestEnv::new();
    let (exit, _stdout, stderr) =
        common::run_in_process(&["bird", "--output", "json", "--bogus-flag"], &env);
    assert_eq!(exit, 2);

    let json = common::stderr_envelope(&stderr);
    let obj = json
        .as_object()
        .expect("test: error envelope top-level is object");
    for key in &["status", "reason", "message", "exit_code", "next_step"] {
        assert!(obj.contains_key(*key), "error envelope must have {}", key);
    }
    assert_eq!(obj["status"], "error");
    assert_eq!(obj["reason"], "invalid-args");
    for retired in &["error", "kind", "meta"] {
        assert!(!obj.contains_key(*retired), "{retired} is not a key");
    }
}

/// Help asked for as JSON takes the ok envelope, so a success document and an
/// error document share the `status` key; help asked for any other way is
/// plain text.
#[test]
fn help_under_explicit_json_shares_the_status_key_with_errors() {
    let env = common::TestEnv::new();
    let (exit, stdout, _stderr) =
        common::run_in_process(&["bird", "--output", "json", "--help"], &env);
    assert_eq!(exit, 0);
    let help: serde_json::Value = serde_json::from_str(stdout.trim()).expect("help envelope");
    assert_eq!(help["status"], "ok");
    assert!(
        help["data"]["help"]
            .as_str()
            .is_some_and(|h| h.contains("Usage: bird"))
    );

    let (_, _, stderr) =
        common::run_in_process(&["bird", "--output", "json", "--bogus-flag"], &env);
    assert_eq!(common::stderr_envelope(&stderr)["status"], "error");

    let (exit, stdout, _stderr) = common::run_in_process(&["bird", "--help"], &env);
    assert_eq!(exit, 0);
    assert!(
        stdout.starts_with("X API CLI"),
        "plain help, got {stdout:?}"
    );
}

/// Every bird-owned command that runs offline prints one document whose
/// `status` is `ok` or `dry_run` under `--output json`.
#[test]
fn bird_owned_commands_print_a_status_document() {
    let env = common::TestEnv::new();
    let runs: &[(&[&str], &str)] = &[
        (&["bird", "--output", "json", "--examples"], "ok"),
        (&["bird", "--output", "json", "cache", "stats"], "ok"),
        (
            &["bird", "--output", "json", "cache", "clear", "--dry-run"],
            "dry_run",
        ),
        (
            &["bird", "--output", "json", "cache", "clear", "--force"],
            "ok",
        ),
        (
            &["bird", "--output", "json", "watchlist", "add", "alice"],
            "ok",
        ),
        (&["bird", "--output", "json", "watchlist", "list"], "ok"),
        (
            &[
                "bird",
                "--output",
                "json",
                "watchlist",
                "remove",
                "alice",
                "--yes",
            ],
            "ok",
        ),
        (&["bird", "--output", "json", "watchlist", "fetch"], "ok"),
        (&["bird", "--output", "json", "doctor"], "ok"),
        (&["bird", "--output", "json", "usage", "--local"], "ok"),
        (
            &["bird", "--output", "json", "like", "1", "--dry-run"],
            "dry_run",
        ),
    ];
    for (argv, status) in runs {
        let (exit, stdout, stderr) = common::run_in_process(argv, &env);
        assert_eq!(exit, 0, "{argv:?}: stderr {stderr:?}");
        let doc: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("{argv:?}: one JSON document, got {stdout:?}: {e}"));
        assert_eq!(doc["status"], *status, "{argv:?}: {doc}");
    }
}
