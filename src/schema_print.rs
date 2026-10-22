//! `bird schema` subcommand: JSON Schemas for the documents bird prints, in
//! `xr schema`'s shape.
//!
//! `bird schema <command>` prints that command's response schema itself,
//! `bird schema envelope` the envelope schema, `--list` the names, and
//! `--all` every schema keyed by name. Each document at
//! `schema/<name>.schema.json` is embedded at build time; the envelope
//! schema is generated from the envelope types and a test holds the committed
//! file equal to it. `tests/schema_parity.rs` enforces that embedded bytes
//! equal on-disk bytes.

use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::output::envelope::Reason;

/// Single schema entry: the command name it describes paired with the
/// embedded document bytes.
pub(crate) struct SchemaEntry {
    pub name: &'static str,
    pub body: &'static str,
}

/// Table of all embedded schemas, sorted by name for deterministic `--list`
/// output. Names match `schema/<name>.schema.json` on disk.
pub(crate) const SCHEMAS: &[SchemaEntry] = &[
    SchemaEntry {
        name: "bookmarks",
        body: include_str!("../schema/bookmarks.schema.json"),
    },
    SchemaEntry {
        name: "cache-stats",
        body: include_str!("../schema/cache-stats.schema.json"),
    },
    SchemaEntry {
        name: "doctor",
        body: include_str!("../schema/doctor.schema.json"),
    },
    SchemaEntry {
        name: "envelope",
        body: include_str!("../schema/envelope.schema.json"),
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
        name: "skill-install",
        body: include_str!("../schema/skill-install.schema.json"),
    },
    SchemaEntry {
        name: "skill-update",
        body: include_str!("../schema/skill-update.schema.json"),
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
        name: "watchlist-fetch",
        body: include_str!("../schema/watchlist-fetch.schema.json"),
    },
    SchemaEntry {
        name: "watchlist-list",
        body: include_str!("../schema/watchlist-list.schema.json"),
    },
];

/// The forms `bird schema` takes, named in its usage error.
const FORMS: &str =
    "bird schema <command> | bird schema envelope | bird schema --list | bird schema --all";

fn find(name: &str) -> Option<&'static SchemaEntry> {
    SCHEMAS.iter().find(|s| s.name == name)
}

fn names() -> Vec<&'static str> {
    SCHEMAS.iter().map(|s| s.name).collect()
}

fn parse(entry: &SchemaEntry) -> serde_json::Value {
    serde_json::from_str(entry.body).unwrap_or(serde_json::Value::Null)
}

fn title(entry: &SchemaEntry) -> String {
    parse(entry)["title"]
        .as_str()
        .unwrap_or(entry.name)
        .to_string()
}

fn io_to_bird(e: std::io::Error) -> BirdError {
    BirdError::general("schema", Box::new(e))
}

/// `<command>  <title>` rows sorted by command, `envelope` among them. Under
/// JSON one ok document lists them.
fn print_list(out: &OutputConfig, stdout: &mut dyn std::io::Write) -> Result<(), BirdError> {
    if out.format.is_json() {
        let schemas: Vec<serde_json::Value> = SCHEMAS
            .iter()
            .map(|e| serde_json::json!({"command": e.name, "type": title(e)}))
            .collect();
        return out
            .print_ok(stdout, serde_json::json!({ "schemas": schemas }))
            .map_err(io_to_bird);
    }
    let width = SCHEMAS.iter().map(|e| e.name.len()).max().unwrap_or(0);
    for entry in SCHEMAS {
        writeln!(stdout, "{:<width$}  {}", entry.name, title(entry)).map_err(io_to_bird)?;
    }
    Ok(())
}

/// One object keyed by command name, each value the schema itself.
fn print_all(out: &OutputConfig, stdout: &mut dyn std::io::Write) -> Result<(), BirdError> {
    let all: serde_json::Map<String, serde_json::Value> = SCHEMAS
        .iter()
        .map(|e| (e.name.to_string(), parse(e)))
        .collect();
    let json = OutputConfig {
        format: crate::output::OutputFormat::Json,
        ..out.clone()
    };
    json.write_structured(stdout, &serde_json::Value::Object(all))
        .map_err(io_to_bird)
}

/// The schema document itself, verbatim, under every output mode.
fn print_schema(entry: &SchemaEntry, stdout: &mut dyn std::io::Write) -> Result<(), BirdError> {
    writeln!(stdout, "{}", entry.body.trim_end_matches('\n')).map_err(io_to_bird)
}

/// Dispatch `bird schema [<command>] [--list] [--all]`.
///
/// No command and no flag is a usage error naming the forms, as
/// `xr schema` is; an unknown name is `unknown-schema`.
pub fn run(
    name: Option<&str>,
    list: bool,
    all: bool,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
) -> Result<(), BirdError> {
    if all {
        return print_all(out, stdout);
    }
    if list {
        return print_list(out, stdout);
    }
    let Some(target) = name else {
        return Err(BirdError::usage(
            Reason::InvalidArgs,
            format!("usage: {FORMS}"),
        ));
    };
    match find(target) {
        Some(entry) => print_schema(entry, stdout),
        None => Err(BirdError::usage(
            Reason::UnknownSchema,
            format!(
                "unknown schema '{}'; available: {}",
                target,
                names().join(", ")
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputFormat;

    fn out(format: OutputFormat) -> OutputConfig {
        OutputConfig {
            format,
            use_color: false,
            quiet: false,
            raw: false,
        }
    }

    #[test]
    fn schema_table_is_sorted_alphabetically() {
        let mut sorted = names();
        sorted.sort_unstable();
        assert_eq!(names(), sorted);
    }

    #[test]
    fn every_schema_parses_with_a_title_and_no_id() {
        for entry in SCHEMAS {
            let parsed: serde_json::Value = serde_json::from_str(entry.body)
                .unwrap_or_else(|e| panic!("schema {} did not parse: {e}", entry.name));
            assert!(
                parsed.get("$schema").is_some(),
                "{} lacks $schema",
                entry.name
            );
            assert!(parsed.get("title").is_some(), "{} lacks title", entry.name);
            assert!(parsed.get("$id").is_none(), "{} carries $id", entry.name);
        }
    }

    #[test]
    fn a_named_schema_prints_unwrapped() {
        let mut stdout = Vec::new();
        run(
            Some("thread"),
            false,
            false,
            &out(OutputFormat::Json),
            &mut stdout,
        )
        .unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert!(doc.get("status").is_none());
        assert_eq!(doc["title"], "bird thread");
    }

    #[test]
    fn the_envelope_schema_is_the_committed_generated_one() {
        let mut stdout = Vec::new();
        run(
            Some("envelope"),
            false,
            false,
            &out(OutputFormat::Json),
            &mut stdout,
        )
        .unwrap();
        let printed: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(printed, crate::output::envelope::envelope_schema());
    }

    #[test]
    fn no_name_and_no_flag_is_a_usage_error_naming_the_forms() {
        let mut stdout = Vec::new();
        let err = run(None, false, false, &out(OutputFormat::Text), &mut stdout)
            .expect_err("bare schema refuses");
        assert_eq!((err.error_id(), err.exit_code()), ("invalid-args", 2));
        assert!(err.message().contains("bird schema --all"));
    }

    #[test]
    fn an_unknown_name_is_unknown_schema() {
        let mut stdout = Vec::new();
        let err = run(
            Some("nope"),
            false,
            false,
            &out(OutputFormat::Text),
            &mut stdout,
        )
        .expect_err("unknown schema");
        assert_eq!(err.error_id(), "unknown-schema");
        assert!(err.message().contains("watchlist-list"));
    }

    #[test]
    fn list_and_all_name_the_same_commands() {
        let mut list = Vec::new();
        run(None, true, false, &out(OutputFormat::Json), &mut list).unwrap();
        let list: serde_json::Value = serde_json::from_slice(&list).unwrap();
        assert_eq!(list["status"], "ok");
        let listed: Vec<&str> = list["schemas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["command"].as_str().unwrap())
            .collect();

        let mut all = Vec::new();
        run(None, false, true, &out(OutputFormat::Text), &mut all).unwrap();
        let all: serde_json::Value = serde_json::from_slice(&all).unwrap();
        let keys: Vec<&str> = all
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(listed, keys);
        assert!(listed.contains(&"envelope"));
    }

    #[test]
    fn list_text_prints_one_row_per_schema() {
        let mut stdout = Vec::new();
        run(None, true, false, &out(OutputFormat::Text), &mut stdout).unwrap();
        let text = String::from_utf8(stdout).unwrap();
        assert_eq!(text.lines().count(), SCHEMAS.len());
        assert!(text.lines().any(|l| l.starts_with("envelope ")));
    }
}
