---
title: "test: Automated Live Integration Tests for Entity Store"
type: test
status: active
date: 2026-02-19
supersedes: 2026-02-17-test-live-production-validation-plan.md
related:
  - 2026-02-17-refactor-entity-store-cache-replacement-plan.md
  - 2026-02-17-entity-store-cache-redesign-brainstorm.md
---

# test: Automated Live Integration Tests for Entity Store

## Overview

A single `#[test] #[ignore]` Rust integration test that verifies the entity store
(BirdClient + BirdDb) against the live X API. One command runs all 16 verification
phases, checks off the acceptance criteria from the entity store plan, and costs
~$0.10-0.15 per run.

```bash
# Run the live test
cargo test --test live_integration -- --ignored --nocapture

# Normal test suite (live test skipped)
cargo test
```

## Problem Statement

The entity store refactor is code-complete with 156 passing unit/integration tests.
However, the plan's acceptance criteria (lines 482-509 of
`2026-02-17-refactor-entity-store-cache-replacement-plan.md`) require verification
against the live X API. The previous live validation plan was manual (8 phases, ~90
minutes, ~$8 cost). This plan automates everything into a single command with zero
manual work.

### What existing tests cover

| Layer | Count | Tests | Gaps |
|-------|-------|-------|------|
| Unit (in-memory SQLite) | 150 | Entity upsert, freshness, partition_ids, decomposition, usage migration, anti-tamper, pruning, cost estimation | No real HTTP, no real API responses |
| Integration (CLI smoke) | 6 | Arg parsing, version/help, watchlist config | No API commands, no auth, no entity store |

### What this test adds

Real HTTP calls, real API responses, real entity decomposition, real freshness
checks, real cost tracking, real DB persistence verification.

## Proposed Solution

### Architecture: Single Sequential Test

A single `#[test] #[ignore]` function with 16 sequential phases in
`tests/live_integration.rs`. This minimizes API cost (~8 API calls total) since
phases reuse entities from earlier phases.

**Why single function over independent `#[test]`s:**

- Each test would re-seed data (redundant API calls = higher cost)
- Sequential phases reuse entities from earlier phases
- `--nocapture` shows progress per-phase for clear failure diagnosis
- `#[ignore]` prevents accidental runs in normal `cargo test`

**Dependencies (all already available):**

- `assert_cmd` + `predicates` (dev-deps)
- `rusqlite` with `bundled` (regular dep, accessible to integration tests)
- `serde_json`, `dirs`, `tempfile` (regular deps)

No `Cargo.toml` changes needed.

### Test Harness: `TestEnv`

```text
TestEnv struct:
  - _tmp: TempDir (RAII cleanup)
  - home: PathBuf (fake HOME)
  - config_dir: PathBuf ($home/.config/bird/)
  - db_path: PathBuf ($home/.config/bird/bird.db)
  - auth: AuthLevel enum (None | Bearer | OAuth2User)
```

#### `TestEnv::new()`

1. Create TempDir + `$home/.config/bird/`
2. Copy `tokens.json` + `config.toml` from real `~/.config/bird/` if they exist
3. Detect auth level: check env vars first, then stored tokens
4. Return `TestEnv`

#### `TestEnv::bird() -> Command`

```rust
fn bird(&self) -> Command {
    let mut cmd = Command::cargo_bin("bird").unwrap();
    cmd.env("HOME", &self.home);
    cmd.env("XDG_CONFIG_HOME", self.home.join(".config")); // Critical: override XDG too
    cmd.env("NO_COLOR", "1");
    cmd.env_remove("BIRD_NO_CACHE");  // Ensure store is enabled
    // Pass through X_API_* env vars if set
    for key in X_API_ENV_VARS {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
    cmd
}
```

#### `TestEnv::open_db() -> rusqlite::Connection`

Opens `bird.db` in read-only mode for SQL assertions after commands complete.

### Critical Design Decisions

#### 1. Auth validation via pre-flight API call (not `bird doctor`)

**Problem discovered in first implementation attempt:** Copying `tokens.json` to a
temp dir works, but the OAuth2 access token may be expired. `ensure_access_token()`
tries to refresh via the X API, and this can fail (expired refresh token, network
issue, client_id mismatch). `bird doctor` is NOT authoritative -- it checks file
existence via `has_oauth2_available()` but never validates the token against the API.

**Solution:** Use `bird profile elonmusk` (or another cheap API call) as the
pre-flight gate. If it fails with exit code 77 (auth error), skip the test with a
clear message:

```text
SKIP: Auth failed. Run `bird login` to refresh tokens, or set X_API_BEARER_TOKEN.
```

This is Phase 1 -- the pre-flight API call that gates all subsequent phases.

#### 2. `XDG_CONFIG_HOME` must be set alongside `HOME`

**Problem:** `dirs::config_dir()` on Linux prioritizes `XDG_CONFIG_HOME` over
`$HOME/.config`. If the user's shell has `XDG_CONFIG_HOME` set, `HOME=$tmpdir` alone
does not isolate config. The bird subprocess would read/write the real config.

**Solution:** Every `TestEnv::bird()` call sets both:

- `HOME=$tmpdir`
- `XDG_CONFIG_HOME=$tmpdir/.config`

This ensures complete config isolation.

#### 3. UTC midnight boundary handling

**Problem:** Phase 3 (freshness test) asserts that a second profile lookup within the
same UTC day returns "from store". If the test runs near midnight UTC, the two calls
may span the day boundary, making the entity appear stale.

**Solution:** At the start of the freshness phase, check
`Utc::now().hour() == 23 && Utc::now().minute() >= 55`. If true, skip the freshness
assertion with a warning rather than fail intermittently.

#### 4. Synthetic `cache.db` for migration test

The usage migration test (Phase 16) needs a valid old-format `cache.db` to migrate
from. Create it in-test using `rusqlite`:

```rust
let old_conn = Connection::open(config_dir.join("cache.db")).unwrap();
old_conn.execute_batch("CREATE TABLE usage (...); INSERT INTO usage ...").unwrap();
drop(old_conn);
// Now run any bird command -- BirdClient::new() triggers migration
```

#### 5. Error-in-200 test uses multi-digit ID

`extract_single_tweet_id()` in `client.rs:106` requires `id.len() >= 2`. Use
`ids=9999999999999999999` (not `ids=0`) to ensure the entity store path is exercised.

## Phase Plan

16 phases, ~8-10 API calls, ~$0.10-0.15 total cost per run.

| Phase | AC # | Test | Auth | API Calls |
|-------|------|------|------|-----------|
| 0 | - | Environment setup + doctor baseline | None | 0 |
| 1 | - | Pre-flight auth gate (`bird profile elonmusk`) | Any | 1 |
| 2 | #1 | `bird search "twitter"` stores entities | Any | 1 |
| 3 | #2, #13 | Profile freshness + "from store" cost | Any | 0 (hit) |
| 4 | #6 | `--cache-only` serves/errors correctly | Any | 0 |
| 5 | #7 | `--refresh` skips reads, still writes | Any | 1 |
| 6 | #8 | `--no-cache` bypasses store entirely | Any | 1 |
| 7 | #3 | `bird get /2/tweets?ids=...` batch | Any | 1 |
| 8 | #4 | `bird bookmarks` stores relationships | OAuth2User | 1-2 |
| 9 | #5 | `bird thread <id>` entity lookup | Any | 1 |
| 10 | #9 | `bird cache stats` reports counts | None | 0 |
| 11 | #12 | `bird usage` works with data | None | 0 |
| 12 | #10 | `bird cache clear` preserves usage | None | 0 |
| 13 | #14 | Error-in-200 (nonexistent tweet ID) | Any | 1 |
| 14 | #15-16, #19, #21 | DB inspection (permissions, WAL, schema) | None | 0 |
| 15 | #18 | Graceful degradation (corrupt DB) | Any | 1 |
| 16 | #11 | Usage migration from cache.db (synthetic) | None | 0 |

## Phase Details

### Phase 0: Environment Setup

Create `TestEnv`, display temp HOME path, run `bird doctor` for diagnostic baseline.
Doctor is a zero-API-call command -- use its JSON output to display config state. Do
NOT use doctor for auth validation (it doesn't check token validity).

**Assertions:**

- Config dir exists
- Doctor JSON has `config` and `auth` keys

### Phase 1: Pre-Flight Auth Gate

Run `bird profile elonmusk` as a real API call to validate auth works end-to-end.

**If exit 0:** Auth works. Extract from stdout that we got valid JSON with `data.id`.
Set `auth = OAuth2User` or `Bearer` based on doctor output. Continue to Phase 2.

**If exit 77 (auth error):** Skip entire test with message:

```text
SKIP: Auth failed (exit 77). Run `bird login` to refresh tokens.
```

**If exit 1 (command error):** Likely API/network issue. Skip with message.

This phase also seeds the `elonmusk` user entity in the store for Phase 3-5.

**Assertions:**

- Exit 0
- Stdout is valid JSON with `data.id`
- Stderr contains `[cost]` and `cache miss`
- DB has user with `username = 'elonmusk'`

### Phase 2: Search Stores Entities (AC #1)

Run `bird search "twitter" --max-results 10`. Verify entities stored in DB.

**Assertions:**

- Exit 0
- Stderr contains `[cost]` + `cache miss`
- `SELECT count(*) FROM tweets` > 0
- `SELECT count(*) FROM users` > 0
- Extract tweet IDs from stdout JSON for use in Phases 7 and 9

### Phase 3: Profile Freshness + "From Store" Cost (AC #2, #13)

Run `bird profile elonmusk` again. Since Phase 1 already fetched this user within the
same UTC day, the second call should serve from the entity store.

**Midnight guard:** If `Utc::now().hour() == 23 && Utc::now().minute() >= 55`, skip
the "from store" assertion with a warning.

**Assertions:**

- Exit 0
- Stderr contains `from store`
- Stderr contains `$0.00`

### Phase 4: `--cache-only` Serves/Errors (AC #6)

Test both success and failure paths for offline mode.

**Success path:** `bird --cache-only profile elonmusk` (seeded in Phase 1).
**Failure path:** `bird --cache-only profile nonexistent_user_xyz_12345`.

**Assertions:**

- Success: exit 0, stdout has valid JSON
- Failure: exit != 0, stderr contains `not in local store`

### Phase 5: `--refresh` Skips Reads, Still Writes (AC #7)

Run `bird --refresh profile elonmusk`. The `--refresh` flag skips store reads (always
goes to API) but still writes the response to the store.

**Assertions:**

- Exit 0
- Stderr contains `cache miss` (skipped read)
- DB still has user `elonmusk` (write succeeded)

### Phase 6: `--no-cache` Bypasses Store Entirely (AC #8)

Use a **separate `TestEnv`** to verify `--no-cache` prevents DB creation.

Run `bird --no-cache search "twitter" --max-results 10` in the fresh env.

**Assertions:**

- Exit 0
- `bird.db` does NOT exist in the fresh env
- Stderr contains `[cost]` (still shows cost for real API call)

**Bonus assertion (from SpecFlow Gap 10):** Record usage count in the main env
before and after a `--no-cache` call. Assert usage count did NOT increase (because
`--no-cache` sets `db: None`, which prevents usage logging).

### Phase 7: Batch IDs via `bird get` (AC #3)

Use 3 tweet IDs from Phase 2. Run `bird get "/2/tweets?ids=ID1,ID2,ID3"`.

**Assertions:**

- Exit 0
- Stdout contains all 3 IDs
- If >= 3 IDs available; skip if search returned fewer

### Phase 8: Bookmarks Stores Relationships (AC #4)

**OAuth2User only.** Skip if auth level is Bearer.

Run `bird bookmarks`.

**Assertions:**

- Exit 0
- Open DB: `SELECT count(*) FROM bookmarks` (may be 0 if user has no bookmarks)
- If bookmarks exist: verify positions are monotonically increasing
  (`SELECT position FROM bookmarks ORDER BY position`)

### Phase 9: Thread Entity Lookup (AC #5)

Use a tweet ID from Phase 2. Run `bird thread <id>`.

Thread may fail if the tweet is not part of a conversation (exit 1). This is
acceptable -- the test verifies the command does not panic and exercises the entity
lookup code path.

**Assertions:**

- Exit 0 or 1 (no panic)
- If exit 0: stdout is valid JSON

### Phase 10: Cache Stats (AC #9)

Run `bird cache stats`. Parse JSON output.

**Assertions:**

- Exit 0
- JSON `tweets` > 0
- JSON `users` > 0
- JSON `healthy` == true

### Phase 11: Usage Works with Data (AC #12)

Run `bird usage`. Parse JSON output.

**Assertions:**

- Exit 0
- JSON `summary.total_calls` > 0

### Phase 12: Cache Clear Preserves Usage (AC #10)

Record `SELECT count(*) FROM usage` before clear.

Run `bird cache clear`. Run `bird cache stats`.

**Assertions:**

- Stderr contains `Cleared`
- Cache stats: `tweets` == 0, `users` == 0
- `SELECT count(*) FROM usage` unchanged (usage preserved)

### Phase 13: Error-in-200 (AC #14)

Run `bird get "/2/tweets?ids=9999999999999999999"`. This ID does not exist; the API
returns HTTP 200 with an `errors` array.

**Assertions:**

- Exit 0 (HTTP was 200)
- Stdout parsed as JSON
- Graceful handling (no panic, no crash)

### Phase 14: DB Inspection (AC #15-16, #19, #21)

Re-seed the DB (Phase 12 cleared it) with `bird search "twitter" --max-results 10`.
Then open `bird.db` directly with `rusqlite` for schema assertions.

**Assertions:**

- `#[cfg(unix)]`: File permissions `& 0o777 == 0o600` (AC #15)
- `PRAGMA journal_mode` == `wal` (AC #16)
- `SELECT sql FROM sqlite_master WHERE name='bookmarks'` contains `WITHOUT ROWID`
  (AC #19)
- `SELECT sql FROM sqlite_master WHERE name='tweets'` does NOT contain
  `WITHOUT ROWID` (AC #21)

### Phase 15: Graceful Degradation with Corrupt DB (AC #18)

Create a **separate `TestEnv`**. Write garbage bytes to `bird.db`. Run
`bird profile elonmusk`.

**Assertions:**

- Exit 0 (degraded to API-only mode)
- Stderr contains `[store] warning` (from `BirdClient::new()` at
  `src/db/client.rs:251`)

### Phase 16: Usage Migration from `cache.db` (AC #11)

Create a **separate `TestEnv`**. Use `rusqlite` to create a synthetic `cache.db` with
the old schema and insert test usage rows.

Run `bird cache stats` to trigger `BirdClient::new()` which calls
`migrate_usage_from_cache()`.

**Assertions:**

- Stderr contains `migrated usage data from cache.db`
- `SELECT count(*) FROM usage` > 0 in `bird.db`

Run `bird cache stats` again (idempotency check).

**Assertions:**

- Stderr does NOT contain `migrated usage data` on second run
- Usage count unchanged

## Key Implementation Notes

### Environment isolation

Every `TestEnv::bird()` call sets:

- `HOME=$tmpdir`
- `XDG_CONFIG_HOME=$tmpdir/.config` (prevents `dirs::config_dir()` leak)
- `NO_COLOR=1` (deterministic stderr parsing)
- `BIRD_NO_CACHE` removed (ensure store enabled)

### Credential propagation

Copy `tokens.json` + `config.toml` from real `~/.config/bird/`. Also pass through
`X_API_*` env vars for users who authenticate via environment.

### Assertion format

Use `assert!(cond, "AC #N: description")` for traceability back to acceptance
criteria.

### Progress output

```rust
eprintln!("=== Phase N: description ===")
```

Per phase for `--nocapture` diagnosis.

### Cost management

~8-10 API calls, ~$0.10-0.15 per run. Document at the top of the test file.

### Rate limits

~10 requests per run is well within all endpoint limits (300/15min for search,
900/15min for profile). Safe to run up to ~30 times per 15-minute window.

## Output Format

### With auth

```text
=== Live Integration Test: Entity Store ===
Auth level: OAuth2User (stored tokens)

=== Phase 0: Environment setup ===
  Temp HOME: /tmp/.tmpXXXXXX
  Config dir: /tmp/.tmpXXXXXX/.config/bird
=== Phase 1: Pre-flight auth gate ===
  profile elonmusk: exit 0, cache miss  PASS
=== Phase 2: Search stores entities (AC #1) ===
  tweets: 10, users: 8  PASS
...
=== Phase 16: Usage migration (AC #11) ===
  Migrated 3 usage rows, idempotent  PASS
=== 16/16 phases passed ===
```

### Without auth

```text
=== Live Integration Test: Entity Store ===
Auth level: None
SKIP: No API credentials available.
  Set X_API_BEARER_TOKEN or run `bird login`.
test live_integration ... ok
```

### With expired auth

```text
=== Phase 1: Pre-flight auth gate ===
  profile elonmusk: exit 77 (auth error)
SKIP: Auth failed. Run `bird login` to refresh tokens.
test live_integration ... ok
```

## Acceptance Criteria Coverage

After this test passes, all acceptance criteria from the entity store plan are
verified:

| AC # | Criterion | Verified By |
|------|-----------|-------------|
| 1 | Search stores entities | Phase 2 |
| 2 | Profile freshness (UTC day) | Phase 3 |
| 3 | Batch ID splitting + merge | Phase 7 |
| 4 | Bookmarks stores relationships | Phase 8 |
| 5 | Thread entity lookup | Phase 9 |
| 6 | `--cache-only` | Phase 4 |
| 7 | `--refresh` | Phase 5 |
| 8 | `--no-cache` | Phase 6 |
| 9 | `bird cache stats` | Phase 10 |
| 10 | `bird cache clear` preserves usage | Phase 12 |
| 11 | Usage migration from cache.db | Phase 16 |
| 12 | `bird usage` works | Phase 11 |
| 13 | "From store" cost display | Phase 3 |
| 14 | Error-in-200 handling | Phase 13 |
| 15 | DB permissions 0o600 | Phase 14 |
| 16 | WAL mode | Phase 14 |
| 17 | Anti-tamper (triggers/views) | Unit test `anti_tamper_rejects_views` |
| 18 | Graceful degradation | Phase 15 |
| 19 | `WITHOUT ROWID` on bookmarks | Phase 14 |
| 20 | `PRAGMA optimize` on close | Unit test (Drop impl) |
| 21 | No FK on tweets | Phase 14 |
| 22 | Pruning | Unit test `pruning_raw_responses_by_age` |

## Files

| File | Action | Lines |
|------|--------|-------|
| `tests/live_integration.rs` | **New** | ~450-550 |
| `Cargo.toml` | No changes | - |

## Technical Considerations

### Hardest decision

**Auth validation strategy.** Three options were considered:

1. **`bird doctor` check** -- Rejected. Doctor only checks file existence via
   `has_oauth2_available()`, not token validity. This was the approach used in the
   first implementation attempt and caused a confusing failure.
2. **Require `X_API_BEARER_TOKEN` env var** -- Rejected. Forces users to set up an
   app-only token, which many won't have.
3. **Pre-flight API call as gate** -- Chosen. Run `bird profile elonmusk` first. If
   it succeeds, auth works. If exit 77, skip gracefully. Simple, reliable, tests the
   real auth flow.

### Alternatives rejected

1. **Independent `#[test]` per phase** -- Rejected. Each test would re-seed data,
   costing ~$0.30+ per run instead of ~$0.12. No shared state between tests.

2. **Mock HTTP layer** -- Rejected. The entire point is testing against the live API.
   Mocks would duplicate the 150 existing unit tests without adding confidence.

3. **Shell script wrapper** -- Rejected. Rust integration test gets `#[ignore]`
   protection, `assert_cmd` ergonomics, direct `rusqlite` access for DB assertions,
   and runs with `cargo test`.

### Where least confident

1. **Token refresh reliability** -- If stored tokens expire and refresh fails, the
   test skips entirely. This is the correct behavior but means the test requires a
   recent `bird login`. Documented in output.

2. **UTC midnight boundary** -- The midnight guard (skip if >= 23:55 UTC) is
   pragmatic but not perfect. A very slow test run could still span midnight.

3. **Bookmarks phase** -- May have 0 bookmarks if the authenticated user has none.
   The test treats this as acceptable (verifies the command works, just not the
   relationship storage).

## References

### Internal

- Entity store plan: `docs/plans/2026-02-17-refactor-entity-store-cache-replacement-plan.md` (lines 482-509 for AC)
- Brainstorm: `docs/brainstorms/2026-02-17-entity-store-cache-redesign-brainstorm.md`
- Previous manual validation: `docs/plans/2026-02-17-test-live-production-validation-plan.md`
- Existing smoke tests: `tests/cli_smoke.rs`

### Key source files

| File | Relevant Code |
|------|---------------|
| `src/auth.rs:376-424` | `ensure_access_token()` -- token refresh flow |
| `src/auth.rs:361-365` | `has_oauth2_available()` -- file existence check (NOT validity) |
| `src/config.rs:117` | `dirs::config_dir()` -- affected by `XDG_CONFIG_HOME` |
| `src/config.rs:178` | `BIRD_NO_CACHE` env var check |
| `src/db/client.rs:220-254` | `BirdClient::new()` -- graceful degradation |
| `src/db/client.rs:250-254` | Store open failure → `None` with warning |
| `src/db/db.rs:298-382` | `migrate_usage_from_cache()` |
| `src/db/db.rs:723-738` | `clear()` -- preserves usage |
| `src/cost.rs:77-119` | `display_cost()` -- stderr format |
| `src/main.rs:592-608` | `cache stats` JSON output shape |

### Documented gotchas (from `docs/solutions/` and live validation)

1. `bird doctor` checks file existence, not token validity
2. `XDG_CONFIG_HOME` takes priority over `$HOME/.config` in `dirs` crate
3. `--no-cache` disables usage tracking too (`db: None`)
4. Error-in-200 responses (HTTP 200 with `errors` array) are stored in entity store
5. Non-2xx responses are NOT stored as entities
6. `extract_single_tweet_id()` requires `id.len() >= 2`
7. `bird cache clear` preserves `usage` and `usage_actual` tables
