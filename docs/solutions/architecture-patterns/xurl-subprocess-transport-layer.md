---
title: "Replace reqwest with xurl subprocess transport layer"
category: architecture-patterns
date: 2026-03-16
tags:
  - transport-layer
  - xurl
  - subprocess
  - auth-delegation
  - dependency-reduction
  - write-commands
  - passthrough
  - entity-store
  - terminology-alignment
problem_type: architectural-refactor
components:
  - transport.rs
  - auth.rs (deleted)
  - login.rs (deleted)
  - doctor.rs
  - main.rs
  - db/client.rs
  - config.rs
  - requirements.rs
  - schema.rs
severity: high
pr: 8
branch: refactor/xurl-transport-layer
---

# Replace reqwest with xurl Subprocess Transport Layer

## Problem

The bird CLI embedded its own HTTP transport (reqwest + tokio) and OAuth implementation (reqwest-oauth1, custom PKCE flow in `auth.rs`, browser launcher in `login.rs`). This created several architectural problems:

1. **Duplicated auth complexity** — Bird reimplemented OAuth2 PKCE (with CSRF state, code verifier, local HTTP callback server) and OAuth1 signing, duplicating what xurl already handles correctly.
2. **Heavy dependency tree** — reqwest, tokio, hyper, rustls, and transitive deps pulled in 303 crates, bloating compile times and binary size.
3. **Two sources of truth for X API compatibility** — Both bird and xurl tracked X API changes independently; auth bugs had to be fixed in both.
4. **Async runtime for a synchronous CLI** — tokio was required only because reqwest is async, adding complexity to a fundamentally sequential CLI tool.

## Root Cause / Motivation

xurl is the official X Developer Platform CLI tool. It already handles OAuth2 PKCE flows, OAuth1 signing, token storage (`~/.xurl`), and X API compatibility. Bird was duplicating all of this transport-layer work instead of delegating to xurl.

**Key insight: Bird should own the intelligence layer (entity store, caching, cost tracking, UX); xurl should own the transport layer (auth, HTTP, X API compatibility).**

## Solution

### Phase 1: Add transport layer alongside existing code

Introduced `src/transport.rs` (500 lines) with three entry points:

- `xurl_call(&[&str])` — spawn xurl, capture stdout as JSON, classify errors
- `xurl_passthrough(&[&str])` — spawn xurl with inherited stdio (for `bird login` interactive OAuth flow)
- `resolve_xurl_path()` — cached binary resolution via `BIRD_XURL_PATH` env var or `which::which`

Also decoupled `ApiResponse` from reqwest types (changed `.status` from `reqwest::StatusCode` to `u16`, removed `.headers` field).

### Phase 2: Migrate all commands to transport layer

Replaced `reqwest::Client` with `Box<dyn Transport>` in `BirdClient`. All command handlers simplified: removed async, removed auth token resolution, reduced to `client.get(url, &ctx)` / `client.request(method, url, ctx, body)`.

Deleted `auth.rs` (504 lines) and `login.rs` (166 lines). Login now delegates to `xurl auth oauth2` via passthrough.

### Phase 3: Remove old code, add write commands, polish

- Dropped 7 dependencies: reqwest, tokio, base64, rand, webbrowser, percent-encoding, reqwest-oauth1
- Added 13 write commands (tweet, reply, like/unlike, repost/unrepost, follow/unfollow, dm, block/unblock, mute/unmute) via `xurl_write_call`
- Rebuilt `doctor.rs` around `xurl version` and `xurl whoami`
- Renamed `--account` to `--username` / `-u` to match xurl's conventions
- Migrated `account_username` DB column to `username` (SQLite ALTER TABLE RENAME COLUMN)

## Key Code Patterns

### Transport trait for testability

```rust
pub trait Transport {
    fn request(&self, args: &[String])
        -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct XurlTransport;  // delegates to xurl_call
pub struct MockTransport {  // pre-configured responses for tests
    pub responses: RefCell<VecDeque<Result<serde_json::Value, ...>>>,
}
```

The trait boundary is at the right abstraction level — arg-building logic (auth flags, `-u`, URL) runs in tests while subprocess execution is mocked.

### Security invariants: no shell, no secrets in argv

```rust
let mut child = Command::new(path)
    .args(args)           // execvp directly, no shell interpretation
    .env("NO_COLOR", "1") // suppress ANSI in subprocess output
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
```

Tokens are never passed as arguments — xurl reads auth from `~/.xurl`.

### Two-layer error classification

1. `classify_error()` in `transport.rs` parses xurl's JSON error response into `XurlError` variants (Auth, Api, Process, Timeout)
2. `map_cmd_error()` in `main.rs` downcasts to detect auth errors and map to exit code 77:

```rust
fn map_cmd_error(name: &'static str, e: Box<dyn Error + Send + Sync>) -> BirdError {
    if let Some(xurl_err) = e.downcast_ref::<transport::XurlError>() {
        if matches!(xurl_err, transport::XurlError::Auth(_)) {
            return BirdError::Auth(e);
        }
    }
    BirdError::Command { name, source: e }
}
```

### Startup path validation with OnceLock

`resolve_xurl_path()` validates the xurl binary once at startup: exists → canonicalize (resolve symlinks) → is_file → is_executable. Cached in `OnceLock` for process lifetime.

### ANSI stripping with zero-alloc fast path

```rust
pub fn strip_ansi_lines(s: &str) -> Cow<'_, str> {
    if !s.contains('\x1b') { return Cow::Borrowed(s); }
    Cow::Owned(s.lines().filter(|l| !l.contains('\x1b')).collect::<Vec<_>>().join("\n"))
}
```

### ETXTBSY retry for integration tests

Tests that create mock shell scripts handle the kernel race condition where write and exec overlap:

```rust
fn exec_mock(cmd: &mut Command) -> std::process::Output {
    match cmd.output() {
        Ok(out) => out,
        Err(e) if e.raw_os_error() == Some(26) => {
            std::thread::sleep(Duration::from_millis(50));
            cmd.output().unwrap()
        }
        Err(e) => panic!("failed: {}", e),
    }
}
```

## Results

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Direct dependencies | ~21 | 17 | -7 +3 (net -4) |
| Crates in Cargo.lock | 303 | 166 | -137 (45% reduction) |
| Auth/transport code | 1,683 lines | 0 | -1,683 deleted |
| New transport.rs | 0 | 500 lines | +500 |
| Async runtime | Required (tokio) | None | Eliminated |
| Write commands | 0 | 13 | +13 for free via passthrough |
| Tests passing | — | 179 (162 unit + 8 CLI + 9 transport) | All green |

## Bugs Caught in Review

Six bugs were caught during the multi-agent code review, each representing a class of issue to watch for in future refactors:

### 1. Lexicographic version comparison

**Bug:** `"1.0.9" > "1.0.10"` is true in string comparison. **Fix:** Use `semver::Version::parse()` for correct multi-digit ordering.

**Rule:** Never compare version strings with `<`/`>`/`==`. Always use a semantic versioning library.

### 2. Exit code 77 silently lost

**Bug:** Removing `BirdError::Auth` changed auth failure exit code from 77 to 1. Scripts relying on `$? -eq 77` would break. **Fix:** Restore `BirdError::Auth` variant, add `map_cmd_error()` to detect auth errors via downcast.

**Rule:** Exit codes are public API. Write contract tests for them *before* refactoring.

### 3. --account flag not forwarded to write commands

**Bug:** Read commands forwarded the flag via `BirdClient`; write commands called `xurl_call` directly without it. Posts went to wrong user. **Fix:** Centralized flag injection in `xurl_write_call()`.

**Rule:** Global flags that affect identity must flow through a single chokepoint function.

### 4. BIRD_XURL_PATH not validated

**Bug:** Directory paths, broken symlinks, and non-executable files produced confusing errors at spawn time. **Fix:** Validate at startup: exists → canonicalize → is_file → is_executable.

**Rule:** Validate external binary paths at the earliest possible moment with specific diagnostics for each failure mode.

### 5. Unnecessary json.clone() in hot paths

**Bug:** `get()` and `batch_get()` cloned full JSON responses where a borrow sufficed. **Fix:** Changed `decompose_and_upsert` to accept `&serde_json::Value`.

**Rule:** Before cloning large types, check if a reference suffices. Reserve `.clone()` for genuine ownership transfer.

### 6. Duplicate validate_username across modules

**Bug:** Both `profile.rs` and `watchlist.rs` had near-identical validation. **Fix:** Extracted to `schema::validate_username()`, both modules import.

**Rule:** STAR principle — shared validation must live in one canonical module.

## Refactoring Checklist

Use when performing architectural refactors that change transport, error handling, or command dispatch:

### Pre-Refactor

- [ ] Identify all behavioral contracts (exit codes, CLI flags, JSON output)
- [ ] Write contract tests first (before removing any code)
- [ ] Record baseline test count
- [ ] Check `docs/solutions/` for prior art
- [ ] Catalog all enum variants and functions being removed; search for all consumers

### During Refactor

- [ ] Global flags flow through a single chokepoint function
- [ ] Version strings use domain-specific comparison (semver crate)
- [ ] External binary paths validated at startup with specific diagnostics
- [ ] No `.clone()` on large types without justification
- [ ] Shared validation in one canonical module
- [ ] Error variants preserve exit code semantics
- [ ] Security invariants documented at module level

### Post-Refactor

- [ ] Test count did not decrease
- [ ] All contract tests pass
- [ ] `cargo clippy -- -D warnings` passes clean
- [ ] No duplicate logic across modules
- [ ] Integration tests cover failure modes (bad paths, auth failures, timeouts)

## Cross-References

### Origin Chain

- [Brainstorm: Wrap xurl vs Native API Client](../../brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md) — Architecture A decision, coverage comparison
- [Plan: xurl transport layer](../../plans/2026-03-12-001-refactor-xurl-transport-layer-plan.md) — 3-phase implementation plan, Transport trait design
- [Review: xurl transport layer](../../reviews/2026-03-12-xurl-transport-review.md) — 19 findings, 6-agent review
- [Plan: Review findings](../../plans/2026-03-12-002-fix-xurl-transport-review-findings-plan.md) — semver, exit code 77, Cow<str>, validate_username
- [Plan: Terminology alignment](../../plans/2026-03-13-001-refactor-align-terminology-with-xurl-plan.md) — --account to --username rename

### Related Solutions

- [Security audit](../security-issues/rust-cli-security-code-quality-audit.md) — BirdError enum, exit codes 78/77/1, token file permissions
- [Code review round 2](code-review-round2-quality-improvements.md) — sanitize_for_stderr, "public methods log" rule
- [SQLite cache layer](../performance-issues/sqlite-cache-layer-api-cost-reduction.md) — entity store architecture, graceful degradation
- [Live integration testing](live-integration-testing-cli-external-api.md) — TestEnv pattern, exit code gotchas
- [Thread command pattern](thread-command-tree-reconstruction-pattern.md) — validate_username origin
- [Search command pattern](search-command-paginated-api-pattern.md) — pagination via next_token, auth constant patterns
