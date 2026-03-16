---
title: "feat: Add --quiet flag and document agentic contracts"
type: feat
status: completed
date: 2026-03-16
deepened: 2026-03-16
deepened_round2: 2026-03-16
origin: docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md
schedule: docs/plans/2026-03-16-000-meta-implementation-schedule.md
schedule_phase: 3
---

# feat: Add --quiet flag and document agentic contracts

## Enhancement Summary

**Deepened round 2 on:** 2026-03-16
**Research agents used (round 2):** best-practices-researcher, framework-docs-researcher, git-history-analyzer, learnings-researcher, code-simplicity-reviewer, pattern-recognition-specialist, architecture-strategist, security-sentinel, agent-native-reviewer, performance-oracle, spec-flow-analyzer + Context7 (clap docs)

### Critical Fixes Discovered (Round 2)

1. **`"env"` feature missing from `Cargo.toml`** -- clap dependency is `features = ["derive"]` only; must add `"env"` for `#[arg(env = "BIRD_QUIET")]` to compile
2. **`FalseyValueParser` required** -- bare `bool` + `env` causes `BIRD_QUIET=1` to error with "invalid value '1' for '--quiet'"; must use `clap::builder::FalseyValueParser::new()` so `0`/`false`/`no`/`off`/`n`/`f` = false, anything else = true
3. **Phase 1 code snippet was wrong** -- missing `env` attribute and `FalseyValueParser`; corrected below
4. **`main.rs` has 5 suppressible calls, not 3** -- X_API_USERNAME warning (1), post-login store clear (1), cache clear success (1), 2x "Store is not available" = 5 to suppress + 3 fatal to keep = 8 total
5. **`display_cost()` called from 6 modules (7 sites), not 5** -- also called from `profile.rs`

### Architectural Decisions Confirmed (Round 2)

- **Hybrid threading approach**: store `quiet` on `BirdClient` for 11 DB-layer sites + pass as parameter for command handlers (committed, stop hedging)
- **Define `diag!` macro in `src/output.rs`** with `#[macro_export]` -- this is the first macro in the codebase, justified for 35+ active call sites; a function alternative forces allocation even when quiet
- **`BIRD_QUIET=1` is NOT YAGNI** -- zero implementation cost via clap `env` attribute; core to the agentic contract
- **Split README documentation to a separate follow-up PR** -- Phase 4 documents pre-existing features alongside the new flag, violating SRP for PRs; this PR should only add `--quiet`
- **Note `OutputContext` struct as follow-up refactor** -- before any third display flag is added, consolidate `use_color` + `quiet` into a struct to avoid parameter proliferation (`run_raw` already has 9 params with `#[allow(clippy::too_many_arguments)]`)
- **Exit codes are unchanged by `--quiet`** -- Bird is not grep; exit codes reflect operation success/failure, not match presence

### Security Findings (Round 2)

- **xurl version warning (`transport.rs:99`) is only called from `doctor.rs`** -- not during normal operation; no runtime risk for `--quiet` users
- **No secret leakage in error messages** -- `BirdError::print()` verified safe; API responses sanitized via `sanitize_for_stderr()`
- **Store warnings safe to suppress** -- graceful degradation to no-cache is robust; `bird doctor` reports store health on stdout (unaffected by `--quiet`)

### Agent-Native Findings (Round 2)

- **`--pretty` is a semantic trap for agents** -- for `cache stats`, `usage`, and `doctor`, `--pretty` means "plain text" not "pretty-printed JSON"; agents must never use `--pretty`; document in README follow-up PR
- **Watchlist check uses NDJSON** -- one JSON object per line, not a single JSON document; `bird watchlist check | jq .` fails; document in README follow-up PR
- **`bird cache clear` produces no stdout** -- breaks "stdout is always JSON" contract; note in README follow-up PR
- **Exit code 2 is clap's, not Bird's** -- not tested; add integration test

### Pre-existing Bug Found (Round 2)

- **`watchlist add` duplicate detection prints misleading "Added" message** -- `add_to_watchlist()` returns `Ok(())` early on duplicate (line 44), then `run_watchlist_add()` unconditionally prints "Added @alice to watchlist." (line 149); both messages appear for duplicates; fix separately as a prerequisite or note in this PR

### Performance (Round 2)

- **Zero performance concerns** -- the branch check at 43 sites adds ~43ns total; a single `eprintln!` costs 1,000-10,000ns (stderr lock + syscall); network I/O (200-2000ms) dwarfs everything; `diag!` macro produces identical machine code to inline `if`; parameter passing is the fastest threading approach for single-threaded CLI

## Overview

Add a `--quiet` / `-q` global flag that suppresses informational stderr diagnostics, keeping only fatal error messages. This is PR 4 of 4 from the distribution/DX brainstorm (see brainstorm: `docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md`).

**Scope change from round 1:** README documentation of agentic contracts is split to a separate follow-up PR (SRP). This PR focuses solely on the `--quiet` flag implementation and tests.

## Problem Statement / Motivation

Bird already has a strong agentic foundation: JSON stdout by default, structured exit codes (0/1/77/78), `--plain` for color suppression, and errors on stderr. The remaining gap is that informational diagnostics (cost estimates, pagination progress, store warnings, action confirmations) are mixed with error messages on stderr. Agents piping bird's output need a way to get only data on stdout and only fatal errors on stderr.

## Proposed Solution

### `--quiet` suppression scope

Follow the `curl --silent` convention: suppress **informational diagnostics** but preserve **fatal error messages**.

| Category | Example | Suppressed by `--quiet`? |
|----------|---------|:---:|
| Fatal errors | `"auth failed: unauthorized"` | No |
| Progress info | `"[search] page 1/3: 42 new tweets"` | Yes |
| Cost estimates | `"[cost] ~$0.0050 (1 tweet lookup)"` | Yes |
| Store warnings | `"[store] warning: pruning failed"` | Yes |
| Action feedback | `"Added @alice to watchlist."` | Yes |
| Config warnings | `"[config] warning: X_API_USERNAME invalid"` | Yes |
| Cache clear feedback | `"Cleared 42 stored entities (1.2 MB)."` | Yes |
| Store unavailable | `"Store is not available."` | Yes |
| `RUST_LOG` tracing output | `DEBUG bird: ...` | No (separate concern; developer opt-in via `RUST_LOG`) |

Fatal errors (`BirdError::print()` at `src/main.rs:50-63`) are the primary error reporting mechanism. They must remain visible so agents can diagnose failures beyond just the exit code.

`--quiet` is orthogonal to all existing flags (`--plain`, `--no-color`, `--pretty`). No interactions.

### Clap flag definition (corrected)

**Prerequisite:** Add `"env"` feature to clap in `Cargo.toml`:

```toml
clap = { version = "4.4", features = ["derive", "env"] }
```

Flag definition:

```rust
/// Suppress informational stderr output (keep only fatal errors)
#[arg(
    long,
    short = 'q',
    global = true,
    env = "BIRD_QUIET",
    value_parser = clap::builder::FalseyValueParser::new(),
)]
quiet: bool,
```

**Why `FalseyValueParser`:** Without it, clap's default `BoolValueParser` treats ANY non-empty env var string as "flag present" -- so `BIRD_QUIET=0`, `BIRD_QUIET=false`, and `BIRD_QUIET=no` would all incorrectly enable quiet mode. `FalseyValueParser` correctly interprets: `0`, `false`, `no`, `off`, `n`, `f` (case-insensitive) as false; everything else (including `1`, `true`, `yes`) as true; absent = false.

**Precedence (clap built-in):** CLI flag > env var > default. If `--quiet` is passed, it wins regardless of `BIRD_QUIET`.

### Threading strategy (committed)

**Hybrid approach:**

1. **Parameter passing for command handlers** (matching `use_color: bool` pattern) -- add `quiet: bool` alongside `use_color` in `run()` and all command entry functions
2. **Struct field on `BirdClient`** -- store `quiet: bool` set in `BirdClient::new()` for the 11 `eprintln!` sites in `db/client.rs` and `db/db.rs`

This is a deliberate deviation from the pure `use_color` pattern (which is never stored on a struct). Justified because `BirdClient` has 7 internal `eprintln!` sites in methods like `decompose_and_upsert()`, `log_api_call()`, and `store_raw_response()` that would require threading `quiet` through every method otherwise. Add a code comment on the `quiet` field explaining this deviation.

**Historical precedent:** `use_color` was threaded through the codebase in 7 commits over 4 days with zero follow-up fixes. The parameter position convention is: `use_color` comes after business/content params, before auth/mode params. Place `quiet` immediately after `use_color`.

### `diag!` macro

Define in `src/output.rs` with `#[macro_export]`:

```rust
/// Diagnostic output macro -- prints to stderr unless quiet mode is active.
/// Use this instead of bare `eprintln!` for all informational output.
/// Fatal errors use `BirdError::print()` directly (never suppressed).
#[macro_export]
macro_rules! diag {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            eprintln!($($arg)*);
        }
    };
}
```

This is the first macro in the codebase. Justified: the function alternative forces `format!()` allocation at each call site even when quiet. The macro avoids this -- format arguments are only evaluated when `!quiet`. Within `BirdClient` methods, use `diag!(self.quiet, ...)`.

### eprintln! audit (corrected counts)

43 `eprintln!` call sites across 10 files:

| File | Count | Keep / Suppress | Notes |
|------|-------|-----------------|-------|
| `src/main.rs` | 8 | 3 keep (fatal in `BirdError::print()`), 5 suppress | X_API_USERNAME warning, post-login store clear, cache clear success, 2x store unavailable |
| `src/watchlist.rs` | 8 | All suppress | Action feedback, progress, errors |
| `src/db/client.rs` | 7 | All suppress | Store warnings via `self.quiet` on struct |
| `src/usage.rs` | 7 | All suppress | Sync status, progress, hints |
| `src/db/db.rs` | 4 | All suppress | Migration warnings, pass `quiet` to migration fn |
| `src/thread.rs` | 3 | All suppress | Progress, age warning, summary |
| `src/search.rs` | 2 | All suppress | Pagination progress, summary |
| `src/cost.rs` | 2 | All suppress | Cost estimate display |
| `src/transport.rs` | 1 | Suppress | xurl version warning (only called from doctor) |
| `src/bookmarks.rs` | 1 | All suppress | Progress |

**Total: 43** (3 fatal keep + 40 suppress)

**`display_cost()` call sites (corrected):** Called from **6 modules** with **7 call sites**: `raw.rs`, `bookmarks.rs` (x2), `search.rs`, `profile.rs`, `thread.rs` (x2), `watchlist.rs` (via `execute_check`).

## Technical Considerations

### `--quiet` and `bird login`

`bird login` delegates to `xurl_passthrough()` which inherits all stdio. xurl's stderr output (browser-open instructions, success confirmation) still appears because `--quiet` only guards Bird's own `eprintln!` calls. The post-login store clear message at `main.rs:444` IS Bird's own `eprintln!` and SHOULD be suppressed. An agent would never run `bird login` (document in README follow-up).

### `--quiet` and `bird doctor`

Doctor output goes entirely to stdout (both JSON and pretty modes). `--quiet` has no visible effect. This is correct.

### `--quiet` and `bird cache clear`

Cache clear feedback goes to stderr (`eprintln!`). With `--quiet`, the agent gets only exit code 0 on success. No stdout output is produced (note this in README follow-up as a known gap in the "stdout is always JSON" contract).

### Store initialization warnings timing

Warnings in `BirdClient::new()` and migration functions fire during client construction in `main()` (line 845-851), BEFORE `run()` is called (line 853). The `quiet` flag (`cli.quiet`) is available at that point and must be passed to `BirdClient::new()`. This is the highest-risk gap -- if missed, `bird --quiet me` with a degraded store emits `[store] warning: ...` on stderr.

### `RUST_LOG` tracing output

The `tracing_subscriber` is initialized at `main.rs:776-782` before argument parsing. If `RUST_LOG=bird=debug` is set, tracing output appears on stderr regardless of `--quiet`. This is correct -- `RUST_LOG` is a developer opt-in, not a user-facing concern. No tracing macros are currently invoked in production code (only the subscriber setup exists).

### CI guard against bare `eprintln!` drift ($100 rule)

After migration to `diag!`, add a CI check to prevent new bare `eprintln!` calls:

```bash
# Fail if bare eprintln! exists outside BirdError::print() and tests
rg 'eprintln!' src/ --glob '!*test*' | rg -v 'BirdError' | rg -v '// quiet-exempt'
```

Any new `eprintln!` should use `diag!` unless explicitly marked `// quiet-exempt`.

## Acceptance Criteria

- [x] `"env"` feature added to clap dependency in `Cargo.toml`
- [x] `--quiet` / `-q` global flag added to `Cli` struct with `FalseyValueParser` and `env = "BIRD_QUIET"`
- [x] `diag!` macro defined in `src/output.rs` with `#[macro_export]`
- [x] `quiet: bool` stored as field on `BirdClient` struct (with code comment explaining deviation)
- [x] `bird --quiet me` (with valid auth) produces JSON on stdout, empty stderr, exit 0
- [x] `bird --quiet me` (without auth) produces error on stderr, exit 77 (fatal errors not suppressed)
- [x] `bird --quiet me` with degraded store produces no `[store]` warnings on stderr
- [x] `bird --quiet search "test"` suppresses `[search]` progress and `[cost]` lines on stderr
- [x] `bird --quiet bookmarks` suppresses `[cost]` and progress on stderr
- [x] `bird --quiet watchlist add alice` suppresses "Added @alice" confirmation on stderr
- [x] `bird --quiet cache clear` suppresses "Cleared N entities" confirmation on stderr
- [x] `bird --quiet usage` suppresses hints and sync status on stderr
- [x] `bird --quiet thread <id>` suppresses progress and age warnings on stderr
- [x] `bird --quiet --plain me` -- both flags work together without conflict
- [x] `BIRD_QUIET=1 bird me` produces same behavior as `bird --quiet me`
- [x] `BIRD_QUIET=0 bird me` does NOT enable quiet mode (FalseyValueParser)
- [x] All existing tests pass; `cargo clippy` clean; `cargo fmt` clean

## Implementation Plan

### Phase 1: Add `--quiet` flag and `diag!` macro

1. Add `"env"` feature to clap in `Cargo.toml`:

   ```toml
   clap = { version = "4.4", features = ["derive", "env"] }
   ```

2. Add to `Cli` struct in `src/main.rs`:

   ```rust
   /// Suppress informational stderr output (keep only fatal errors)
   #[arg(
       long,
       short = 'q',
       global = true,
       env = "BIRD_QUIET",
       value_parser = clap::builder::FalseyValueParser::new(),
   )]
   quiet: bool,
   ```

3. Define `diag!` macro in `src/output.rs` with `#[macro_export]`.

4. Pass `cli.quiet` to `BirdClient::new()` and to `run()`.

5. Update `run()` signature to accept `quiet: bool`.

6. Add `quiet: bool` field to `BirdClient` struct with code comment.

### Phase 2: Thread through all command handlers and convert `eprintln!` to `diag!`

Update each module's entry function to accept `quiet: bool` and convert all 40 suppressible `eprintln!` calls to `diag!`:

**Command handlers (parameter passing, `quiet` placed after `use_color`):**

- `src/main.rs` -- 5 suppressible `eprintln!` calls in `run()`: X_API_USERNAME warning, post-login store clear, cache clear success, 2x store unavailable
- `src/bookmarks.rs` -- `run_bookmarks()` (1 site)
- `src/search.rs` -- `run_search()` (2 sites)
- `src/thread.rs` -- `run_thread()` (3 sites)
- `src/watchlist.rs` -- `run_watchlist_*()` functions (8 sites); note: `run_watchlist_add/remove/list` do not currently receive `use_color` either
- `src/usage.rs` -- `run_usage()` (7 sites)
- `src/cost.rs` -- `display_cost()` (2 sites, called from 6 modules with 7 call sites)
- `src/transport.rs` -- xurl version warning (1 site)

**DB layer (struct field `self.quiet`):**

- `src/db/client.rs` -- 7 sites via `self.quiet` on `BirdClient`
- `src/db/db.rs` -- 4 migration warning sites; pass `quiet` to `migrate_usage_from_cache()` directly (fires during `BirdClient::new()` before struct is fully constructed)

Do NOT touch `BirdError::print()` -- fatal errors always print.

### Phase 3: Tests (`tests/cli_smoke.rs`)

Using `assert_cmd` patterns (already in dev-dependencies):

1. `bird --quiet --help` still shows help (clap exits before `run()`)
2. `bird --quiet completions bash` works (if completions PR exists)
3. `--quiet` flag accepted by clap (basic parse test)
4. `BIRD_QUIET=1` env var activates quiet mode (`.env("BIRD_QUIET", "1").assert().success().stderr("")`)
5. `BIRD_QUIET=0` env var does NOT activate quiet mode (FalseyValueParser test)
6. `bird --invalid-flag` exits 2 (exit code contract test -- new, tests clap behavior)

## Dependencies & Risks

- **`Cargo.toml` change required**: Adding `"env"` feature to clap. This is a compile-time feature flag, no runtime cost.
- **Function signature changes**: Adding `quiet: bool` to ~10 function signatures is mechanical but touches many files. `run_raw` will reach 10 params (already has `#[allow(clippy::too_many_arguments)]`).
- **Follow-up: `OutputContext` struct**: Before any third display flag is added, consolidate `use_color` + `quiet` into an `OutputContext` struct to avoid unbounded parameter growth. Not in this PR.
- **Follow-up: README documentation**: "Machine-Readable Output" section documenting agentic contracts (exit codes, `--pretty` semantic trap warning, NDJSON format for watchlist check, `bird cache clear` no-stdout). Separate PR.
- **Follow-up: Watchlist add duplicate bug**: `add_to_watchlist()` returns `Ok(())` on duplicate, then caller unconditionally prints "Added". Fix in separate PR.
- **Depends on PR 3 (completions)**: The `Completions` early-return means `quiet` doesn't need to be threaded to the completions handler. If PR 4 ships first, no conflict.

## Sources & References

- **Origin brainstorm:** [docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md](docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md)
- `curl --silent` convention: suppress informational output, preserve errors
- Existing `use_color: bool` threading pattern (threaded in 7 commits, zero follow-up fixes)
- `src/main.rs:50-63` -- `BirdError::print()` (fatal errors, NOT suppressed)
- `src/cost.rs:77-119` -- `display_cost()` called from 6 modules (7 sites)
- Security audit solution: `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md` -- exit codes are public API
- xurl transport solution: `docs/solutions/architecture-patterns/xurl-subprocess-transport-layer.md` -- "exit codes are public API; write contract tests before refactoring"
- Code review round 2: `docs/solutions/architecture-patterns/code-review-round2-quality-improvements.md` -- "public methods log; private methods do not"

### Ecosystem References

- Cargo `--quiet`: uses `Verbosity` enum on `Shell` struct; issue [#11691](https://github.com/rust-lang/cargo/issues/11691) shows incomplete suppression is a common pitfall
- ripgrep: uses global `AtomicBool` + macros (overkill for single-threaded CLI)
- just: uses `Verbosity` enum on `Config` struct
- intermodal: swaps stderr writer with `io::sink()` when quiet (elegant but requires refactoring all `eprintln!`)
- clap `FalseyValueParser`: [docs.rs/clap/latest/clap/builder/struct.FalseyValueParser.html](https://docs.rs/clap/latest/clap/builder/struct.FalseyValueParser.html) -- false values: `n`, `no`, `f`, `false`, `off`, `0` (case-insensitive)
- clap boolean env var gotcha: [clap-rs/clap#5591](https://github.com/clap-rs/clap/issues/5591) -- env var "false" still counts as "present" for conflict detection
