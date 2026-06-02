//! `bird schema` subcommand: runtime-discoverable JSON Schemas for bird's
//! output shapes.
//!
//! Each schema document at `schema/*.schema.json` in the repo root is embedded
//! at build time via `include_str!`. Agents and downstream tooling can call
//! `bird schema --list` to enumerate names and `bird schema <name>` to fetch
//! the schema bytes — no network or filesystem lookup required at runtime.
//!
//! A parity test in `tests/schema_parity.rs` enforces that embedded bytes
//! equal on-disk bytes; the on-disk files remain the single source of truth.

use crate::error::BirdError;
use crate::output::{OutputConfig, success_envelope_string};

/// Single schema entry: stable name (used in URLs and on the CLI) paired with
/// the embedded document bytes.
pub(crate) struct SchemaEntry {
    pub name: &'static str,
    pub body: &'static str,
}

/// Table of all embedded schemas. Names match `schema/<name>.schema.json` on
/// disk; the parity test asserts byte equality. Order is sorted for
/// deterministic `--list` output.
pub(crate) const SCHEMAS: &[SchemaEntry] = &[
    SchemaEntry {
        name: "bookmarks",
        body: include_str!("../schema/bookmarks.schema.json"),
    },
    SchemaEntry {
        name: "doctor",
        body: include_str!("../schema/doctor.schema.json"),
    },
    SchemaEntry {
        name: "error-envelope",
        body: include_str!("../schema/error-envelope.schema.json"),
    },
    SchemaEntry {
        name: "profile",
        body: include_str!("../schema/profile.schema.json"),
    },
    SchemaEntry {
        name: "raw-get",
        body: include_str!("../schema/raw-get.schema.json"),
    },
    SchemaEntry {
        name: "search",
        body: include_str!("../schema/search.schema.json"),
    },
    SchemaEntry {
        name: "success-envelope",
        body: include_str!("../schema/success-envelope.schema.json"),
    },
    SchemaEntry {
        name: "thread",
        body: include_str!("../schema/thread.schema.json"),
    },
    SchemaEntry {
        name: "usage",
        body: include_str!("../schema/usage.schema.json"),
    },
    SchemaEntry {
        name: "watchlist",
        body: include_str!("../schema/watchlist.schema.json"),
    },
];

/// Default schema when `bird schema` is invoked with no name: the universal
/// success envelope.
const DEFAULT_SCHEMA: &str = "success-envelope";

fn find(name: &str) -> Option<&'static SchemaEntry> {
    SCHEMAS.iter().find(|s| s.name == name)
}

fn names() -> Vec<&'static str> {
    SCHEMAS.iter().map(|s| s.name).collect()
}

/// Emit the list of available schema names. Text mode: one per line. JSON
/// mode: success-envelope wrapper with the names array as `data`.
fn print_list(out: &OutputConfig) -> Result<(), BirdError> {
    if out.format.is_json() {
        let data = serde_json::Value::Array(
            names()
                .into_iter()
                .map(|n| serde_json::Value::String(n.to_string()))
                .collect(),
        );
        let meta = serde_json::json!({});
        let line = success_envelope_string(&data, &meta).map_err(|e| {
            BirdError::general(
                "schema",
                Box::<dyn std::error::Error + Send + Sync>::from(e),
            )
        })?;
        crate::out_println!("{}", line);
    } else {
        for n in names() {
            crate::out_println!("{}", n);
        }
    }
    Ok(())
}

fn print_schema(entry: &SchemaEntry, out: &OutputConfig) -> Result<(), BirdError> {
    // Schema bodies are themselves JSON documents; emit verbatim so byte
    // equality with disk files holds. Trim trailing newline first so the
    // out_println! macro adds exactly one.
    let body = entry.body.trim_end_matches('\n');

    if out.format.is_json() {
        // Even under JSON mode, the schema document itself is the meaningful
        // payload — wrapping it in the success envelope would obscure $schema/
        // $id. Emit the schema directly; agents that parse it will see a
        // self-describing JSON Schema 2020-12 document.
        crate::out_println!("{}", body);
    } else {
        crate::out_println!("{}", body);
    }
    Ok(())
}

/// Dispatch `bird schema [--list] [<name>]`.
///
/// - `name=None, list=false` -> emit the success-envelope schema (universal shape)
/// - `list=true` -> emit names (text: one per line; json: envelope array)
/// - `name=Some(n)` -> emit schema `n`; unknown name -> Usage error (exit 2)
pub(crate) fn run(name: Option<&str>, list: bool, out: &OutputConfig) -> Result<(), BirdError> {
    if list {
        return print_list(out);
    }
    let target = name.unwrap_or(DEFAULT_SCHEMA);
    match find(target) {
        Some(entry) => print_schema(entry, out),
        None => {
            let available = names().join(", ");
            Err(BirdError::usage(
                "unknown-schema",
                format!("unknown schema '{}'; available: {}", target, available),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputFormat;

    fn out_text() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    fn out_json() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Json,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    #[test]
    fn schema_table_is_sorted_alphabetically() {
        let names: Vec<&str> = SCHEMAS.iter().map(|s| s.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            names, sorted,
            "SCHEMAS table must be sorted by name for stable --list output"
        );
    }

    #[test]
    fn every_schema_body_parses_as_json() {
        for entry in SCHEMAS {
            let parsed: serde_json::Value = serde_json::from_str(entry.body)
                .unwrap_or_else(|e| panic!("schema {} did not parse as JSON: {}", entry.name, e));
            assert!(
                parsed.get("$schema").is_some(),
                "schema {} missing $schema",
                entry.name
            );
            assert!(
                parsed.get("$id").is_some(),
                "schema {} missing $id",
                entry.name
            );
            assert!(
                parsed.get("title").is_some(),
                "schema {} missing title",
                entry.name
            );
            assert!(
                parsed.get("type").is_some(),
                "schema {} missing type",
                entry.name
            );
            assert!(
                parsed.get("properties").is_some(),
                "schema {} missing properties",
                entry.name
            );
        }
    }

    #[test]
    fn id_uses_stable_bird_dev_url_pattern() {
        for entry in SCHEMAS {
            let parsed: serde_json::Value =
                serde_json::from_str(entry.body).expect("parses as JSON");
            let id = parsed
                .get("$id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let expected = format!("https://bird.dev/schema/{}-v1.json", entry.name);
            assert_eq!(
                id, expected,
                "schema {} $id should be {}",
                entry.name, expected
            );
        }
    }

    #[test]
    fn find_returns_known_schema() {
        let entry = find("bookmarks").expect("bookmarks is in the table");
        assert_eq!(entry.name, "bookmarks");
        assert!(entry.body.contains("\"$id\""));
    }

    #[test]
    fn find_returns_none_for_unknown() {
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn run_unknown_returns_usage_error() {
        let err =
            run(Some("does-not-exist"), false, &out_text()).expect_err("unknown schema must error");
        assert_eq!(err.error_id(), "unknown-schema");
        assert_eq!(err.kind(), "usage");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn run_list_text_succeeds() {
        assert!(run(None, true, &out_text()).is_ok(), "--list (text) failed");
    }

    #[test]
    fn run_list_json_succeeds() {
        assert!(run(None, true, &out_json()).is_ok(), "--list (json) failed");
    }

    #[test]
    fn run_default_emits_success_envelope_schema() {
        assert!(
            run(None, false, &out_text()).is_ok(),
            "default schema emit failed"
        );
    }

    #[test]
    fn run_each_named_schema_succeeds() {
        for entry in SCHEMAS {
            run(Some(entry.name), false, &out_text()).unwrap_or_else(|e| {
                panic!(
                    "schema {} should print, got error: {}",
                    entry.name,
                    e.message()
                )
            });
        }
    }
}
