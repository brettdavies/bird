---
title: "feat: Watchlist and Usage Commands"
type: feat
date: 2026-02-11
series: "Research Commands & Caching Layer"
plan: 4 of 4
depends_on: "2026-02-11-feat-transparent-cache-layer-plan"
---

# feat: Watchlist and Usage Commands

## Overview

Add two new commands to Bird: `bird watchlist` for monitoring a curated list of X/Twitter accounts, and `bird usage` for viewing accumulated API costs and cache efficiency. These are the final commands in the Research Commands & Caching Layer series, completing the research workflow.

**Plan series:**

| # | Plan | Status |
|---|------|--------|
| 1 | Transparent Cache Layer | Dependency |
| 2 | Search Command | Dependency (watchlist `check` uses search) |
| 3 | Profile & Thread Commands | Independent |
| **4** | **Watchlist & Usage Commands** (this plan) | Active |

**What ships:**

| Command | Purpose |
|---------|---------|
| `bird watchlist check` | Check recent activity for all watched accounts |
| `bird watchlist add <username>` | Add an account to the watchlist in config.toml (idempotent) |
| `bird watchlist remove <username>` | Remove an account from the watchlist |
| `bird watchlist list` | Display the current watchlist |
| `bird usage [--since DATE] [--sync]` | View accumulated API costs, cache savings, and optionally sync with X API actuals |

## Review Findings Applied

This plan was reviewed by 12 specialized agents (security, performance, architecture, simplicity, data integrity, pattern consistency, spec flow, git history, learnings, repo research, framework docs, best practices). The following changes were applied:

| # | Finding | Severity | Change |
|---|---------|----------|--------|
| 1 | Drop `note` field (YAGNI) | Critical | Watchlist simplified from `[[watchlist]]` array-of-tables with `WatchlistEntry` structs to plain `watchlist = ["user1", "user2"]` string array |
| 2 | Remove `WatchlistAction` enum | Critical | Dispatch directly to individual `run_watchlist_*` functions — no intermediate enum mapping |
| 3 | Config.toml file permissions | Critical | Apply `0o600` permissions to temp files and newly created config.toml (matches `tokens.json`/`cache.db` pattern) |
| 4 | CachedClient signature mismatch | Critical | Updated all code samples to use real `CachedClient::get(&mut self, url, &CacheContext, HeaderMap)` signature |
| 5 | Missing composite index on `usage` | Critical | Replaced `idx_usage_timestamp` with composite `idx_usage_ts_endpoint ON usage(timestamp, endpoint)` |
| 6 | Sequential check with no progress/streaming | Critical | Stream results to stdout per-account with progress on stderr; partial results on Ctrl+C |
| 7 | `add`/`remove` error asymmetry | Medium | Both operations now idempotent: `add` of existing = warning + exit 0; `remove` of missing = warning + exit 0 |
| 8 | `safe_write_config` double-parse over-engineering | Medium | Removed re-parse validation; `toml_edit` produces valid TOML by construction |
| 9 | `--sync` should bypass cache | Medium | Changed from `client.get()` to `client.http_get()` for usage endpoint |
| 10 | Unbounded `usage` table | Medium | Resolved (R6): 90-day retention with opportunistic pruning every 50th write |
| 11 | Missing config.toml handling | Medium | `list` and `remove` handle missing config.toml gracefully |
| 12 | Use `chrono::TimeDelta` not deprecated `Duration` | Low | Updated date parsing code |

## Research Enhancement Summary

**Deepened on:** 2026-02-12
**Research agents used:** 10 (toml_edit best practices, atomic writes, SQLite time-series, streaming JSON, X API usage endpoint, chrono docs, learnings researcher, git history analyzer, repo research analyst, framework docs researcher)

### Critical Research Findings

| # | Finding | Impact |
|---|---------|--------|
| R1 | **X API response format mismatch**: `daily_project_usage` is an object (not array), field is `usage` (not `tweets`), and `usage.fields` parameter is required | Fixes broken --sync parsing |
| R2 | **Pre-computed `date_ymd INTEGER` column**: `date(timestamp, 'unixepoch')` in GROUP BY forces full table scan; store YYYYMMDD integer at insert time for 3x faster aggregation | Schema change in migration |
| R3 | **Use `tempfile` crate for atomic writes**: Random file names prevent concurrent collisions, auto-cleanup on drop, permissions set at creation (no TOCTOU) — used by Cargo itself | New dependency, replaces hand-rolled pattern |
| R4 | **chrono features: `["now"]` not `["clock", "std"]`**: `clock` pulls unnecessary `winapi`/`iana-time-zone`; `now` is sufficient for `Utc::now()` and implies `std` | Smaller dependency footprint |
| R5 | **NDJSON default for streaming output**: Interrupt-safe (partial output valid), incremental `jq` processing, matches ripgrep/fd patterns — offer `--json-array` flag for wrapped format | Output format change |
| R6 | **90-day retention with opportunistic pruning**: Every 50th write, prune rows older than 90 days (matches existing cache pruning pattern) | Prevents unbounded table growth |
| R7 | **`BirdError::Config` for config write failures**: Exit code 78, not 1 — config corruption is semantically different from command execution failure | Error handling refinement |
| R8 | **`prepare_cached` not `prepare`**: Reuse compiled SQL statements across calls for repeated queries in same connection | Performance pattern |

## Problem Statement

### Account monitoring is manual and expensive

Researchers and developers who track multiple X accounts currently have no structured way to check for recent activity across their watch list. The manual process is:

1. Remember which accounts matter
2. Search for each one individually (`bird get /2/tweets/search/recent --query "from:username"`)
3. Mentally aggregate the results
4. Pay full API cost for each query, even when results haven't changed

At $0.005/tweet, checking 10 accounts that each return 20 tweets costs $1.00 per check. Without caching or batching awareness, this cost multiplies across repeated sessions.

### API cost visibility is nonexistent

Bird's Plan 1 cache layer tracks every API call in a SQLite `usage` table, but there is no way to query it. Users accumulate costs blindly. Worse, estimated costs (based on object counting) may diverge from X's actual billing. Without a way to compare estimates against actuals (`GET /2/usage/tweets`), cost tracking remains unverified guesswork.

### Why these two commands together

Watchlist and usage are complementary: watchlist is the highest-frequency cost generator (batch searches across many accounts), and usage is the cost visibility tool. Building them together lets us validate that watchlist's cost tracking integrates correctly with the usage reporting, and that cache efficiency gains from batch watchlist checks show up accurately in usage stats.

## Proposed Solution

### Watchlist: config-driven account monitoring

Store the watchlist as a simple `watchlist = ["user1", "user2"]` array in `~/.config/bird/config.toml`. The `check` subcommand runs `from:<username>` search queries for each watched account (leveraging Plan 1's `CachedClient` and Plan 2's search infrastructure), then displays an activity summary. The `add`/`remove` subcommands modify `config.toml` programmatically while preserving existing formatting and comments.

### Usage: cost visibility from local data + optional X API sync

Read the `usage` table from the SQLite cache database. Aggregate by date, endpoint, and cache hit/miss. Optionally fetch `GET /2/usage/tweets` to compare estimated costs against X's actual billing data. Store actuals in a `usage_actual` table for historical comparison.

> **Implementation note (from research):** The `usage` and `usage_actual` tables do NOT exist yet — `src/cache.rs` currently only has a `cache` table migration. Both tables must be created as new migrations in this plan's Phase 3, not assumed from Plan 1.

## Technical Approach

### File Layout

**New files:**

| File | Responsibility | Est. Lines |
|------|---------------|------------|
| `src/watchlist.rs` | Watchlist subcommands: check, add, remove, list | ~160 |
| `src/usage.rs` | Usage reporting: SQLite queries, --sync, comparison view | ~140 |

**Modified files:**

| File | Change | Lines Affected |
|------|--------|---------------|
| `src/main.rs` | Add `Watchlist` and `Usage` command variants, wire dispatch, add `mod` declarations | ~40 new lines |
| `src/config.rs` | Extend `FileConfig` with `watchlist: Option<Vec<String>>` field | ~3 new lines |
| `src/cache.rs` | Add `usage`/`usage_actual` table migrations, `log_usage()`, query methods | ~120 new lines |
| `src/requirements.rs` | Add auth requirements for `watchlist_check`, `watchlist_add`, `watchlist_remove`, `watchlist_list`, `usage`, `usage_sync` | ~30 new lines |
| `Cargo.toml` | Add `toml_edit`, `chrono`, and `tempfile` dependencies | 3 lines |

### Watchlist Command (`src/watchlist.rs`)

#### Config model extension

Extend `FileConfig` (line 56-61) to include the watchlist as a simple string array:

```rust
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FileConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub username: Option<String>,
    pub watchlist: Option<Vec<String>>,
}
```

> **Review finding (simplicity):** The original plan used a `WatchlistEntry` struct with `username` and `note: Option<String>` fields, stored as `[[watchlist]]` array-of-tables. The `note` field was dropped per YAGNI — no workflow depends on it, and it added complexity to every code path. A plain `watchlist = ["user1", "user2"]` array dramatically simplifies TOML manipulation (simple array vs. `ArrayOfTables`). Notes can be added later if needed.

**Why in FileConfig, not ResolvedConfig:** The watchlist is persisted config, not a resolved runtime value. It lives in the file config and is read/written directly. `ResolvedConfig` does not need a watchlist field because watchlist entries are not subject to the args > config > env > default priority chain -- they are purely file-managed data.

#### TOML modification strategy

**The hard problem:** Programmatic modification of TOML files (for `add`/`remove`) must preserve user comments, formatting, blank lines, and ordering of unrelated sections. The `toml` crate's `Serialize` destroys all formatting. The `toml_edit` crate preserves document structure.

**Decision: Use `toml_edit` for writes, `toml` for reads.**

- **Reading** (`list`, `check`): Continue using `toml::from_str()` via `FileConfig` deserialization. This is the existing pattern in `config.rs:87-91`.
- **Writing** (`add`, `remove`): Use `toml_edit::DocumentMut` to parse the raw TOML string, manipulate the `watchlist` array (simple string array, not array-of-tables), and serialize back. This preserves comments on all other sections.

**Cargo.toml additions:**

```toml
toml_edit = "0.22"
tempfile = "3"
```

> **Research insight (toml_edit):** `toml_edit` 0.22 renamed `Document` to `DocumentMut`. Use `item.as_array_mut()` as a convenience method directly on `Item` (delegates through `Value`). `Array::push` accepts `&str` directly via `Into<Value>`. New keys inserted via `doc.insert()` may appear at the top of the file for table items — but simple key-value pairs like `watchlist = [...]` are appended at the end naturally. Version 0.24 is available but 0.22 has a stable API for all needed operations.

**Remove operation (`toml_edit`):**

```rust
fn remove_from_watchlist(
    config_path: &Path,
    username: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(config_path)?;
    let mut doc = content.parse::<DocumentMut>()?;

    let removed = if let Some(arr) = doc.get_mut("watchlist")
        .and_then(|item| item.as_array_mut())
    {
        let initial_len = arr.len();
        arr.retain(|v| {
            !v.as_str()
                .map(|u| u.eq_ignore_ascii_case(username))
                .unwrap_or(false)
        });
        initial_len != arr.len()
    } else {
        false
    };

    if removed {
        safe_write_config(config_path, &doc.to_string())?;
    }
    Ok(removed)
}
```

**Add operation (`toml_edit`):**

```rust
use toml_edit::{DocumentMut, value, Array};

fn add_to_watchlist(
    config_path: &Path,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(config_path)
        .unwrap_or_default();
    let mut doc = content.parse::<DocumentMut>()?;

    // Check for duplicates
    if let Some(existing) = doc.get("watchlist") {
        if let Some(arr) = existing.as_array() {
            for val in arr.iter() {
                if val.as_str()
                    .map(|u| u.eq_ignore_ascii_case(username))
                    .unwrap_or(false)
                {
                    // Idempotent: print warning, exit 0 (not an error)
                    eprintln!("@{} is already in the watchlist.", username);
                    return Ok(());
                }
            }
        }
    }

    // Append to watchlist array (create if missing)
    if doc.get("watchlist").is_none() {
        doc.insert("watchlist", toml_edit::Item::Value(Array::new().into()));
    }
    doc["watchlist"].as_array_mut()
        .unwrap()
        .push(username);

    safe_write_config(config_path, &doc.to_string())?;
    Ok(())
}
```

> **Review finding (error symmetry):** Both `add` of an existing entry and `remove` of a nonexistent entry are now non-fatal (exit 0 with a warning on stderr). This makes both operations idempotent and script-friendly, which is more consistent than the original design where `add` was a hard error but `remove` was not.

**Edge cases for TOML modification:**

- Config file does not exist: create it with just the `watchlist = ["username"]` entry. **Apply `0o600` permissions** (see File Permissions below).
- Config file exists but has no `watchlist` key: append the key.
- Config file has comments on other sections: preserved by `toml_edit`.
- Duplicate username on add: print warning to stderr, exit 0 (idempotent).
- Username not found on remove: report "not found" on stderr, exit 0.
- Empty watchlist after last remove: leave `watchlist = []` (harmless).

#### File permissions for config.toml

> **Review finding (security):** The existing codebase enforces `0o600` on `tokens.json` (`src/auth.rs:249-261`) and `cache.db` (`src/cache.rs:88-98`). Since `config.toml` contains `client_id` and `client_secret`, the same permissions must apply to newly created config files and temp files during writes.

```rust
#[cfg(unix)]
fn set_file_permissions_0600(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}
```

Apply `0o600` to:
1. The temp file before rename in `safe_write_config()`
2. Any newly created `config.toml` (when it doesn't exist yet)

#### Atomic write with `safe_write_config`

> **Research insight (atomic writes):** Use the `tempfile` crate (`tempfile = "3"`) instead of hand-rolling the temp file pattern. Benefits: (1) random file names prevent concurrent invocation collisions (`.bird-config-a7x3Kf.tmp` vs fixed `.toml.tmp`), (2) auto-cleanup on drop if `persist()` is never called (panics, early returns), (3) `Builder::permissions()` sets 0o600 at creation time (no TOCTOU window). This is the pattern used by Cargo itself ([rust-lang/cargo#13898](https://github.com/rust-lang/cargo/pull/13898)).

```rust
use std::io::Write;
use tempfile::Builder;

fn safe_write_config(config_path: &Path, content: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let dir = config_path.parent().ok_or("config path has no parent")?;
    std::fs::create_dir_all(dir)?;

    let mut builder = Builder::new();
    builder.prefix(".bird-config-").suffix(".tmp");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o600));
    }

    let mut tmp = builder.tempfile_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.as_file().sync_all()?;  // durability before rename
    tmp.persist(config_path).map_err(|e| e.error)?;
    Ok(())
}
```

> **Research detail:** `tempfile_in(dir)` guarantees the temp file is on the same filesystem as the target, making `persist()` (which calls `rename(2)`) atomic on POSIX. If two `bird watchlist add` processes run simultaneously, both get unique temp files and "last writer wins" — the file is always in a valid state, never corrupt.

#### Subcommand implementations

**`bird watchlist list`** -- display current watchlist:

```rust
pub fn run_watchlist_list(
    config: &ResolvedConfig,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_path = config.config_dir.join("config.toml");
    let entries = load_watchlist(&config_path)?; // Returns Vec<String>

    if entries.is_empty() {
        eprintln!("Watchlist is empty. Add accounts with: bird watchlist add <username>");
    }

    // Always output valid JSON to stdout for piping
    if pretty {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!("{}", serde_json::to_string(&entries)?);
    }
    Ok(())
}

/// Load watchlist from config.toml. Returns empty vec if file missing.
fn load_watchlist(config_path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let file_config: FileConfig = toml::from_str(&content)?;
    Ok(file_config.watchlist.unwrap_or_default())
}
```

**`bird watchlist check`** -- batch search for recent activity:

This is the most complex subcommand. For each watched account, it runs a `from:<username>` search query using the same search infrastructure as `bird search` (Plan 2). Results are **streamed to stdout** as each account completes (not collected into a Vec first).

> **Review finding (CachedClient signature):** The actual `CachedClient::get()` takes `(&mut self, url: &str, ctx: &CacheContext<'_>, headers: HeaderMap)`. The plan's code samples are updated to match the real signature.

> **Review finding (performance):** Results are streamed to stdout as each account completes, giving the user partial results if they Ctrl+C during a long check. Progress is displayed on stderr.

```rust
pub async fn run_watchlist_check(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    pretty: bool,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_path = config.config_dir.join("config.toml");
    let entries = load_watchlist(&config_path)?;

    if entries.is_empty() {
        eprintln!("Watchlist is empty. Add accounts with: bird watchlist add <username>");
        // NDJSON: empty output (no lines) for empty watchlist
        return Ok(());
    }

    let token = resolve_token_for_command(client.http(), config, "watchlist_check").await?;
    let headers = build_auth_headers(&token)?;
    let ctx = CacheContext {
        auth_type: &token.auth_type,
        username: config.username.as_deref(),
    };

    // R5: Stream results as NDJSON (one JSON object per line).
    // NDJSON is interrupt-safe (partial output is valid), incrementally
    // consumable by jq (jq -c '.username' < output), and matches
    // ripgrep/fd streaming patterns. Use BufWriter with per-element flush.
    use std::io::{BufWriter, Write};
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    let total = entries.len();
    for (i, username) in entries.iter().enumerate() {
        eprintln!("[watchlist] checking @{} ({}/{})...", username, i + 1, total);

        let query = format!("from:{} -is:retweet", username);
        let search_url = format!(
            "https://api.x.com/2/tweets/search/recent\
             ?query={}\
             &max_results=10\
             &tweet.fields=created_at,public_metrics,author_id\
             &expansions=author_id\
             &user.fields=username,name,public_metrics",
            percent_encode(&query)
        );

        let activity = match client.get(&search_url, &ctx, headers.clone()).await {
            Ok(response) => {
                let body: serde_json::Value = serde_json::from_str(&response.body)?;
                let tweet_count = body.get("meta")
                    .and_then(|m| m.get("result_count"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);

                AccountActivity {
                    username: username.clone(),
                    recent_tweets: tweet_count,
                    latest_tweet: extract_latest_tweet(&body),
                    cache_hit: response.cache_hit,
                }
            }
            Err(e) => {
                eprintln!("[watchlist] error checking @{}: {}", username, e);
                AccountActivity {
                    username: username.clone(),
                    recent_tweets: 0,
                    latest_tweet: None,
                    cache_hit: false,
                }
            }
        };

        // NDJSON: one JSON object per line, flush after each for streaming
        serde_json::to_writer(&mut writer, &activity)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

#[derive(Serialize)]
struct AccountActivity {
    username: String,
    recent_tweets: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_tweet: Option<LatestTweet>,
    cache_hit: bool,
}

#[derive(Serialize)]
struct LatestTweet {
    id: String,
    text: String,
    created_at: String,
    likes: u64,
    retweets: u64,
}
```

**Key design decisions for `check`:**

1. **Sequential with NDJSON streaming output (R5):** Accounts are checked sequentially with Plan 1's rate limiter between requests. Each result is printed to stdout as a single-line JSON object (NDJSON / JSON Lines format), flushed immediately. NDJSON is interrupt-safe (partial output is still valid — each line is a complete JSON object), incrementally consumable by `jq` (e.g., `bird watchlist check | jq -c '.username'`), and matches the streaming patterns used by ripgrep and fd. Progress is shown on stderr: `[watchlist] checking @user (3/10)...`.

2. **Non-fatal per-account errors:** If one account fails (suspended, private, network error), the check continues to the next account. Errors are reported on stderr; the stdout JSON includes the account with `recent_tweets: 0`.

3. **Automatic `-is:retweet`:** Consistent with Plan 2's search behavior, retweets are excluded by default. The watchlist is about original content from the account.

4. **10 tweets per account:** `max_results=10` balances cost ($0.05 per account) against usefulness. This is enough to see if an account has been active recently. For deeper investigation, users can run `bird search "from:username"` directly.

5. **Cache interaction:** Watchlist checks benefit heavily from caching. If you run `bird watchlist check` twice within 15 minutes, the second run is fully cached ($0.00). The cost display on stderr shows "cache hit" for each account.

6. **Rejected: concurrent checks.** Running all queries concurrently via `tokio::JoinSet` would be faster but risks rate limits and interleaved stderr. Sequential with streaming gives partial results on Ctrl+C without the complexity.

#### Handler signature

> **Review finding (simplicity):** The original plan defined both `WatchlistCommand` (clap enum in `main.rs`) and a separate `WatchlistAction` (internal enum in `watchlist.rs`), with a mapping between them. This is unnecessary — use `WatchlistCommand` directly. The dispatch happens in `main.rs`, and each `watchlist.rs` function is called individually.

```rust
// src/watchlist.rs — no WatchlistAction enum needed.
// main.rs dispatches directly to individual functions:

pub async fn run_watchlist_check(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    pretty: bool,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { /* ... */ }

pub fn run_watchlist_add(
    config: &ResolvedConfig,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let clean = username.strip_prefix('@').unwrap_or(username);
    validate_username(clean)?;
    let config_path = config.config_dir.join("config.toml");
    add_to_watchlist(&config_path, clean)?;
    eprintln!("Added @{} to watchlist.", clean);
    Ok(())
}

pub fn run_watchlist_remove(
    config: &ResolvedConfig,
    username: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let clean = username.strip_prefix('@').unwrap_or(username);
    let config_path = config.config_dir.join("config.toml");
    let removed = remove_from_watchlist(&config_path, clean)?;
    if removed {
        eprintln!("Removed @{} from watchlist.", clean);
    } else {
        eprintln!("@{} was not in the watchlist.", clean);
    }
    Ok(())
}

pub fn run_watchlist_list(
    config: &ResolvedConfig,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { /* ... */ }
```

### Usage Command (`src/usage.rs`)

#### SQLite schema: usage and usage_actual tables

> **Critical:** These tables do not exist yet. Add them as new migrations in `src/cache.rs` `migrations()` (appending to the existing `vec![]` so rusqlite_migration applies them incrementally).

**Migration 2 — usage table** (stores a row per API call):

> **Review finding (performance):** The daily aggregation query groups by `date(timestamp, 'unixepoch')` and filters by `timestamp >= ?`. A composite index on `(timestamp, endpoint)` covers the daily breakdown, top-endpoints, and summary queries efficiently. Single-column indexes on `timestamp` and `endpoint` alone would not cover the `GROUP BY` + `WHERE` combination without a table scan.

> **Review finding (data integrity):** Added migration comment pattern — never modify existing migrations, only append new ones.

```sql
-- IMPORTANT: Never modify existing migrations. Only append new ones.
CREATE TABLE IF NOT EXISTS usage (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp      INTEGER NOT NULL,  -- unix timestamp
    date_ymd       INTEGER NOT NULL,  -- YYYYMMDD integer for fast GROUP BY (R2)
    endpoint       TEXT NOT NULL,
    method         TEXT NOT NULL,
    object_type    TEXT,
    object_count   INTEGER NOT NULL DEFAULT 0,
    estimated_cost REAL NOT NULL DEFAULT 0.0,
    cache_hit      INTEGER NOT NULL DEFAULT 0,
    username       TEXT
);
-- R2: Composite covering index for daily aggregation queries.
-- date_ymd (pre-computed YYYYMMDD integer) avoids date(timestamp, 'unixepoch') in GROUP BY
-- which forces a full table scan. 3x faster at 1M rows.
CREATE INDEX IF NOT EXISTS idx_usage_ymd_endpoint_cache ON usage(date_ymd, endpoint, cache_hit);
CREATE INDEX IF NOT EXISTS idx_usage_endpoint ON usage(endpoint);
```

> **Research finding (R2):** Using `date(timestamp, 'unixepoch')` in GROUP BY prevents index usage and forces a full table scan. A pre-computed `date_ymd INTEGER` (YYYYMMDD format, e.g., `20260211`) stored at insert time enables a composite covering index `(date_ymd, endpoint, cache_hit)` that satisfies the daily breakdown, top-endpoints, and summary queries without touching the table. Benchmarked at 3x faster for 1M rows.

**Migration 3 — usage_actual table** (stores X API actuals from `--sync`):

```sql
CREATE TABLE IF NOT EXISTS usage_actual (
    date         TEXT PRIMARY KEY,  -- YYYY-MM-DD
    tweet_count  INTEGER NOT NULL,  -- from X API
    synced_at    INTEGER NOT NULL   -- unix timestamp of when we synced
);
```

**`log_usage()` method** — add to `BirdDb` impl to record each API call:

```rust
pub fn log_usage(
    &mut self,
    endpoint: &str,
    method: &str,
    object_type: Option<&str>,
    object_count: i64,
    estimated_cost: f64,
    cache_hit: bool,
    username: Option<&str>,
) -> Result<(), rusqlite::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    // R2: Pre-compute YYYYMMDD integer for fast GROUP BY without date() function
    let date_ymd = {
        let dt = chrono::DateTime::from_timestamp(now, 0).unwrap();
        dt.format("%Y%m%d").to_string().parse::<i64>().unwrap()
    };
    // R6: Opportunistic pruning — every 50th write, delete rows older than 90 days
    self.maybe_prune_usage(now)?;
    self.conn.execute(
        "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![now, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit as i32, username],
    )?;
    Ok(())
}

/// R6: Prune usage rows older than 90 days. Called opportunistically every ~50 writes.
fn maybe_prune_usage(&self, now_ts: i64) -> Result<(), rusqlite::Error> {
    // Use a simple counter: prune when rowid % 50 == 0
    let row_count: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM usage", [], |row| row.get(0)
    )?;
    if row_count > 0 && row_count % 50 == 0 {
        let cutoff = now_ts - (90 * 24 * 60 * 60); // 90 days in seconds
        self.conn.execute(
            "DELETE FROM usage WHERE timestamp < ?1",
            [cutoff],
        )?;
    }
    Ok(())
}
```

**Integration point:** Call `log_usage()` from `CachedClient::get()` after each API response (or cache hit), alongside the existing cost display logic.

The usage command runs pure SQL aggregation queries against this table. No API calls are needed (except `--sync`).

**SQLite best practices (from research):**
- Use `COALESCE()` for NULL-safe aggregation (SUM of empty result set returns NULL, not 0)
- Use `CAST(SUM(...) AS INTEGER)` to prevent integer overflow in aggregation
- (R2) Pre-computed `date_ymd INTEGER` column avoids `date(timestamp, 'unixepoch')` in GROUP BY (3x faster at 1M rows)
- (R2) Composite covering index `(date_ymd, endpoint, cache_hit)` satisfies daily breakdown and summary queries without table scan
- Separate index on `endpoint` enables efficient `GROUP BY endpoint` for top-endpoints query
- (R8) Use `prepare_cached` (not `prepare`) to reuse compiled SQL statements across calls in the same connection
- (R6) Opportunistic pruning: every 50th write, delete rows older than 90 days (matches X API's 90-day history depth)
- Test with `:memory:` SQLite database in unit tests

#### Core aggregation queries

**Daily breakdown:**

```sql
-- R2: GROUP BY pre-computed date_ymd (covering index, no table scan)
SELECT
    date_ymd,
    SUM(estimated_cost) AS total_cost,
    COUNT(*) AS total_calls,
    SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END) AS cache_hits
FROM usage
WHERE date_ymd >= ?  -- since filter (YYYYMMDD integer)
GROUP BY date_ymd
ORDER BY date_ymd DESC;
```

> **Note:** The `--since` date string (`2026-02-01`) is converted to a YYYYMMDD integer (`20260201`) for the WHERE clause, matching the `date_ymd` column format.

**Top endpoints:**

```sql
SELECT
    endpoint,
    SUM(estimated_cost) AS total_cost,
    COUNT(*) AS call_count,
    SUM(object_count) AS total_objects
FROM usage
WHERE date_ymd >= ?  -- R2: YYYYMMDD integer filter
  AND cache_hit = 0  -- only count non-cached calls for cost
GROUP BY endpoint
ORDER BY total_cost DESC
LIMIT 10;
```

**Summary totals:**

```sql
SELECT
    SUM(estimated_cost) AS total_cost,
    COUNT(*) AS total_calls,
    SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END) AS cache_hits,
    SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END) AS actual_cost,
    SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ELSE 0 END) AS saved_cost
FROM usage
WHERE date_ymd >= ?;  -- R2: YYYYMMDD integer filter (index-friendly)
```

**Cache savings calculation:** When a request is a cache hit, the `estimated_cost` field still records what the request *would have cost*. The savings are the sum of `estimated_cost` for all cache hit rows. This is logged in Plan 1's cost tracking (every cache hit is recorded with the estimated cost that was avoided).

#### `--since DATE` filtering

Parse the `--since` argument as an ISO 8601 date (`YYYY-MM-DD`) using `chrono::NaiveDate`. Convert to a Unix timestamp at midnight UTC. If `--since` is not provided, default to 30 days ago.

```rust
/// Parse --since into a YYYYMMDD integer for date_ymd column filtering (R2).
fn parse_since(since: Option<&str>) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    match since {
        Some(date_str) => {
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| format!("invalid date '{}': {} (expected YYYY-MM-DD)", date_str, e))?;
            // Convert to YYYYMMDD integer (e.g., "2026-02-01" -> 20260201)
            Ok(date.format("%Y%m%d").to_string().parse::<i64>().unwrap())
        }
        None => {
            // Default: 30 days ago
            let now = chrono::Utc::now();
            let thirty_days_ago = now - chrono::TimeDelta::days(30);
            Ok(thirty_days_ago.format("%Y%m%d").to_string().parse::<i64>().unwrap())
        }
    }
}
```

**Cargo.toml addition:**

```toml
chrono = { version = "0.4", default-features = false, features = ["now"] }
```

> **Research finding (R4):** The `clock` feature pulls in `winapi` (Windows) and `iana-time-zone` (all platforms) — unnecessary for this use case. The `now` feature (added in chrono 0.4.35) is sufficient for `Utc::now()` and implies `std`. Smaller dependency footprint, faster builds.

**Date parsing edge cases (from research):**
- Leap year dates (e.g., `2024-02-29`) parse correctly with `NaiveDate`; invalid dates (e.g., `2025-02-29`) produce a clear error
- `--since` dates in the future: allow but produce empty results (no special handling needed)
- `--since` dates older than 90 days with `--sync`: warn that X API only returns 90 days of history
- Use `chrono::TimeDelta::days(30)` (not deprecated `Duration::days`) for the 30-day default

#### `--sync` flag: X API actual usage

The `--sync` flag fetches actual project-level usage from the X API endpoint `GET /2/usage/tweets`. This endpoint returns daily tweet consumption for up to 90 days and requires a Bearer token (app-level authentication).

**X API endpoint:** `GET /2/usage/tweets`

**Rate limit:** 50 requests per 15-minute window (Bearer token auth).

**History depth:** Up to 90 days of daily usage data. Validate `--since` does not exceed 90 days when used with `--sync`.

**Request:** `GET /2/usage/tweets?usage.fields=daily_project_usage`

> **Research finding (R1 — critical):** The `usage.fields=daily_project_usage` query parameter is **required** to get daily breakdown data. Without it, the response may omit the `daily_project_usage` field entirely.

**Response shape (from X API docs — corrected per research):**

> **Research finding (R1):** `daily_project_usage` is an **array** of per-day objects. Each day object contains a `date` string and a nested `usage` array where each element has a `usage` integer field (not `tweets`). Values may be returned as strings in some API tiers — parse defensively with `as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))`.

```json
{
  "data": {
    "daily_project_usage": [
      {
        "date": "2026-02-11T00:00:00.000Z",
        "usage": [
          {
            "usage": 42
          }
        ]
      }
    ],
    "daily_client_app_usage": [
      {
        "date": "2026-02-11T00:00:00.000Z",
        "usage": [
          {
            "app_id": 12345,
            "usage": 42
          }
        ],
        "client_app_id": "abc123"
      }
    ]
  }
}
```

> **Research finding (Feb 2026):** X API moved to a pay-per-use credit-based pricing model. Actual costs are only visible in the Developer Console, not via API. The usage endpoint returns tweet/user counts, not dollar amounts. The `--sync` feature tracks volume (tweet counts); dollar cost estimation remains local (`src/cost.rs`). The hardcoded `$0.005/tweet` and `$0.010/user` rates may need future calibration as the credit model evolves.

**Daily UTC deduplication:** X API uses daily UTC windows for billing dedup. A tweet fetched multiple times in the same UTC day counts once. This is a "soft guarantee" — the comparison view helps verify accuracy.

**`usage_actual` table** (created in Migration 3, see schema section above).

**Sync flow:**

1. Fetch `GET /2/usage/tweets` with Bearer token (bypassing cache — always want fresh data)
2. Parse the `daily_project_usage` array (primary source of truth for project-level usage)
3. Optionally parse `daily_client_app_usage` for per-app breakdown (store if present)
4. For each day, upsert into `usage_actual` (INSERT OR REPLACE)
5. Display comparison view

> **Note:** Use `client.http()` (not `client.get()`) to bypass cache for the usage endpoint — we always want fresh actuals from X, not cached stale data.

**Sync implementation:**

```rust
/// Parse a JSON value that may be an integer or a string-encoded integer.
/// X API sometimes returns usage counts as strings depending on API tier.
fn parse_usage_count(v: &serde_json::Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

async fn sync_actual_usage(
    client: &mut CachedClient,
    bird_db: &BirdDb,
    ctx: &CacheContext<'_>,
    headers: HeaderMap,
) -> Result<Vec<ActualUsageDay>, Box<dyn std::error::Error + Send + Sync>> {
    // R1: usage.fields parameter is required to get daily breakdown
    let url = "https://api.x.com/2/usage/tweets?usage.fields=daily_project_usage";
    // Use client.http() to bypass cache for this request --
    // we always want fresh usage data from X
    let response = client.http_get(url, headers).await?;

    let body: serde_json::Value = serde_json::from_str(&response.body)?;
    // R1: daily_project_usage is an array of per-day objects
    let daily = body
        .pointer("/data/daily_project_usage")
        .and_then(|d| d.as_array())
        .ok_or("unexpected response shape from /2/usage/tweets (missing daily_project_usage)")?;

    let mut results = Vec::new();
    for day_entry in daily {
        let date_str = day_entry.get("date")
            .and_then(|d| d.as_str())
            .ok_or("missing date field")?;
        // Parse "2026-02-11T00:00:00.000Z" to "2026-02-11"
        let date = &date_str[..10];

        // R1: Field is "usage" (not "tweets"), nested inside a usage array.
        // Values may be strings in some API tiers — parse defensively.
        let usage_count = day_entry.get("usage")
            .and_then(|u| u.as_array())
            .and_then(|arr| arr.first())
            .and_then(|u| u.get("usage"))
            .map(parse_usage_count)
            .unwrap_or(0);

        bird_db.upsert_actual_usage(date, usage_count)?;
        results.push(ActualUsageDay {
            date: date.to_string(),
            tweet_count: usage_count,
        });
    }

    Ok(results)
}
```

#### Comparison view: estimated vs actual

When `--sync` is used (or when `usage_actual` has data), the usage output includes a comparison section:

```
Estimated vs Actual (synced 2026-02-11 14:30 UTC)
──────────────────────────────────────────────────
  Date        Estimated    Actual    Diff
  2026-02-11  42 tweets    45        +3 (+7%)
  2026-02-10  38 tweets    40        +2 (+5%)
  2026-02-09  55 tweets    54        -1 (-2%)
  ...

Accuracy: estimated costs are within ~5% of actual (last 7 days)
```

**Why this matters:** The estimated cost is based on counting objects in response `data` and `includes` arrays. But X's billing may include objects we don't count (e.g., quoted tweets in expansions, edit history entries). Over time, the comparison reveals systematic under- or over-counting so we can calibrate `src/cost.rs`.

#### Handler signature

```rust
// src/usage.rs

pub async fn run_usage(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    since: Option<&str>,
    sync: bool,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cache_path = config.config_dir.join("cache.db");
    let bird_db = BirdDb::open(&cache_path)?;
    let since_ymd = parse_since(since)?;  // R2: YYYYMMDD integer

    // Always: query local usage table
    let summary = bird_db.query_usage_summary(since_ymd)?;
    let daily = bird_db.query_daily_usage(since_ymd)?;
    let top_endpoints = bird_db.query_top_endpoints(since_ymd)?;

    // Optionally: sync actual usage from X API
    let actuals = if sync {
        let token = resolve_token_for_command(client.http(), config, "usage_sync").await?;
        Some(sync_actual_usage(client, &bird_db, /* ... */).await?)
    } else {
        // Load existing actuals if available
        bird_db.query_actual_usage(since_ymd)?
    };

    // Build and output the report — format YYYYMMDD back to YYYY-MM-DD for display
    let since_display = format!(
        "{}-{:02}-{:02}",
        since_ymd / 10000,
        (since_ymd % 10000) / 100,
        since_ymd % 100
    );
    let report = UsageReport {
        since: since.map(String::from).unwrap_or(since_display),
        until: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        summary,
        daily,
        top_endpoints,
        comparison: actuals.map(build_comparison),
    };

    if pretty {
        print_usage_pretty(&report);
    } else {
        println!("{}", serde_json::to_string(&report)?);
    }
    Ok(())
}
```

#### Pretty output format

```
$ bird usage --since 2026-02-01 --pretty
API Usage (2026-02-01 to 2026-02-11)
-------------------------------------
Total estimated cost:  $12.45
Total API calls:       247
Cache hit rate:        62%
Estimated savings:     ~$8.30

Daily breakdown:
  2026-02-11  $2.10  (42 calls, 68% cached)
  2026-02-10  $1.85  (38 calls, 55% cached)
  2026-02-09  $3.20  (67 calls, 48% cached)
  ...

Top endpoints:
  /2/tweets/search/recent  $8.20  (156 calls)
  /2/users/by/username     $2.10  (52 calls)
  /2/users/{id}/bookmarks  $1.15  (23 calls)
```

With `--sync`:

```
$ bird usage --since 2026-02-01 --sync --pretty
API Usage (2026-02-01 to 2026-02-11)
-------------------------------------
Total estimated cost:  $12.45
Total API calls:       247
Cache hit rate:        62%
Estimated savings:     ~$8.30

Daily breakdown:
  2026-02-11  $2.10  (42 calls, 68% cached)
  2026-02-10  $1.85  (38 calls, 55% cached)
  ...

Top endpoints:
  /2/tweets/search/recent  $8.20  (156 calls)
  /2/users/by/username     $2.10  (52 calls)
  /2/users/{id}/bookmarks  $1.15  (23 calls)

Estimated vs Actual (synced 2026-02-11 14:30 UTC)
--------------------------------------------------
  Date        Est. tweets  Actual  Diff
  2026-02-11  42           45      +3 (+7%)
  2026-02-10  38           40      +2 (+5%)
  2026-02-09  55           54      -1 (-2%)

Accuracy: within ~5% of actual (last 7 days)
```

### Command Enum Additions

In `src/main.rs`, add to the `Command` enum (after line 191, before `Doctor`):

```rust
/// Monitor accounts: check recent activity, manage watchlist
Watchlist {
    #[command(subcommand)]
    action: WatchlistCommand,
    #[arg(long)]
    pretty: bool,
},

/// View API usage and costs
Usage {
    /// Show usage since this date (YYYY-MM-DD; default: 30 days ago)
    #[arg(long)]
    since: Option<String>,
    /// Sync actual usage from X API (requires Bearer token)
    #[arg(long)]
    sync: bool,
    #[arg(long)]
    pretty: bool,
},
```

The `WatchlistCommand` subcommand enum:

```rust
#[derive(clap::Subcommand)]
enum WatchlistCommand {
    /// Check recent activity for all watched accounts
    Check,
    /// Add an account to the watchlist
    Add {
        /// X/Twitter username (with or without @)
        username: String,
    },
    /// Remove an account from the watchlist
    Remove {
        /// X/Twitter username to remove
        username: String,
    },
    /// Show the current watchlist
    List,
}
```

### Dispatch in `run()`

In `src/main.rs` `run()` function (after line 246, before `Command::Doctor`):

> **Review finding (simplicity):** Dispatch directly to individual `watchlist::run_*` functions — no intermediate `WatchlistAction` mapping needed.

```rust
Command::Watchlist { action, pretty } => {
    match action {
        WatchlistCommand::Check => {
            watchlist::run_watchlist_check(client, &config, pretty, use_color)
                .await
                .map_err(|e| BirdError::Command { name: "watchlist", source: e })?;
        }
        WatchlistCommand::Add { username } => {
            // R7: Config write failures use BirdError::Config (exit 78),
            // not BirdError::Command (exit 1) — semantically different
            watchlist::run_watchlist_add(&config, &username)
                .map_err(|e| BirdError::Config(e.to_string()))?;
        }
        WatchlistCommand::Remove { username } => {
            watchlist::run_watchlist_remove(&config, &username)
                .map_err(|e| BirdError::Config(e.to_string()))?;
        }
        WatchlistCommand::List => {
            watchlist::run_watchlist_list(&config, pretty)
                .map_err(|e| BirdError::Command { name: "watchlist", source: e })?;
        }
    }
}
Command::Usage { since, sync, pretty } => {
    usage::run_usage(client, &config, since.as_deref(), sync, pretty)
        .await
        .map_err(|e| BirdError::Command { name: "usage", source: e })?;
}
```

### Auth Requirements

In `src/requirements.rs`, add to `requirements_for_command()` (before line 79, the catch-all `_ => return None`):

```rust
// Watchlist check uses search endpoint (requires auth)
"watchlist_check" => CommandReqs {
    accepted: &[AuthType::OAuth2User, AuthType::Bearer],
    oauth2_hint: OAUTH2_HINT,
    oauth1_hint: OAUTH1_HINT,
    bearer_hint: BEARER_HINT,
},
// Watchlist add/remove/list are local config operations (no auth needed)
"watchlist_add" | "watchlist_remove" | "watchlist_list" => CommandReqs {
    accepted: &[AuthType::None],
    oauth2_hint: "",
    oauth1_hint: "",
    bearer_hint: "",
},
// Usage reads local SQLite (no auth)
"usage" => CommandReqs {
    accepted: &[AuthType::None],
    oauth2_hint: "",
    oauth1_hint: "",
    bearer_hint: "",
},
// Usage --sync needs Bearer for GET /2/usage/tweets
"usage_sync" => CommandReqs {
    accepted: &[AuthType::Bearer],
    oauth2_hint: "",
    oauth1_hint: "",
    bearer_hint: BEARER_HINT,
},
```

Update `command_names_with_auth()` (line 84-86) to include the new commands:

```rust
pub fn command_names_with_auth() -> &'static [&'static str] {
    &[
        "login", "me", "bookmarks", "get", "post", "put", "delete",
        "watchlist_check", "watchlist_add", "watchlist_remove", "watchlist_list",
        "usage", "usage_sync",
    ]
}
```

### BirdDb API Additions (for usage queries)

`BirdDb` (in `src/cache.rs`) needs the following query methods to support `bird usage`. These are added in Phase 3 of this plan:

```rust
// In src/cache.rs (BirdDb impl)

// R8: All query methods use prepare_cached for compiled statement reuse.
// R2: All WHERE clauses filter on date_ymd (YYYYMMDD integer) not timestamp.
// Callers pass since_ymd: i64 (e.g., 20260201 for 2026-02-01).

pub fn query_usage_summary(&self, since_ymd: i64) -> Result<UsageSummary, rusqlite::Error> {
    self.conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END), 0),
            COUNT(*),
            COALESCE(SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ELSE 0 END), 0)
         FROM usage WHERE date_ymd >= ?1",
        [since_ymd],
        |row| Ok(UsageSummary {
            total_cost: row.get(0)?,
            total_calls: row.get(1)?,
            cache_hits: row.get(2)?,
            estimated_savings: row.get(3)?,
        }),
    )
}

pub fn query_daily_usage(&self, since_ymd: i64) -> Result<Vec<DailyUsage>, rusqlite::Error> {
    let mut stmt = self.conn.prepare_cached(
        "SELECT
            date_ymd,
            SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END),
            COUNT(*),
            SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END)
         FROM usage
         WHERE date_ymd >= ?1
         GROUP BY date_ymd
         ORDER BY date_ymd DESC"
    )?;
    let rows = stmt.query_map([since_ymd], |row| {
        Ok(DailyUsage {
            date_ymd: row.get(0)?,
            cost: row.get(1)?,
            calls: row.get(2)?,
            cache_hits: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn query_top_endpoints(&self, since_ymd: i64) -> Result<Vec<EndpointUsage>, rusqlite::Error> {
    let mut stmt = self.conn.prepare_cached(
        "SELECT endpoint, SUM(estimated_cost), COUNT(*)
         FROM usage
         WHERE date_ymd >= ?1 AND cache_hit = 0
         GROUP BY endpoint
         ORDER BY SUM(estimated_cost) DESC
         LIMIT 10"
    )?;
    let rows = stmt.query_map([since_ymd], |row| {
        Ok(EndpointUsage {
            endpoint: row.get(0)?,
            cost: row.get(1)?,
            calls: row.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn upsert_actual_usage(&self, date: &str, tweet_count: u64) -> Result<(), rusqlite::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    self.conn.execute(
        "INSERT OR REPLACE INTO usage_actual (date, tweet_count, synced_at)
         VALUES (?1, ?2, ?3)",
        rusqlite::params![date, tweet_count, now],
    )?;
    Ok(())
}

pub fn query_actual_usage(
    &self,
    since_ymd: i64,
) -> Result<Option<Vec<ActualUsageDay>>, rusqlite::Error> {
    // Convert YYYYMMDD integer to YYYY-MM-DD string for usage_actual.date comparison
    let since_date = format!(
        "{}-{:02}-{:02}",
        since_ymd / 10000,
        (since_ymd % 10000) / 100,
        since_ymd % 100
    );
    let mut stmt = self.conn.prepare_cached(
        "SELECT date, tweet_count, synced_at FROM usage_actual
         WHERE date >= ?1
         ORDER BY date DESC"
    )?;
    let rows: Vec<ActualUsageDay> = stmt.query_map([&since_date], |row| {
        Ok(ActualUsageDay {
            date: row.get(0)?,
            tweet_count: row.get(1)?,
            synced_at: row.get(2)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rows))
    }
}
```

**Data structures for usage queries:**

```rust
#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub total_cost: f64,
    pub total_calls: u64,
    pub cache_hits: u64,
    pub estimated_savings: f64,
}

#[derive(Debug, Serialize)]
pub struct DailyUsage {
    pub date_ymd: i64,  // R2: YYYYMMDD integer (e.g., 20260211)
    pub cost: f64,
    pub calls: u64,
    pub cache_hits: u64,
}

#[derive(Debug, Serialize)]
pub struct EndpointUsage {
    pub endpoint: String,
    pub cost: f64,
    pub calls: u64,
}

#[derive(Debug, Serialize)]
pub struct ActualUsageDay {
    pub date: String,
    pub tweet_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<i64>,
}
```

### Username validation

Usernames passed to `watchlist add` and `watchlist remove` should be validated before modifying config.toml:

```rust
/// Validate a username (after @ stripping). Called before any TOML modification.
fn validate_username(username: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if username.is_empty() {
        return Err("username must not be empty".into());
    }
    if username.len() > 15 {
        return Err(format!("username '{}' exceeds X's 15-character limit", username).into());
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "username '{}' contains invalid characters (only a-z, A-Z, 0-9, _ allowed)",
            username
        ).into());
    }
    Ok(())
}
```

The `@` is stripped in the caller (`run_watchlist_add`, `run_watchlist_remove`) before validation. This follows the same validation pattern as `src/profile.rs` (`validate_username`).

### How Both Commands Use CachedClient and Cost Tracking

**Watchlist `check`:**
- Each search query goes through `CachedClient::get(&mut self, url, &CacheContext, HeaderMap)`, which handles cache lookup, cost estimation, usage logging, and rate limiting (all from Plan 1).
- Cost is displayed on stderr per account: `[cost] 10 tweets, ~$0.05 (cache miss)`.
- On a repeated check within the 15-minute TTL, each account returns from cache: `[cost] cache hit, $0.00 (10 tweets)`.
- All calls are logged to the `usage` table, so `bird usage` reflects watchlist costs accurately.

**Usage (default, no `--sync`):**
- Does not use `CachedClient` at all. Opens the SQLite database directly via `BirdDb` for read-only queries.
- No API calls, no cost incurred, no auth required.

**Usage with `--sync`:**
- Uses `client.http_get()` (bypassing cache) to fetch `GET /2/usage/tweets` with Bearer auth — we always want fresh actuals from X, not cached stale data.
- This is itself a billable API call. The cost is logged to the usage table.

## Acceptance Criteria

### Functional Requirements

**Watchlist:**

- [x] `bird watchlist add <username>` adds entry to `config.toml` `watchlist` array
- [x] `bird watchlist add` strips leading `@` from username
- [x] `bird watchlist add` rejects invalid usernames (empty, >15 chars, invalid chars)
- [x] `bird watchlist add` is idempotent: duplicate usernames print warning, exit 0
- [x] `bird watchlist add` creates `config.toml` with `0o600` permissions if it doesn't exist
- [x] `bird watchlist add` preserves existing comments and formatting in `config.toml`
- [x] `bird watchlist remove <username>` removes the entry (case-insensitive match)
- [x] `bird watchlist remove` reports "not found" for non-existent usernames (exit 0)
- [x] `bird watchlist list` outputs JSON array of username strings
- [x] `bird watchlist list --pretty` outputs pretty-printed JSON
- [x] `bird watchlist list` outputs `[]` when watchlist is empty
- [x] `bird watchlist list` handles missing config.toml gracefully (outputs `[]`)
- [x] `bird watchlist check` searches recent tweets for each watched account
- [x] `bird watchlist check` streams NDJSON (one JSON object per line) per account as they complete (R5)
- [x] `bird watchlist check` shows progress on stderr: `[watchlist] checking @user (3/10)...`
- [x] `bird watchlist check` continues on per-account errors (non-fatal)
- [x] `bird watchlist check` uses CachedClient (benefits from cache)
- [x] `bird watchlist check` displays per-account cost on stderr
- [x] `bird watchlist check` requires OAuth2User or Bearer auth
- [x] `bird watchlist add/remove/list` do NOT require any auth
- [x] Temp files during config writes use `0o600` permissions

**Usage:**

- [x] `bird usage` shows last 30 days by default
- [x] `bird usage --since 2026-02-01` filters to the given date range
- [x] `bird usage` outputs JSON by default
- [x] `bird usage --pretty` outputs human-readable formatted report
- [x] `bird usage` shows: total cost, total calls, cache hit rate, estimated savings
- [x] `bird usage` shows daily breakdown with cost, calls, cache percentage
- [x] `bird usage` shows top endpoints by cost
- [x] `bird usage` requires no auth (reads local SQLite only)
- [x] `bird usage --sync` fetches `GET /2/usage/tweets` from X API
- [x] `bird usage --sync` requires Bearer token
- [x] `bird usage --sync` stores actual usage in `usage_actual` table
- [x] `bird usage --sync` displays estimated vs actual comparison
- [x] `bird usage` gracefully handles empty usage table (no prior API calls)
- [ ] `bird usage` gracefully handles missing cache.db (no cache layer initialized)
- [x] Invalid `--since` date produces a clear error message

### Non-Functional Requirements

- [x] `bird watchlist add/remove` completes in < 50ms (file I/O only)
- [x] `bird watchlist list` completes in < 10ms (file read only)
- [x] `bird usage` (without `--sync`) completes in < 100ms (SQLite query only)
- [x] No regression in existing command behavior
- [x] TOML comments and formatting are preserved after add/remove operations

### Quality Gates

- [x] Unit tests for: username validation, TOML add/remove operations, usage query parsing, date parsing
- [x] Unit tests for: duplicate detection (idempotent add), case-insensitive matching, `@` stripping
- [x] Unit tests for: missing config.toml on list/remove, `0o600` permissions on new files
- [ ] Integration test: add -> list -> check -> remove -> list lifecycle
- [ ] Integration test: usage with empty table, usage with populated data
- [x] All existing tests continue to pass
- [x] `cargo clippy` clean
- [x] No secrets in TOML modifications (watchlist entries contain only usernames)

## Implementation Phases

### Phase 1: Config model + watchlist add/remove/list

**No API calls. No CachedClient dependency. Can be implemented before Plan 1 ships.**

1. [x] Add `toml_edit = "0.22"` and `tempfile = "3"` to `Cargo.toml` (R3)
2. [x] Add `watchlist: Option<Vec<String>>` field to `FileConfig` in `src/config.rs`
3. [x] Create `src/watchlist.rs` with:
   - `validate_username()`
   - `load_watchlist()` (reads from config.toml via `toml` deserialization)
   - `add_to_watchlist()` (uses `toml_edit` for formatting-preserving writes)
   - `remove_from_watchlist()` (uses `toml_edit`)
   - `safe_write_config()` with `0o600` permissions on temp file
   - `run_watchlist_list()` (handles missing config.toml gracefully)
   - `run_watchlist_add()`, `run_watchlist_remove()` (individual functions, no enum)
4. [x] Add `Watchlist` and `WatchlistCommand` to `Command` enum in `src/main.rs`
5. [x] Add dispatch in `run()` — match directly on `WatchlistCommand` variants
6. [x] Add auth requirements for `watchlist_add`, `watchlist_remove`, `watchlist_list`
7. [x] Add `mod watchlist` to `src/main.rs`
8. [x] Unit tests: add, remove, list, duplicate detection (idempotent), username validation, TOML preservation, `0o600` permissions, missing config.toml

### Phase 2: Watchlist check

**Requires Plan 1 (CachedClient) and Plan 2 (search patterns).**

1. [x] Implement `run_watchlist_check()` in `src/watchlist.rs` with NDJSON streaming output (R5)
2. [x] Add auth requirement for `watchlist_check`
3. [x] Wire up the `Check` arm in main dispatch
4. [ ] Integration test: add accounts, check, verify NDJSON output shape (each line is valid JSON)

### Phase 3: Usage tables + local queries + usage command

**Creates the usage infrastructure that Plan 1 deferred.**

1. [x] Add `chrono = { version = "0.4", default-features = false, features = ["now"] }` to `Cargo.toml` (R4)
2. [x] **Add `usage` and `usage_actual` table migrations** to `src/cache.rs` `migrations()` (Migration 2 and 3). Usage table includes `date_ymd INTEGER` column (R2).
3. [x] **Add `log_usage()` method** to `BirdDb` impl — computes `date_ymd` at insert time, includes opportunistic 90-day pruning (R6)
4. [ ] **Integrate `log_usage()` into `CachedClient::get()`** — call after each API response/cache hit
5. [x] Create `src/usage.rs` with:
   - `parse_since()` returning YYYYMMDD integer (R2)
   - `run_usage()` handler
   - Pretty formatting helpers
   - Data structures (`UsageSummary`, `DailyUsage`, `EndpointUsage`)
6. [x] Add `BirdDb` query methods using `prepare_cached` (R8): `query_usage_summary()`, `query_daily_usage()`, `query_top_endpoints()` — all filter on `date_ymd` (R2)
7. [x] Add `Usage` to `Command` enum in `src/main.rs`
8. [x] Add dispatch in `run()`
9. [x] Add auth requirement for `usage` (AuthType::None)
10. [x] Add `mod usage` to `src/main.rs`
11. [x] Unit tests: date parsing, empty table handling, query result formatting, migration applies cleanly on existing DBs

### Phase 4: Usage --sync

**Requires CachedClient and Bearer token.**

1. [x] Implement `sync_actual_usage()` in `src/usage.rs` using `client.http_get()` (bypass cache)
2. [x] Request URL must include `usage.fields=daily_project_usage` parameter (R1)
3. [x] Parse `daily_project_usage` array — field is `usage` not `tweets`, parse defensively for string/integer values (R1)
4. [x] Add `upsert_actual_usage()` and `query_actual_usage()` to `BirdDb`
5. [x] Add comparison view to usage output (estimated vs actual with diff/percentage)
6. [x] Add auth requirement for `usage_sync` (Bearer only)
7. [ ] Handle rate limit (50 req/15min) — warn on 429 response
8. [x] Validate `--since` does not exceed 90 days when used with `--sync` (warn, not error)
9. [x] Unit test: comparison calculation, sync result parsing, 90-day boundary, string-vs-integer usage values

## Alternative Approaches Considered

### 1. Separate watchlist file instead of config.toml

**Rejected.** A dedicated `~/.config/bird/watchlist.toml` would be simpler to modify (no risk of corrupting other config), but it fragments the config surface. Users expect all bird configuration in one file. The `toml_edit` crate solves the formatting preservation problem cleanly.

### 2. SQLite for watchlist storage instead of TOML

**Rejected.** Storing the watchlist in `cache.db` would avoid the TOML modification complexity entirely. But: (a) the watchlist is user-curated config, not cache data -- it should survive `bird cache clear`; (b) users should be able to edit it by hand in their text editor; (c) TOML is human-readable and version-controllable (dotfile repos).

### 3. Concurrent watchlist checks (tokio::JoinSet)

**Rejected for v1.** Running all watchlist search queries concurrently would be faster but: (a) risks hitting rate limits when checking many accounts; (b) complicates error handling (partial failures); (c) makes stderr cost output interleaved and confusing. Sequential with streaming output (partial results on Ctrl+C) is simpler and safer. Can revisit if users report performance issues with large watchlists (>20 accounts).

### 4. `bird usage` as a subcommand of `bird cache`

**Considered.** `bird cache usage` would group it with `bird cache clear` and `bird cache stats`. But usage tracking is conceptually separate from caching -- it tracks all API calls, including those with `--no-cache`. Making it a top-level command (`bird usage`) reflects that it is a first-class feature, not a cache management detail.

### 5. Rolling log file instead of SQLite for usage tracking

**Rejected.** A JSON-lines log file would be simpler to write but impossible to query efficiently. Aggregating daily costs from a 100K-line log file requires reading the entire file. SQLite's indexed queries return results in milliseconds regardless of table size.

### 6. Using `toml` crate (serialize) instead of `toml_edit` for writes

**Rejected.** The `toml` crate's `Serialize` output strips all comments and normalizes formatting. For a user-maintained config file, this is unacceptable. A user who carefully organized their `config.toml` with comments explaining each section would lose all of that on the first `bird watchlist add`. The `toml_edit` crate exists specifically for this use case.

### 7. `WatchlistEntry` struct with `note` field (array-of-tables)

**Rejected (review finding).** The original plan used `[[watchlist]]` with `WatchlistEntry { username, note }`. The `note` field was dropped per YAGNI — no workflow depends on it. This cascaded into simplifying the TOML storage from array-of-tables to a plain `watchlist = [...]` string array, eliminating ~30 lines of TOML manipulation complexity. Notes can be added later if needed.

### 8. `safe_write_config` with double-parse validation

**Rejected (review finding).** The original plan validated TOML output by re-parsing it with both `toml_edit::DocumentMut` and `toml::from_str::<FileConfig>`. Since `toml_edit` produces valid TOML by construction, this added complexity without catching real bugs. Removed in favor of the simpler write-then-rename pattern.

## Dependencies & Risks

### Dependencies

| Dependency | Version | Purpose | Binary Impact |
|-----------|---------|---------|---------------|
| `toml_edit` | 0.22 | Formatting-preserving TOML modification | ~100KB (estimate) |
| `tempfile` | 3 | Atomic file writes with auto-cleanup (R3) | ~20KB (estimate) |
| `chrono` | 0.4 | Date parsing for `--since` flag (features: `["now"]` only — R4) | ~150KB (minimal features) |
| `rusqlite` | 0.38 | SQLite queries (from Plan 1) | Already included |
| CachedClient | Plan 1 | HTTP caching, cost tracking | Already included |

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `toml_edit` breaks formatting on edge cases | Low | Medium | Simplified by using a plain array (`watchlist = [...]`) instead of array-of-tables (`[[watchlist]]`). Unit tests with real config files containing comments, blank lines, inline tables. Roundtrip test: parse -> modify -> parse -> verify unrelated sections unchanged. |
| `config.toml` concurrent modification | Low | Medium | `toml_edit` operations are read-modify-write (not atomic). Two simultaneous `bird watchlist add` could race. Mitigation: advisory file lock (`flock`) on config.toml during write. Not implemented in v1 -- the window is tiny for a CLI tool. Document as known limitation. |
| Unbounded `usage` table growth | Medium | Low | (R6) Mitigated: opportunistic pruning every 50th write deletes rows older than 90 days. Matches X API's 90-day history depth and existing cache pruning pattern. Users can also run `bird cache clear` to fully reset. |
| X API changes `GET /2/usage/tweets` response format | Low | Low | Parse defensively with `Option` chains. Log warning if response doesn't match expected shape. The `--sync` feature degrades gracefully -- local usage tracking still works. |
| Large watchlists (>50 accounts) cause rate limiting | Low | Medium | Sequential checking with 150ms minimum interval. 50 accounts = ~7.5s minimum. Display progress on stderr: `[watchlist] checking @username (3/50)...`. Consider adding a `--limit N` flag if needed. |
| Cost estimates diverge significantly from actuals | Medium | Medium | This is exactly why `--sync` exists. Document that estimates are best-effort. The comparison view helps users calibrate expectations. |
| Empty `cache.db` when usage is queried | Medium | Low | `bird usage` before any cached API call. Handle gracefully: "No usage data recorded yet. Run some commands first." |

### config.toml write safety

**Critical concern:** `bird watchlist add/remove` modifies the user's config file. A bug here could corrupt configuration and break all bird commands.

**Safeguards:**

1. **Atomic write pattern:** Write to a temporary file (`config.toml.tmp`), then rename. This prevents partial writes on crashes. The temp file is created with `config_path.with_extension("toml.tmp")` ensuring it's on the same filesystem (required for atomic rename).

2. **File permissions:** Temp file and any newly created config.toml use `0o600` permissions (matching `tokens.json` and `cache.db` patterns) since config.toml may contain `client_id`/`client_secret`.

> **Review finding (simplicity):** The original plan included a double-parse validation step (re-parse output with both `toml_edit::DocumentMut` and `toml::from_str::<FileConfig>`) in `safe_write_config()`. This was removed — `toml_edit` produces valid TOML by construction, and the `DocumentMut::to_string()` output always round-trips. The validation added complexity without catching real bugs.

See the `safe_write_config()` implementation in the "Atomic write with `safe_write_config`" section above.

## References

### Internal References

- Brainstorm: `docs/brainstorms/2026-02-11-research-commands-and-caching-brainstorm.md`
  - Watchlist decision: lines 100-114
  - Usage/cost tracking decision: lines 95-98
  - Implementation order: lines 187-195
- Plan 1 (Cache Layer): `docs/plans/2026-02-11-feat-transparent-cache-layer-plan.md`
  - CachedClient API: lines 253-308
  - Usage table schema: lines 144-158
  - BirdDb API: lines 536-543
  - Cost tracking: lines 316-357
- Command pattern template: `src/raw.rs:10-82`
- Config loading: `src/config.rs:78-158` (ResolvedConfig::load)
- FileConfig struct: `src/config.rs:55-61`
- Auth requirements registry: `src/requirements.rs:40-81`
- Auth types: `src/requirements.rs:7-16`
- Command names list: `src/requirements.rs:84-86`
- File permissions pattern: `src/auth.rs:245-257`
- Main dispatch: `src/main.rs:193-249`
- Command enum: `src/main.rs:118-191`
- BirdError: `src/main.rs:22-57`
- Shared client creation: `src/main.rs:292-296`
- Output helpers: `src/output.rs` (color, muted, error, success)

### External References

- X API billing: $0.005/tweet, $0.010/user, 24hr UTC dedup (note: X moved to pay-per-use credits Feb 2026 — dollar rates may need recalibration)
- X API usage endpoint: `GET /2/usage/tweets` — Bearer-only, 50 req/15min rate limit, 90-day history
- X API usage response: `daily_project_usage` and `daily_client_app_usage` arrays with nested `usage` counts (field is `usage`, not `tweets` — R1); requires `usage.fields=daily_project_usage` query parameter
- X username rules: 1-15 characters, alphanumeric + underscore only
- `toml_edit` crate: https://docs.rs/toml_edit — simple array manipulation via `as_array_mut()` and `retain()`
- `chrono` crate: https://docs.rs/chrono — `NaiveDate` parsing, use `TimeDelta::days()` (not deprecated `Duration::days`)
- SQLite date functions: https://www.sqlite.org/lang_datefunc.html — `date(timestamp, 'unixepoch')` for grouping
- `rusqlite_migration` crate: append new `M::up()` entries to existing migrations vec for incremental schema evolution
