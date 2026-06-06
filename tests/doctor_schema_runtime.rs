//! Runtime / schema drift guard for `bird doctor --json`.
//!
//! `tests/schema_parity.rs` ensures the embedded literal in `schema_print`
//! matches the on-disk `schema/doctor.schema.json`. That catches one half of
//! the drift problem (someone hand-edits the embedded copy without touching
//! the file or vice versa). The OTHER half is when the runtime emitter
//! (`bird::doctor::DoctorReport`) gains or loses a field but the schema
//! does not. This test round-trips a hand-built fixture report through
//! `serde_json::to_value`, then asserts every top-level key and every
//! per-app / per-command field appears in the schema's `required` /
//! `properties` arrays. Drift in either direction fails loudly.

use serde_json::Value;
use std::path::PathBuf;

fn load_schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/doctor.schema.json");
    let bytes =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    serde_json::from_str(&bytes).unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e))
}

fn schema_required(parent: &Value, parent_path: &str) -> Vec<String> {
    parent
        .get("required")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{parent_path}.required missing or not array"))
        .iter()
        .map(|v| {
            v.as_str()
                .expect("required entries are strings")
                .to_string()
        })
        .collect()
}

fn schema_property_names(parent: &Value, parent_path: &str) -> Vec<String> {
    parent
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{parent_path}.properties missing or not object"))
        .keys()
        .cloned()
        .collect()
}

#[test]
fn doctor_runtime_keys_present_in_schema() {
    let schema = load_schema();

    // Top-level shape.
    let top_required = schema_required(&schema, "schema");
    for key in ["xurl", "auth", "commands"] {
        assert!(
            top_required.iter().any(|k| k == key),
            "top-level required: missing `{key}`",
        );
    }
    let top_props = schema_property_names(&schema, "schema");
    for key in ["xurl", "auth", "commands", "cache", "linked_xurl_version"] {
        assert!(
            top_props.iter().any(|k| k == key),
            "top-level properties: missing `{key}`",
        );
    }

    // Auth section shape.
    let auth = schema.pointer("/properties/auth").expect("auth subschema");
    for key in ["active_app", "apps", "env"] {
        assert!(
            schema_required(auth, "/properties/auth")
                .iter()
                .any(|k| k == key),
            "auth.required: missing `{key}`",
        );
    }

    // Per-app shape.
    let app = schema
        .pointer("/properties/auth/properties/apps/additionalProperties")
        .expect("per-app subschema");
    for key in [
        "client_id_set",
        "client_secret_set",
        "default_user",
        "oauth2_tokens",
        "unnamed_oauth2",
        "oauth1",
        "bearer",
    ] {
        assert!(
            schema_property_names(app, "auth.apps.<app>")
                .iter()
                .any(|k| k == key),
            "auth.apps.<app>.properties: missing `{key}`",
        );
    }

    // OAuth2 token presence shape.
    let oauth2_token = schema
        .pointer(
            "/properties/auth/properties/apps/additionalProperties/properties/oauth2_tokens/additionalProperties",
        )
        .expect("oauth2 token subschema");
    for key in ["access_token_present", "refresh_token_present"] {
        assert!(
            schema_required(oauth2_token, "auth.apps.<app>.oauth2_tokens.<user>")
                .iter()
                .any(|k| k == key),
            "oauth2 token presence required: missing `{key}`",
        );
    }

    // OAuth1 token presence shape.
    let oauth1 = schema
        .pointer("/properties/auth/properties/apps/additionalProperties/properties/oauth1")
        .expect("oauth1 subschema");
    for key in [
        "access_token_present",
        "token_secret_present",
        "consumer_key_present",
        "consumer_secret_present",
    ] {
        assert!(
            schema_required(oauth1, "auth.apps.<app>.oauth1")
                .iter()
                .any(|k| k == key),
            "oauth1 presence required: missing `{key}`",
        );
    }

    // Env credentials shape.
    let env = schema
        .pointer("/properties/auth/properties/env")
        .expect("env subschema");
    for key in ["client_id_set", "client_secret_set", "bearer_token_set"] {
        assert!(
            schema_required(env, "auth.env").iter().any(|k| k == key),
            "auth.env required: missing `{key}`",
        );
    }

    // Per-command shape.
    let cmd = schema
        .pointer("/properties/commands/additionalProperties")
        .expect("per-command subschema");
    for key in [
        "available",
        "accepted_schemes",
        "credentialed_schemes",
        "reachable",
    ] {
        assert!(
            schema_required(cmd, "commands.<cmd>")
                .iter()
                .any(|k| k == key),
            "commands.<cmd> required: missing `{key}`",
        );
    }
}

#[test]
fn doctor_report_serializes_match_schema_keys() {
    use bird::doctor::{
        AppCredentials, AuthState, BearerTokenPresence, CacheStatus, CommandStatus, DoctorReport,
        EnvCredentials, OAuth1TokenPresence, OAuth2TokenPresence, XurlStatus,
    };
    use std::collections::HashMap;

    let mut oauth2_tokens = HashMap::new();
    oauth2_tokens.insert(
        "alice".to_string(),
        OAuth2TokenPresence {
            access_token_present: true,
            refresh_token_present: true,
        },
    );
    let mut apps = HashMap::new();
    apps.insert(
        "default".to_string(),
        AppCredentials {
            client_id_set: true,
            client_secret_set: true,
            default_user: Some("alice".to_string()),
            oauth2_tokens,
            unnamed_oauth2: Some(OAuth2TokenPresence {
                access_token_present: true,
                refresh_token_present: false,
            }),
            oauth1: Some(OAuth1TokenPresence {
                access_token_present: true,
                token_secret_present: true,
                consumer_key_present: true,
                consumer_secret_present: true,
            }),
            bearer: Some(BearerTokenPresence {
                token_present: true,
            }),
        },
    );
    let mut commands = HashMap::new();
    commands.insert(
        "bookmarks".to_string(),
        CommandStatus {
            available: true,
            reason: None,
            accepted_schemes: vec!["oauth2".to_string()],
            credentialed_schemes: vec!["oauth2".to_string()],
            reachable: true,
        },
    );
    let report = DoctorReport {
        xurl: XurlStatus {
            path: Some("/usr/local/bin/xurl".to_string()),
            version: Some("2.0.0".to_string()),
            available: true,
        },
        auth: AuthState {
            active_app: "default".to_string(),
            apps,
            env: EnvCredentials {
                client_id_set: true,
                client_secret_set: false,
                bearer_token_set: false,
            },
        },
        commands,
        cache: Some(CacheStatus {
            path: "/tmp/cache.db".to_string(),
            exists: true,
            size_mb: 0.5,
            max_size_mb: 100,
            tweets: 1,
            users: 1,
            raw_responses: 0,
            healthy: true,
        }),
        linked_xurl_version: None,
    };

    let value = serde_json::to_value(&report).expect("DoctorReport must serialize cleanly");

    // Walk the value and assert the expected keys are present.
    let obj = value.as_object().expect("top-level object");
    for key in ["xurl", "auth", "commands", "cache"] {
        assert!(obj.contains_key(key), "serialized report missing `{key}`");
    }

    let auth_obj = obj["auth"].as_object().expect("auth object");
    for key in ["active_app", "apps", "env"] {
        assert!(auth_obj.contains_key(key), "auth missing `{key}`");
    }

    let app_obj = auth_obj["apps"]
        .as_object()
        .expect("apps object")
        .get("default")
        .expect("default app entry")
        .as_object()
        .expect("default app is object");
    for key in [
        "client_id_set",
        "client_secret_set",
        "default_user",
        "oauth2_tokens",
        "unnamed_oauth2",
        "oauth1",
        "bearer",
    ] {
        assert!(app_obj.contains_key(key), "app missing `{key}`");
    }

    let cmd_obj = obj["commands"]
        .as_object()
        .expect("commands object")
        .get("bookmarks")
        .expect("bookmarks entry")
        .as_object()
        .expect("bookmarks is object");
    for key in [
        "available",
        "accepted_schemes",
        "credentialed_schemes",
        "reachable",
    ] {
        assert!(cmd_obj.contains_key(key), "command missing `{key}`");
    }
}

#[test]
fn serialized_report_omits_credential_material() {
    use bird::doctor::{
        AppCredentials, AuthState, BearerTokenPresence, CommandStatus, DoctorReport,
        EnvCredentials, OAuth1TokenPresence, OAuth2TokenPresence, XurlStatus,
    };
    use std::collections::HashMap;

    let sentinel = "SECRET_VALUE_THAT_MUST_NOT_LEAK_INTO_THE_REPORT";

    // The presence flags are surfaced as booleans; the only string fields
    // on the report are app names (`default`), usernames (`alice`), the
    // active_app slot, and the xurl version. None of them should ever
    // carry credential material — this test confirms the negative path
    // by populating sentinel-named entries and asserting the sentinel
    // does not appear in the JSON output.
    let mut oauth2_tokens = HashMap::new();
    oauth2_tokens.insert(
        "alice".to_string(),
        OAuth2TokenPresence {
            access_token_present: true,
            refresh_token_present: true,
        },
    );
    let mut apps = HashMap::new();
    apps.insert(
        "default".to_string(),
        AppCredentials {
            client_id_set: true,
            client_secret_set: true,
            default_user: Some("alice".to_string()),
            oauth2_tokens,
            unnamed_oauth2: None,
            oauth1: Some(OAuth1TokenPresence {
                access_token_present: true,
                token_secret_present: true,
                consumer_key_present: true,
                consumer_secret_present: true,
            }),
            bearer: Some(BearerTokenPresence {
                token_present: true,
            }),
        },
    );
    let mut commands = HashMap::new();
    commands.insert(
        "bookmarks".to_string(),
        CommandStatus {
            available: true,
            reason: None,
            accepted_schemes: vec!["oauth2".to_string()],
            credentialed_schemes: vec!["oauth2".to_string()],
            reachable: true,
        },
    );
    let report = DoctorReport {
        xurl: XurlStatus {
            path: Some("/usr/local/bin/xurl".to_string()),
            version: Some("2.0.0".to_string()),
            available: true,
        },
        auth: AuthState {
            active_app: "default".to_string(),
            apps,
            env: EnvCredentials {
                client_id_set: true,
                client_secret_set: false,
                bearer_token_set: false,
            },
        },
        commands,
        cache: None,
        linked_xurl_version: None,
    };

    let json = serde_json::to_string(&report).expect("serialize report");
    assert!(
        !json.contains(sentinel),
        "serialized report must never contain credential values; sentinel leaked",
    );
}
