//! Each bird-owned command's `--output json` document matches its committed
//! schema: every top-level key it prints is declared, and every required key
//! is present. Runs offline against a temporary store.

mod common;

fn schema(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schema")
        .join(format!("{name}.schema.json"));
    let body = std::fs::read_to_string(&path).expect("test: read schema");
    serde_json::from_str(&body).expect("test: schema is JSON")
}

fn assert_matches(name: &str, doc: &serde_json::Value) {
    let schema = schema(name);
    let declared = schema["properties"].as_object().expect("test: properties");
    let obj = doc.as_object().expect("test: document is an object");
    for key in obj.keys() {
        assert!(
            declared.contains_key(key),
            "{name}: {key} undeclared in {doc}"
        );
    }
    for key in schema["required"].as_array().into_iter().flatten() {
        let key = key.as_str().expect("test: required key is a string");
        assert!(
            obj.contains_key(key),
            "{name}: required {key} missing in {doc}"
        );
    }
}

#[test]
fn offline_commands_match_their_schemas() {
    let env = common::TestEnv::new();
    common::run_in_process(&["bird", "watchlist", "add", "alice"], &env);
    let runs: &[(&str, &[&str])] = &[
        (
            "cache-stats",
            &["bird", "--output", "json", "cache", "stats"],
        ),
        (
            "watchlist-list",
            &["bird", "--output", "json", "watchlist", "list"],
        ),
        (
            "watchlist-fetch",
            &[
                "bird",
                "--output",
                "json",
                "--cache-only",
                "watchlist",
                "fetch",
            ],
        ),
        ("usage", &["bird", "--output", "json", "usage", "--local"]),
        ("doctor", &["bird", "--output", "json", "doctor"]),
    ];
    for (name, argv) in runs {
        let (exit, stdout, stderr) = common::run_in_process(argv, &env);
        assert_eq!(exit, 0, "{name}: stderr {stderr:?}");
        let doc: serde_json::Value =
            serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{name}: {e}: {stdout}"));
        assert_matches(name, &doc);
    }
}

#[test]
fn the_skill_dry_run_matches_its_schema() {
    let env = common::TestEnv::new();
    let (_exit, stdout, _stderr) = common::run_in_process(
        &[
            "bird",
            "--output",
            "json",
            "skill",
            "install",
            "--all",
            "--dry-run",
        ],
        &env,
    );
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("test: skill JSON");
    assert_matches("skill-install", &doc);
}
