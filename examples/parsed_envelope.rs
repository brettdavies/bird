//! Capture and parse a JSON success envelope from an in-process invocation.
//!
//! Runs `bird --examples --json` against an injected `ResolvedPaths`,
//! captures stdout, and parses the standard `{"data": ..., "meta": ...}`
//! envelope with `serde_json`. The same envelope shape holds for any
//! JSON-emitting command (`bird cache stats --output json`, `bird doctor
//! --output json`, etc.); this example uses `--examples` because it is
//! network-free.
//!
//! Run with: `cargo run --example parsed_envelope`
//!
//! Expected exit: 0.

use bird::config::{EnvOverrides, ResolvedPaths};
use std::process::ExitCode;
use tempfile::TempDir;

fn main() -> ExitCode {
    let tmp = TempDir::new().expect("example: tempdir");
    let config_dir = tmp.path().join(".config").join("bird");
    std::fs::create_dir_all(&config_dir).expect("example: create config dir");
    let paths = ResolvedPaths {
        config_dir: config_dir.clone(),
        store_path: config_dir,
    };

    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let exit = bird::cli::run_with_paths(
        ["bird", "--examples", "--json"],
        &mut stdout,
        &mut stderr,
        paths,
        EnvOverrides::default(),
    );

    let body = String::from_utf8_lossy(&stdout);
    let envelope: serde_json::Value = match serde_json::from_str(body.trim()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("example: stdout was not valid JSON: {}", e);
            eprintln!("--- raw stdout ---\n{}", body);
            eprintln!("--- raw stderr ---\n{}", String::from_utf8_lossy(&stderr));
            return ExitCode::FAILURE;
        }
    };

    let data = envelope.get("data").and_then(|v| v.as_array());
    let count = envelope
        .get("meta")
        .and_then(|m| m.get("count"))
        .and_then(|c| c.as_u64());

    println!(
        "parsed envelope: data.len={} meta.count={:?}",
        data.map(|a| a.len()).unwrap_or(0),
        count
    );
    if let Some(first) = data.and_then(|a| a.first()) {
        println!("first example invocation: {}", first);
    }

    exit
}
