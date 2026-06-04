//! Invoke `bird doctor` in-process with TempDir-backed paths.
//!
//! Mirrors the fixture pattern from `tests/common/mod.rs`: build a
//! `ResolvedPaths` rooted at a fresh `TempDir`, pass empty `EnvOverrides`,
//! and capture stdout/stderr into `Vec<u8>` buffers. The runner never touches
//! `HOME` or `XDG_CONFIG_HOME` — every read is satisfied by the injected
//! paths/env, so the call is safe to run concurrently with other invocations.
//!
//! Run with: `cargo run --example in_process_doctor`
//!
//! Expected exit: 0 (doctor reports environment status, does not fail on a
//! fresh temp dir).

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
    let env = EnvOverrides::default();

    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();

    let exit = bird::cli::run_with_paths(
        ["bird", "doctor", "--output", "json"],
        &mut stdout,
        &mut stderr,
        paths,
        env,
    );

    println!("--- captured stdout ({} bytes) ---", stdout.len());
    print!("{}", String::from_utf8_lossy(&stdout));
    println!("--- captured stderr ({} bytes) ---", stderr.len());
    print!("{}", String::from_utf8_lossy(&stderr));

    exit
}
