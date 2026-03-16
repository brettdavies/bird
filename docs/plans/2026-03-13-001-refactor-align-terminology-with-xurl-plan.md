---
title: "refactor: Align bird terminology with xurl"
type: refactor
status: completed
date: 2026-03-13
origin: docs/reviews/2026-03-12-xurl-transport-review.md
---

# Align Bird Terminology with xurl

## Overview

Bird wraps xurl as a subprocess but uses inconsistent terminology for the same concepts. xurl is the source of truth — bird should align with its naming. The primary issue is the `--account` CLI flag and `account` variable names, which xurl calls `--username` / `-u`. There are also secondary inconsistencies in display labels, config comments, env var naming, and database columns.

## Problem Statement

### xurl's Terminology (Source of Truth)

| Concept | xurl term | xurl flag | xurl help text |
|---------|-----------|-----------|----------------|
| Multi-user token selection | **username** | `-u, --username` | "OAuth2 username to act as" |
| Auth type selection | auth | `--auth` | "Authentication type (oauth1, oauth2, app)" |
| App selection | app | `--app` | "Use a specific registered app" |
| Current authenticated identity | user | `xurl whoami` | "Show the authenticated user's profile" |
| User lookup | user | `xurl user USERNAME` | "Fetch profile information for any user" |
| Default selection | `auth default` | `xurl auth default [APP [USERNAME]]` | "Set the default app and/or OAuth2 user" |

### Bird's Current Terminology (Inconsistent)

| Location | Current term | Should be |
|----------|-------------|-----------|
| CLI flag `--account` | account | **username** |
| `Cli.account` struct field | account | **username** |
| `config.username` field | username | username (correct) |
| `ArgOverrides.username` field | username | username (correct) |
| `X_API_USERNAME` env var | username | username (correct) |
| `BirdClient.account` field | account | **username** |
| `xurl_write_call` param `account` | account | **username** |
| `let account = config.username.as_deref()` (14x) | account | **username** |
| `account_username` DB column | account_username | **username** |
| `BookmarkRow.account_username` | account_username | **username** |
| `db.replace_bookmarks(account, ...)` param | account | **username** |
| `db.get_bookmarks(account)` param | account | **username** |
| CLI help: "Account name for multi-account token selection" | account | **username** |
| doctor pretty output: `user: @{username}` label | user | username (align with field name) |
| watchlist docs: "manage accounts" / "add an account" | accounts | **users** |
| config.example.toml comment: "which stored account to use" | account | **username** / user |
| DEVELOPER.md: "which stored account to use" | account | **username** |
| CLI smoke tests: `account_invalid_chars_rejected` | account | **username** |

### What's Already Correct

- `config.rs`: `ResolvedConfig.username`, `FileConfig.username`, `ArgOverrides.username` — all correct
- `config.toml` key: `username = "your_handle"` — correct
- `X_API_USERNAME` env var — correct
- `requirements.rs`: `auth_flag()` mapping — correct (uses xurl's `--auth` terminology)
- `doctor.rs`: `AuthState.username` field — correct
- `schema::validate_username()` — correct

## Proposed Solution

Rename `account` → `username` everywhere it refers to the xurl `-u` / `--username` concept. Update watchlist docs to say "users" instead of "accounts". Update config comments. This is a mechanical rename — no behavioral changes.

### Commit Structure

Two commits following SRP:

1. **`refactor: rename --account to --username for xurl alignment`** — CLI flag, struct fields, variable names, function params, tests, help text, docs
2. **`refactor: rename account_username to username in bookmarks schema`** — DB column, struct field, migration, related function params

Commit 2 is separated because it involves a database migration (schema change), while commit 1 is purely code rename.

## Technical Approach

### Commit 1: Rename `--account` → `--username` across codebase

#### `src/main.rs`

**CLI flag:**

```rust
// Before:
/// Account name for multi-account token selection (maps to xurl -u)
#[arg(long, global = true)]
account: Option<String>,

// After:
/// Username for multi-user token selection (maps to xurl -u)
#[arg(long, short = 'u', global = true)]
username: Option<String>,
```

Note: Adding `-u` short flag to match xurl's `-u` flag.

**Variable renames (14 write command sites):**

```rust
// Before (repeated 14x in write command handlers):
let account = config.username.as_deref();
xurl_write_call(&["tweet", &text], account)?;

// After:
let username = config.username.as_deref();
xurl_write_call(&["tweet", &text], username)?;
```

**`xurl_write_call` parameter:**

```rust
// Before:
fn xurl_write_call(
    args: &[&str],
    account: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

// After:
fn xurl_write_call(
    args: &[&str],
    username: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
```

**Validation block:**

```rust
// Before:
let cli_account = match cli.account { ... }
    Err(e) => { BirdError::Config(format!("--account: {}", e).into()); }
let overrides = ArgOverrides { username: cli_account, ... };

// After:
let cli_username = match cli.username { ... }
    Err(e) => { BirdError::Config(format!("--username: {}", e).into()); }
let overrides = ArgOverrides { username: cli_username, ... };
```

- [ ] Rename `Cli.account` field → `Cli.username`
- [ ] Add `short = 'u'` to match xurl's `-u` flag
- [ ] Update help text: "Username for multi-user token selection (maps to xurl -u)"
- [ ] Rename `xurl_write_call` param `account` → `username`
- [ ] Rename all 14 `let account = config.username.as_deref()` → `let username = ...`
- [ ] Update validation block: `cli_account` → `cli_username`, error message `--account:` → `--username:`
- [ ] Update `xurl_write` function param `account` usage if needed

#### `src/db/client.rs`

```rust
// Before:
/// Username for --account flag (maps to xurl -u)
account: Option<String>,

// After:
/// Username for xurl -u flag (multi-user token selection)
username: Option<String>,
```

- [ ] Rename `BirdClient.account` field → `username`
- [ ] Update comment to remove "--account" reference
- [ ] Update constructor parameter names
- [ ] Update `self.account` references → `self.username` (2 sites in `request()` and `build_get_args`)

#### `src/watchlist.rs`

- [ ] Module doc: "manage and check a curated list of X/Twitter accounts" → "manage and check a curated list of X users"
- [ ] Function docs: "add an account to the watchlist" → "add a user to the watchlist" (and "remove")
- [ ] Error messages: "Add accounts with: bird watchlist add <username>" → "Add users with: bird watchlist add <username>"
- [ ] Streaming doc: "per account as they complete" → "per user as they complete"

#### `src/main.rs` — Watchlist command docs

- [ ] "Monitor accounts" → "Monitor users" in Watchlist command doc
- [ ] "Check recent activity for all watched accounts" → "...all watched users"
- [ ] "Add an account to the watchlist" → "Add a user to the watchlist"
- [ ] "Remove an account from the watchlist" → "Remove a user from the watchlist"

#### `src/doctor.rs` — Display label

- [ ] Pretty output label `"  user: {}\n"` → `"  username: {}\n"` (align with field name and xurl terminology)

#### Docs

- [ ] `config.example.toml`: comment "which stored account to use" → "which xurl username to use for multi-user token selection"
- [ ] `docs/DEVELOPER.md`: same comment update

#### Tests

- [ ] `tests/cli_smoke.rs`: rename `account_invalid_chars_rejected` → `username_invalid_chars_rejected`
- [ ] `tests/cli_smoke.rs`: rename `account_at_prefix_normalized` → `username_at_prefix_normalized`
- [ ] Update test assertions from `--account` → `--username` in args and predicates

### Commit 2: Rename `account_username` → `username` in bookmarks schema

#### `src/db/db.rs`

Database migration to rename column:

```sql
ALTER TABLE bookmarks RENAME COLUMN account_username TO username;
```

- [ ] Add migration renaming `account_username` → `username` in bookmarks table
- [ ] Update `BookmarkRow.account_username` field → `username`
- [ ] Update `replace_bookmarks` param name `account` → `username`
- [ ] Update `get_bookmarks` param name `account` → `username`
- [ ] Update all SQL strings referencing `account_username` → `username`
- [ ] Update `PRIMARY KEY (account_username, tweet_id)` — handled by migration (composite key stays, just column name changes)

#### `src/bookmarks.rs`

- [ ] Update `account_username: me_username.clone()` → `username: me_username.clone()`

## Out of Scope

- **`X_API_USERNAME` env var** — already correct, no change needed
- **`config.toml` key `username`** — already correct
- **`AuthState.username` field** — already correct
- **`schema::validate_username()`** — already correct
- **xurl subcommand names** (post, reply, like, etc.) — bird already matches xurl's names
- **`--auth` flag mapping** — already correct in requirements.rs
- **Renaming the `bird profile` command** — xurl calls this `xurl user`, but bird's `profile` is a semantic choice (it fetches profile data). Could be a future alignment but is a separate discussion.

## Acceptance Criteria

- [ ] `bird --username alice tweet "hello"` passes `-u alice` to xurl (was `--account`)
- [ ] `bird -u alice tweet "hello"` works (new short flag)
- [ ] `bird --account ...` produces "unknown flag" error (clean break, no deprecation)
- [ ] `--username "bad!chars"` produces config error (exit 78)
- [ ] `--username "@alice"` normalizes to `alice`
- [ ] All variable names use `username` (no `account` for this concept)
- [ ] `bird watchlist list` help says "users" not "accounts"
- [ ] `bird doctor --pretty` shows `username:` not `user:`
- [ ] Bookmarks table uses `username` column (migration applied transparently)
- [ ] All existing tests pass (`cargo test`)
- [ ] No new compiler warnings (`cargo build`)

## Dependencies & Risks

| Risk | Mitigation |
|------|------------|
| `--account` is a breaking CLI change | Clean break — no deprecation alias. This is pre-1.0, and `--account` has never been in a release. Document in PR description. |
| SQLite column rename needs migration | `ALTER TABLE ... RENAME COLUMN` is supported in SQLite 3.25+ (2018). All modern systems have this. Migration is idempotent — if column already renamed, migration is a no-op (add guard). |
| Short flag `-u` conflicts | Check: no existing `-u` flag in bird CLI. Confirmed — no conflict. |
| Config file `username` key unchanged | Correct — `username` was already the right name. No config migration needed. |
| `X_API_USERNAME` env var unchanged | Correct — already aligned with xurl. |

## Commit Plan

```text
Commit 1: refactor: rename --account to --username for xurl alignment
  src/main.rs           — CLI flag, help text, variable renames, validation
  src/db/client.rs      — BirdClient field rename
  src/watchlist.rs      — doc comments and error messages
  src/doctor.rs         — display label
  config.example.toml   — comment update
  docs/DEVELOPER.md     — comment update
  tests/cli_smoke.rs    — test renames

Commit 2: refactor: rename account_username to username in bookmarks
  src/db/db.rs          — migration, struct field, SQL, function params
  src/bookmarks.rs      — field assignment
```
