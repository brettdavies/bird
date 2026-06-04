//! Minimal smoke test for the binary entrypoint.
//!
//! Calls `bird::cli::run_argv()` — the same function `src/main.rs` calls.
//! Demonstrates that the library can drive the full CLI from a host process
//! without re-implementing argv resolution. Exits with the same code the
//! `bird` binary would.
//!
//! Run with: `cargo run --example library_smoke -- --version`
//!
//! The library surface is internal test infrastructure; downstream consumers
//! should shell out to the `bird` binary in production. See `src/lib.rs`.

use std::process::ExitCode;

fn main() -> ExitCode {
    bird::cli::run_argv()
}
