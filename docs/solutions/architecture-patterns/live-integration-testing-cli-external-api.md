---
title: "Live Integration Testing for CLI with External API"
category: architecture-patterns
date: 2026-03-11
tags:
  - rust
  - integration-test
  - sqlite
  - entity-store
  - x-api
  - live-testing
  - test-isolation
related_files:
  - tests/live_integration.rs
  - docs/plans/2026-02-19-test-automated-live-entity-store-integration-plan.md
  - docs/plans/2026-02-17-refactor-entity-store-cache-replacement-plan.md
  - src/db/client.rs
  - src/db/db.rs
  - src/auth.rs
  - src/config.rs
---

# Live Integration Testing for CLI with External API

## Problem

The entity store refactor (BirdClient + BirdDb replacing CachedClient) was code-complete with 156 passing unit/integration tests, but all 22 acceptance criteria required verification against the live X API. Unit tests used in-memory SQLite with no HTTP; smoke tests covered only CLI parsing. No automated test harness existed that could isolate config/DB state, validate real auth credentials, and exercise the full entity store lifecycle against the real API.

The previous manual validation plan would take ~90 minutes and cost ~$8 per run.

## Root Cause

Unit tests and integration smoke tests test different things than live API behavior. In-memory SQLite tests verify SQL correctness but not API response shapes. Smoke tests verify arg parsing but not auth flows or entity decomposition. Five specific gaps had no unit test coverage: XDG config isolation, token validity vs file existence, UTC midnight boundary races, migration idempotency against real file state, and graceful degradation with actual corrupt files.

## Solution

A single `#[test] #[ignore]` function in `tests/live_integration.rs` with 16 sequential phases. Run with:

```bash
cargo test --test live_integration -- --ignored --nocapture
```

Cost: ~$0.10-0.15 per run (~8-10 API calls). Normal `cargo test` skips it.

### Pattern 1: TestEnv — Full Environment Isolation

The `dirs` crate on Linux checks `$XDG_CONFIG_HOME` before `$HOME/.config`. Setting only `HOME` leaks the real config directory.

```rust
fn bird(&self) -> Command {
    let mut cmd = Command::cargo_bin("bird").unwrap();
    cmd.env("HOME", &self.home);
    cmd.env("XDG_CONFIG_HOME", self.home.join(".config")); // Critical
    cmd.env("NO_COLOR", "1");
    cmd.env_remove("BIRD_NO_CACHE");
    // Pass through X_API_* env vars for users who auth via environment
    for key in X_API_ENV_VARS {
        if let Ok(val) = std::env::var(key) { cmd.env(key, val); }
    }
    cmd
}
```

Copy real credentials (tokens.json, config.toml) into the temp dir so the CLI subprocess can authenticate, while keeping all state (DB, config writes) isolated.

### Pattern 2: Pre-Flight Auth Gate (Not Doctor)

`bird doctor` calls `has_oauth2_available()` which checks **file existence** only — never validates the token against the API. Expired or revoked tokens pass doctor.

Use a cheap real API call as the gate:

```rust
let output = env.bird().args(["profile", "elonmusk"]).output()?;
if exit_code != 0 {
    let is_auth = exit_code == 77
        || stderr.contains("no valid auth")   // AuthRequiredError boxed in Command error
        || stderr.contains("auth failed");
    if is_auth {
        eprintln!("SKIP: Auth not available.");
    }
    return; // Skip entire test gracefully
}
```

Check both exit code 77 (direct `AuthRequiredError`) AND exit code 1 with auth-related stderr, because `AuthRequiredError` wrapped in `Box<dyn Error>` loses its exit code mapping.

### Pattern 3: UTC Midnight Guard

Freshness assertions ("fetched today should hit store") are non-deterministic near midnight UTC:

```rust
let skip_freshness = {
    use chrono::Timelike;
    now.hour() == 23 && now.minute() >= 55
};
if !skip_freshness {
    assert!(stderr.contains("from store"), "AC #2: ...");
}
```

Skip time-sensitive assertions rather than produce intermittent failures.

### Pattern 4: Separate TestEnvs for Destructive Tests

Tests that write, modify, or corrupt persistent state need their own isolated environments:

- **Phase 6** (`--no-cache`): Fresh TestEnv to assert `bird.db` is never created
- **Phase 15** (corrupt DB): Pre-written garbage bytes to test graceful degradation
- **Phase 16** (usage migration): Synthetic `cache.db` seeded before first bird command

```rust
// Phase 15: Corrupt DB graceful degradation
let env_corrupt = TestEnv::new();
std::fs::write(&env_corrupt.db_path, b"NOT A VALID SQLITE DB").unwrap();
let output = env_corrupt.bird().args(["profile", "elonmusk"]).output()?;
assert!(stderr.contains("[store] warning")); // Degraded to API-only
```

### Pattern 5: Direct DB Assertions via rusqlite

Open the DB read-only after CLI commands complete for schema-level assertions:

```rust
fn open_db(&self) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(
        &self.db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).expect("open bird.db")
}

// Assert schema properties
let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
assert_eq!(journal_mode, "wal");

let sql: String = conn.query_row(
    "SELECT sql FROM sqlite_master WHERE name='bookmarks'", [], |r| r.get(0)
)?;
assert!(sql.to_uppercase().contains("WITHOUT ROWID"));
```

### Pattern 6: Robust Output Parsing

CLI output format may vary (full API envelope, per-line objects). Parse defensively:

```rust
fn extract_tweet_ids(stdout: &str) -> Vec<String> {
    let mut ids = Vec::new();
    // Try whole stdout as single JSON first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
        collect_ids(&v, &mut ids);
        if !ids.is_empty() { return ids; }
    }
    // Fall back to line-by-line
    for line in stdout.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            collect_ids(&v, &mut ids);
        }
    }
    ids
}
```

## Documented Gotchas

1. `bird doctor` checks file existence, not token validity. Never use it as an auth gate for tests.
2. `dirs::config_dir()` on Linux reads `$XDG_CONFIG_HOME` before `$HOME/.config`. Always set both.
3. `--no-cache` disables usage tracking too (sets `db: None`). Usage count must not change.
4. `AuthRequiredError` boxed inside `Box<dyn Error>` returns exit code 1, not 77. Check stderr content to distinguish auth failures from other command errors.
5. `extract_single_tweet_id()` in `client.rs` requires `id.len() >= 2`. Use multi-digit IDs for error-in-200 tests.
6. `bird cache clear` preserves `usage` and `usage_actual` tables.
7. SQLite `Connection::open()` succeeds on corrupt files — the error surfaces on the first PRAGMA/query.

## Prevention Strategies

- **Live gate for new commands**: Before merging a new API command, require at least one live integration test phase that exercises the real endpoint.
- **Canonical TestEnv builder**: All integration tests should use a shared `TestEnv` struct that sets HOME + XDG_CONFIG_HOME + passes through env vars. No piecemeal env var setting.
- **Skip vs fail**: Tests depending on external resources (API, credentials, network) skip gracefully when unavailable. Failure is reserved for "resource available, behavior wrong."
- **Sequential test for cost control**: Single `#[test]` with sequential phases reuses seeded data across assertions, minimizing API calls. Independent `#[test]` functions each re-seed, multiplying cost.
- **Typed error enums for exit codes**: Never rely on downcasting `Box<dyn Error>` for exit code assignment. Use `BirdError` variants with explicit `exit_code()` methods.

## Cross-References

- Entity store plan: `docs/plans/2026-02-17-refactor-entity-store-cache-replacement-plan.md`
- Live test plan: `docs/plans/2026-02-19-test-automated-live-entity-store-integration-plan.md`
- Original cache solution: `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
- Manual validation (superseded): `docs/plans/2026-02-17-test-live-production-validation-plan.md`
