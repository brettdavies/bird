//! Run several bird invocations concurrently, each with its own fixture.
//!
//! Every thread builds an isolated `ResolvedPaths` rooted at its own
//! `TempDir`. Because the runner never reads `HOME` / `XDG_CONFIG_HOME`
//! itself, the threads do not race on process-global env vars. The
//! `tests/parallel_run.rs` regression covers the same shape at 16 threads.
//!
//! `--version` is a clap-handled short-circuit that prints directly to the
//! process stdout, bypassing the injected writers; that is why each
//! thread's printed `stdout=""` line shows an empty capture while the
//! `bird 0.1.x` banners still appear on the real stdout.
//!
//! Run with: `cargo run --example parallel_invocations`
//!
//! Expected exit: 0 if every thread's invocation exited 0.

use bird::config::{EnvOverrides, ResolvedPaths};
use std::process::ExitCode;
use std::thread;
use tempfile::TempDir;

const THREADS: usize = 8;

fn main() -> ExitCode {
    let handles: Vec<_> = (0..THREADS)
        .map(|i| {
            thread::spawn(move || {
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
                    ["bird", "--version"],
                    &mut stdout,
                    &mut stderr,
                    paths,
                    EnvOverrides::default(),
                );

                let stdout = String::from_utf8_lossy(&stdout).into_owned();
                println!("thread {}: exit={:?} stdout={:?}", i, exit, stdout.trim());
                exit
            })
        })
        .collect();

    let mut all_ok = true;
    for handle in handles {
        let exit = handle.join().expect("example: thread panicked");
        // ExitCode does not impl PartialEq; compare via Debug like the runner tests.
        if format!("{:?}", exit) != format!("{:?}", ExitCode::SUCCESS) {
            all_ok = false;
        }
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
