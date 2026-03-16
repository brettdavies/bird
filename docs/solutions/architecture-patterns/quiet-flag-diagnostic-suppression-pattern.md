---
title: "Global --quiet flag and diag! macro for suppressible stderr diagnostics"
category: architecture-patterns
date: 2026-03-16
tags:
  - cli
  - quiet-mode
  - stderr
  - macros
  - clap
  - agentic-scripting
  - environment-variables
components:
  - src/output.rs
  - src/main.rs
  - src/db/client.rs
  - src/cost.rs
  - src/bookmarks.rs
  - src/search.rs
  - src/thread.rs
  - src/watchlist.rs
  - src/usage.rs
  - src/transport.rs
  - src/db/db.rs
  - src/doctor.rs
severity: medium
resolution_time: "2-4 hours"
---

# Global --quiet flag and diag! macro for suppressible stderr diagnostics

## Problem

Bird's informational stderr diagnostics (store hits/misses, cost estimates, entity upsert warnings, usage logging notes) were emitted unconditionally via `eprintln!`. This polluted stderr for machine consumers -- scripts, agents, and CI pipelines that pipe `bird` output and inspect `$?`. There was no way to get clean structured JSON on stdout without stderr noise.

## Root Cause

All 40 informational `eprintln!` sites across 12 modules called `eprintln!` directly with no gating mechanism. The 3 fatal error paths in `BirdError::print()` were correctly on stderr, but there was no distinction between "informational diagnostic" and "fatal error" at the output layer.

## Solution

### The `diag!` macro

A `#[macro_export]` macro in `src/output.rs` gates `eprintln!` behind a quiet boolean with zero allocation when suppressed (format arguments are not evaluated):

```rust
#[macro_export]
macro_rules! diag {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            eprintln!($($arg)*);
        }
    };
}
```

Usage is a drop-in replacement for `eprintln!`:

```rust
diag!(quiet, "[store] Cleared {} stored entries after login.", count);
diag!(self.quiet, "[store] warning: entity upsert failed: {e}");
```

### The CLI flag with `FalseyValueParser`

The flag supports both `--quiet`/`-q` and a `BIRD_QUIET` environment variable. `FalseyValueParser` is essential: it makes the env var treat `""`, `"0"`, `"false"`, `"no"`, `"off"` as false, so `BIRD_QUIET=0` correctly disables quiet mode:

```rust
#[arg(
    long,
    short = 'q',
    global = true,
    env = "BIRD_QUIET",
    value_parser = clap::builder::FalseyValueParser::new(),
)]
quiet: bool,
```

Requires the clap `env` feature: `clap = { version = "4.4", features = ["derive", "env"] }`.

### Early-return pattern for display functions

Functions whose entire body is conditional stderr use an early return instead of wrapping every line in `diag!`:

```rust
pub fn display_cost(estimate: &CostEstimate, use_color: bool, quiet: bool) {
    if quiet { return; }
    // ... display logic with multiple eprintln! calls unchanged
}
```

### Hybrid threading: parameter + struct field

Two complementary strategies based on call depth:

**Parameter threading** for command handlers -- `quiet: bool` passed explicitly:

```rust
fn run(command: Command, config: ResolvedConfig, client: &mut db::BirdClient,
       use_color: bool, quiet: bool, cache_only: bool) -> Result<(), BirdError> { ... }
```

**Struct field** for `BirdClient` where 7+ internal methods emit diagnostics:

```rust
pub struct BirdClient {
    // ...
    pub quiet: bool,
}
```

Internal methods use `self.quiet`:

```rust
diag!(self.quiet, "[usage] warning: failed to log API call: {e}");
```

The `use_color` parameter is deliberately kept as parameter-only (not a struct field) because color is a display concern at the output boundary, not inside the DB layer. `quiet` crossed into the DB layer because store-level operations emit their own diagnostics.

### Fatal errors preserved

`BirdError::print()` in `main.rs` uses bare `eprintln!` directly -- never gated by `diag!`. Exit codes are unchanged. The classification rule: if the program exits 0 without the line, it's informational (use `diag!`). If removing it would hide a failure cause, it's fatal (use `eprintln!` in `BirdError::print()`).

## Gotchas

### `#[macro_export]` import path

`#[macro_export]` hoists the macro to the crate root, NOT to `crate::output::diag`. The correct import in every module:

```rust
use crate::diag;
```

Not `use crate::output::diag;` -- that path does not exist.

### 33-error cascade on bulk migration

When all 40 `eprintln!` calls were migrated in one pass without adding imports first, every module produced "cannot find macro `diag`" errors. The safe migration strategy: add the macro, then add `use crate::diag;` to each module one at a time.

### Test helpers need `quiet` parameter

`BirdClient::new()` requires `quiet: bool` as its last argument. All test helpers that construct a `BirdClient` must pass `false` (or a test-controlled value). This is caught at compile time but can produce confusing errors if you don't know where to look.

## Prevention

**New `eprintln!` calls must use `diag!` instead.** Verify with:

```sh
rg 'eprintln!' src/ --type rust -n
```

Legitimate survivors: `BirdError::print()` in `main.rs` (3 lines), `display_cost()` in `cost.rs` (2 lines), and the macro definition in `output.rs`. Any other file is a regression.

**Verify `diag!` call sites match import sites:**

```sh
rg 'diag!' src/ --type rust -l | sort
rg 'use crate::diag' src/ --type rust -l | sort
```

Both lists must match exactly.

**New command handlers must thread `quiet: bool`.** After adding a new command, verify its handler accepts quiet:

```sh
rg 'fn run_' src/ --type rust -n
```

**When to use `diag!` vs early-return:**

| Situation | Pattern |
|---|---|
| Single diagnostic line | `diag!(quiet, "message")` |
| Function whose entire body is conditional stderr | `if quiet { return; }` at entry |
| `BirdClient` internal methods | `diag!(self.quiet, "message")` |

## Related Solutions

- [xurl subprocess transport layer](../architecture-patterns/xurl-subprocess-transport-layer.md) -- `transport.rs` modified by quiet flag; "exit codes are public API" rule; "global flags through single chokepoint" pattern
- [Code review round 2 quality improvements](../architecture-patterns/code-review-round2-quality-improvements.md) -- `sanitize_for_stderr()` origin in `output.rs`; complementary stderr governance (content safety vs presence); "public methods log" rule
- [Security audit](../security-issues/rust-cli-security-code-quality-audit.md) -- `BirdError` enum and exit codes 78/77/1; `BirdError::print()` must never be suppressed
- [Live integration testing](../architecture-patterns/live-integration-testing-cli-external-api.md) -- `assert_cmd` patterns used for quiet flag smoke tests; stderr assertion patterns
- [SQLite cache layer](../performance-issues/sqlite-cache-layer-api-cost-reduction.md) -- origin of `[cost]` stderr diagnostics; `display_cost()` pattern
