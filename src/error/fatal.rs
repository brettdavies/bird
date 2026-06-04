//! Single permitted `eprintln!` survivor: the fatal-error chokepoint.
//!
//! Plan 2 U9's CI grep guard forbids `eprintln!` everywhere in `src/` except
//! this file (`src/error/fatal.rs`) and the binary shim (`src/main.rs`). The
//! tightly-scoped carve-out keeps the fatal error path inspectable and lets
//! future contributors add `eprintln!` here without inviting it elsewhere.

/// Write a single line to the process's real stderr.
///
/// Only call this from the fatal-error chokepoint (`BirdError::print()` or
/// the bare-error path in the runner). Every non-fatal diagnostic goes
/// through `OutputConfig::print_diag` against a runner-injected writer
/// instead, so test fixtures can capture them.
pub(crate) fn fatal_eprintln(line: &str) {
    eprintln!("{line}");
}
