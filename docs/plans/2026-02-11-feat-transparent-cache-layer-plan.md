---
title: "feat: Transparent Cache Layer with Cost Tracking"
type: feat
status: completed
date: 2026-02-11
series: "Research Commands & Caching Layer"
plan: 1 of 4
depends_on: null
deepened: 2026-02-11
---

# feat: Transparent Cache Layer with Cost Tracking

## Enhancement Summary

**Deepened on:** 2026-02-11  
**Agents used:** architecture-strategist, code-simplicity-reviewer, performance-oracle, security-sentinel, pattern-recognition-specialist, best-practices-researcher, framework-docs-researcher, security-audit-learnings

### Scope Changes (Simplicity Review)

The original plan front-loaded complexity for Plans 2-4 that don't exist yet. **4 phases cut, ~250 lines saved, 3 fewer files:**

| Phase | Original | After Review | Rationale |
|-------|----------|-------------|-----------|
| Phase 4: Request Packing | Included | **Cut** | Zero current consumers, highest-risk code, add when Plans 2-4 need it |
| Phase 5: Rate Limiting | Full module | **2-line sleep** | A `tokio::time::sleep(150ms)` in bookmarks pagination is sufficient for now |
| Phase 6: Field Profiles | Included | **Deferred to Plan 2** | No current command uses field profiles; YAGNI |
| Usage table | In schema | **Deferred to Plan 4** | Keep cost.rs as stateless stderr display; add persistent tracking when `bird usage` is built |

**Result:** 2 new files (~250 lines) instead of 5 (~530 lines). A **13% codebase increase** instead of 27%.

### Critical Fixes

1. **BUG: `AuthType::None` registration will fail at runtime** — `resolve_token_for_command()` has no handler for `None` in accepted array. Use `login` pattern (return `None` from `requirements_for_command`) instead.
2. **BUG: `auth_type: &str` breaks type safety** — use `&AuthType` enum from `requirements.rs`.
3. **SECURITY (HIGH): WAL/SHM sidecar files don't inherit 0o600** — set umask before SQLite opens.
4. **SECURITY (HIGH): Usage/stats endpoint column stores query params** — strip to path only.
5. **AUTH: Don't change `auth.rs` signatures** — pass `cached_client.http()` from handlers instead.
6. **SCHEMA: Use `PRAGMA user_version`** instead of custom `meta` table.

### Key Research Findings

- **No `spawn_blocking` needed** — single-threaded tokio runtime, SQLite ops are sub-millisecond
- **SHA-256 for cache keys** — already in deps via `sha2` crate, zero binary cost
- **`rusqlite_migration` crate** — lightweight, uses `PRAGMA user_version`, validates at test time
- **`prepare_cached`** — free LRU statement cache, ~100μs savings per repeated query
- **Passive WAL checkpoint on Drop** — keeps WAL file small between invocations

---

## Overview

Add a transparent SQLite-backed cache layer that intercepts all outgoing HTTP GET requests in Bird, reducing X API costs and enabling cost visibility. This is the **foundation plan** in a 4-part series that enables the research commands (`search`, `profile`, `thread`, `watchlist`, `usage`).

**Plan series:**

| # | Plan | Status |
|---|------|--------|
| **1** | **Transparent Cache Layer** (this plan) | Active |
| 2 | Search Command | Blocked by Plan 1 |
| 3 | Profile & Thread Commands | Blocked by Plan 1 |
| 4 | Watchlist & Usage Commands | Blocked by Plan 1 |

## Problem Statement

The X API bills per-object returned:

- $0.005 per post read
- $0.010 per user lookup
- $0.010 per post creation

A 5-query research session across 3 pages each (~1,500 tweets) costs ~$7.50. There is no visibility into accumulated costs, and repeated requests for the same data are billed again (outside X's 24hr UTC dedup window). Even existing commands (`bird get`, `bird bookmarks`, `bird me`) pay full price on every invocation.

**Key insight:** X's 24hr dedup is billing-only, not data-staleness. Re-requesting a tweet within the dedup window returns **fresh data** (updated metrics) at **zero cost**. Short cache TTLs (15min) let us get frequent metric updates for free.

## Proposed Solution

A `CachedClient` wrapper around `reqwest::Client` that:

1. **Intercepts** all HTTP requests transparently (handlers change `&reqwest::Client` to `&CachedClient`)
2. **Caches** GET responses in SQLite at `~/.config/bird/cache.db` with per-endpoint TTL defaults
3. **Displays costs** per API call on stderr (stateless estimation, no persistent tracking yet)
4. **Exposes cache management** via `bird cache [clear|stats]`

**Deferred to later plans:** request packing (Plan 2-3), field profiles (Plan 2), persistent usage tracking (Plan 4), full rate limiter (Plan 2).

## Technical Approach

### Architecture

```
                        +-----------+
                        |  Handler  |  (get, me, bookmarks, etc.)
                        +-----+-----+
                              |
                              v
                     +--------+--------+
                     |  CachedClient   |  (new: wraps reqwest + cache store)
                     |  - get()        |
                     |  - request()    |  (pass-through, never cached)
                     +---+--------+----+
                         |        |
              +----------+        +----------+
              v                              v
     +--------+--------+           +---------+---------+
     |     BirdDb      |           |  reqwest::Client  |
     |   (SQLite)      |           |  (HTTP)           |
     +-----------------+           +-------------------+
```

**Key files (new):**

| File | Responsibility | Est. Lines |
|------|---------------|------------|
| `src/cache.rs` | `BirdDb` — SQLite operations (open, get, put, prune, stats, clear) + `CachedClient` wrapper | ~200 |
| `src/cost.rs` | Stateless cost estimation + stderr display formatting | ~50 |

**Key files (modified):**

| File | Change |
|------|--------|
| `src/main.rs` | Add `CachedClient` creation, pass to handlers, add `Cache` command variant, add global cache flags |
| `src/config.rs` | Extend `FileConfig` and `ResolvedConfig` with cache settings |
| `src/raw.rs` | Change `&reqwest::Client` to `&CachedClient`, restructure request building |
| `src/bookmarks.rs` | Change `&reqwest::Client` to `&CachedClient`, restructure request building |
| `src/requirements.rs` | No change needed — `cache` command follows `login`/`doctor` pattern (no auth) |
| `Cargo.toml` | Add `rusqlite`, `rusqlite_migration` dependencies |

> **Architecture insight:** `auth.rs` signatures are NOT changed. Handlers pass `cached_client.http()` to auth functions, keeping the auth layer independent of the cache layer. The dependency arrow flows: `handlers -> CachedClient -> BirdDb` and `handlers -> auth -> reqwest::Client (via cached_client.http())`.

### Storage Engine: rusqlite (bundled SQLite)

**Decision:** rusqlite with `bundled` feature.

| Engine | Binary | SQL | Pure Rust | Verdict |
|--------|--------|-----|-----------|---------|
| rusqlite | +1.5-2MB | Yes | No (bundled C) | **Chosen** — SQL aggregation perfect for future usage tracking |
| redb | +200-500KB | No | Yes | Runner-up — manual aggregation (~50-100 LOC per query) |
| sled | +300-600KB | No | Yes | Rejected — pre-1.0, stalled development |
| Limbo | TBD | Yes | Yes | Future — pure-Rust SQLite rewrite, still beta in 2026 |

**Cargo.toml additions:**

```toml
rusqlite = { version = "0.38", features = ["bundled"] }
rusqlite_migration = "2.4"
```

#### Research Insights: rusqlite Best Practices

- **No `spawn_blocking`:** Single-threaded tokio runtime means blocking SQLite calls are fine. `spawn_blocking` would create unnecessary OS threads. Document assumption with a comment on `BirdDb`.
- **Single connection per invocation:** No connection pool needed. Open once in `main()`, hold for process lifetime, drop on exit.
- **`prepare_cached` for all queries:** Free LRU statement cache (~16 statements). Saves ~50-100μs per repeated query by avoiding SQL re-parsing.
- **Transaction behavior:** Use `TransactionBehavior::Immediate` for writes — fails fast if another process holds a write lock.
- **Binary size mitigation:** Add `strip = true` and `lto = true` to `[profile.release]` in Cargo.toml. Expected total binary: 7.5-8.5 MB (up from 6.8 MB).

### SQLite Schema

```sql
-- HTTP response cache (single table for v1)
CREATE TABLE IF NOT EXISTS cache (
    key         TEXT PRIMARY KEY,  -- SHA-256 hash
    url         TEXT NOT NULL,     -- original URL (for debugging)
    status_code INTEGER NOT NULL,  -- 200, etc.
    body        BLOB NOT NULL,     -- response body
    body_size   INTEGER NOT NULL,  -- length(body) in bytes
    created_at  INTEGER NOT NULL,  -- unix timestamp
    ttl_seconds INTEGER NOT NULL   -- per-endpoint TTL
);

CREATE INDEX IF NOT EXISTS idx_cache_created ON cache(created_at);
```

**Schema versioning:** Uses `PRAGMA user_version` (built into SQLite header, no extra table). Checked on every open via `rusqlite_migration`.

```rust
use rusqlite_migration::{Migrations, M};

const MIGRATIONS: Migrations<'static> = Migrations::from_slice(&[
    M::up(
        "CREATE TABLE IF NOT EXISTS cache (
            key         TEXT PRIMARY KEY,
            url         TEXT NOT NULL,
            status_code INTEGER NOT NULL,
            body        BLOB NOT NULL,
            body_size   INTEGER NOT NULL,
            created_at  INTEGER NOT NULL,
            ttl_seconds INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_cache_created ON cache(created_at);"
    ),
]);

#[cfg(test)]
#[test]
fn migrations_are_valid() { MIGRATIONS.validate().unwrap(); }
```

#### Research Insights: Schema Simplification

**Removed from original plan (YAGNI):**

- `method` column — only GETs are cached; method is always GET
- `content_type` column — never read by any code
- `auth_type`, `username` columns — baked into the cache key hash
- `last_accessed` column + LRU index — TTL-based expiry is sufficient; LRU adds an UPDATE on every cache hit for negligible benefit in a CLI
- `usage` table + 2 indexes — deferred to Plan 4 when `bird usage` is built
- `meta` table — replaced by `PRAGMA user_version`

**Result:** 1 table, 1 index (was 3 tables, 4 indexes).

**Pragmas (set on every connection open):**

```sql
PRAGMA journal_mode = WAL;           -- concurrent readers + single writer
PRAGMA synchronous = NORMAL;         -- safe with WAL, better performance
PRAGMA busy_timeout = 5000;          -- 5s wait on lock (multiple bird processes)
PRAGMA auto_vacuum = INCREMENTAL;    -- reclaim space without full VACUUM
PRAGMA temp_store = MEMORY;          -- keep temp tables in memory
```

### Cache Key Design

**Key formula:** `SHA-256(method + "\0" + normalized_url + "\0" + auth_type + "\0" + username)`

Where:

- `method`: HTTP method string (`GET`)
- `normalized_url`: URL with query parameters sorted alphabetically, ID lists sorted numerically
- `auth_type`: `AuthType` enum value serialized via `Display` trait (type-safe, not `&str`)
- `username`: Effective username from `ResolvedConfig.username`, or `"__app__"` for Bearer (app-only)

**Hash choice:** SHA-256 via the `sha2` crate already in Cargo.toml (used for PKCE). Zero incremental binary cost. ~350ns per hash on ~100-byte inputs — negligible next to SQLite reads (~50-500μs).

**Normalization rules:**

1. Sort all query parameter keys alphabetically
2. For known ID parameters (`ids`, `usernames`), sort the comma-separated values
3. Skip scheme/host normalization — all URLs are `https://api.x.com` (hardcoded). Add `debug_assert!` instead.

**Why include username:** Fixes the multi-user correctness bug where `GET /2/users/me` for user A would be served from cache to user B.

#### Research Insights: Cache Key

- **URL normalization is necessary for correctness** (~1μs cost). Without it, `?ids=123,456` and `?ids=456,123` are different cache keys for identical API responses.
- **Use `&AuthType` enum, not `&str`** — the plan originally used `auth_type: &str` which breaks compile-time type safety. Use the existing `AuthType` enum from `requirements.rs` and derive/implement `Display` for the string representation needed in the hash.
- **Consider a `CacheContext` struct** to group `auth_type` and `username` rather than passing them as separate parameters.

### Per-Endpoint TTL Defaults

| Endpoint Pattern | Default TTL | Rationale |
|-----------------|-------------|-----------|
| `/2/tweets/search/*` | 15 min | Search results change frequently; free within 24hr dedup |
| `/2/users/*` | 1 hour | Profile data changes infrequently |
| `/2/users/by/*` | 1 hour | Same as above |
| `/2/tweets/{id}` | 15 min | Metrics change; re-fetch free within 24hr dedup |
| `/2/users/{id}/bookmarks` | 15 min | Bookmarks change with user activity |
| Default (all other GET) | 15 min | Safe fallback |

### Cache Control Flags

Three global flags added to the `Cli` struct:

| Flag | Behavior |
|------|----------|
| `--refresh` | Bypass cache read, still write response to cache |
| `--cache-ttl <seconds>` | Override endpoint default TTL for this request |
| `--no-cache` | Disable cache entirely (no read, no write) |

**Precedence:** `--no-cache` > `--refresh` > `--cache-ttl`. If `--no-cache` is set, `--refresh` and `--cache-ttl` are silently ignored.

**Environment variable:** `BIRD_NO_CACHE=1` — equivalent to `--no-cache`. Resolved in `ResolvedConfig::load()` following the `NO_COLOR` precedent.

### Cache Exclusions

The following are **never cached:**

1. All non-GET requests (POST, PUT, DELETE)
2. Auth endpoints: any URL containing `/oauth2/token`
3. Requests with `pagination_token` query parameter (tokens are ephemeral)
4. Failed responses (non-2xx status codes)

### CachedClient API

```rust
// src/cache.rs

/// Application database: cache storage + future usage tracking.
/// Named BirdDb (not CacheStore) because it will grow to include usage tracking in Plan 4.
pub struct BirdDb {
    conn: rusqlite::Connection,
}

/// Cache context for key computation (type-safe, not strings).
pub struct CacheContext<'a> {
    pub auth_type: &'a AuthType,  // reuse enum from requirements.rs
    pub username: Option<&'a str>,
}

pub struct CachedClient {
    http: reqwest::Client,
    db: Option<BirdDb>,          // None if DB unavailable (corrupted, etc.)
    cache_opts: CacheOpts,
}

pub struct CacheOpts {
    pub no_cache: bool,
    pub refresh: bool,
    pub cache_ttl: Option<u64>,  // matches CLI flag name
}

impl CachedClient {
    /// Create a new CachedClient. If cache DB is unavailable, degrades to no-cache.
    pub fn new(
        http: reqwest::Client,
        cache_path: &Path,
        cache_opts: CacheOpts,
    ) -> Self { ... }

    /// GET request with caching. Primary entry point for handlers.
    pub async fn get(
        &self,
        url: &str,
        ctx: &CacheContext<'_>,
        headers: HeaderMap,
    ) -> Result<ApiResponse, Box<dyn Error + Send + Sync>> { ... }

    /// POST/PUT/DELETE — pass-through, no caching.
    pub async fn request(
        &self,
        method: Method,
        url: &str,
        headers: HeaderMap,
        body: Option<String>,
    ) -> Result<ApiResponse, Box<dyn Error + Send + Sync>> { ... }

    /// Inner HTTP client ref (for auth operations that bypass cache).
    pub fn http(&self) -> &reqwest::Client { ... }
}

/// Response from CachedClient (covers both cache hits and fresh responses).
pub struct ApiResponse {
    pub status: StatusCode,
    pub body: String,
    pub headers: HeaderMap,      // needed for rate limit headers
    pub cache_hit: bool,
}

// Custom Debug: redact body (may contain sensitive data like DMs/bookmarks)
impl fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("cache_hit", &self.cache_hit)
            .field("body_len", &self.body.len())
            .finish()
    }
}
```

#### Research Insights: API Design

**Naming fixes (pattern recognition):**

- `CacheStore` renamed to `BirdDb` — reflects dual responsibility (cache now, usage later)
- `CachedResponse` renamed to `ApiResponse` — the struct is returned for both hits and misses
- `CacheOpts.ttl_override` renamed to `cache_ttl` — matches CLI flag `--cache-ttl`

**Handler migration is NOT a type swap.** Every call site that does `client.get(url).header(...).send().await?` must be rewritten to `client.get(url, &ctx, headers).await?`. The plan must include before/after examples:

```rust
// BEFORE (raw.rs):
let res = client.get(&url)
    .header("Authorization", format!("Bearer {}", access))
    .send().await?;
let text = res.text().await?;

// AFTER (raw.rs):
let mut headers = HeaderMap::new();
headers.insert("Authorization", format!("Bearer {}", access).parse()?);
let ctx = CacheContext { auth_type: &resolved_auth_type, username: config.username.as_deref() };
let response = client.get(&url, &ctx, headers).await?;
let text = response.body;
```

**`get()` method should be a clear pipeline** of independently testable steps:

```rust
pub async fn get(&self, url: &str, ctx: &CacheContext<'_>, headers: HeaderMap) -> Result<ApiResponse, ...> {
    let key = compute_cache_key("GET", url, ctx);       // cache.rs
    if let Some(cached) = self.try_cache_hit(&key)? { return Ok(cached); }  // cache.rs
    let response = self.http.get(url).headers(headers).send().await?;       // reqwest
    self.try_cache_write(&key, &response)?;              // cache.rs
    let cost = cost::estimate_and_display(&response);    // cost.rs (pure + I/O)
    Ok(ApiResponse { ... })
}
```

**Error handling posture:** Cache failures (SQLite errors, corrupted DB, disk full) are **never fatal**. If a cache operation fails:

1. Log warning to stderr: `[cache] warning: <reason>`
2. Proceed as if `--no-cache` were set
3. Suggest `bird cache clear` if the error persists

The `Option<BirdDb>` pattern (None when DB is unavailable) is new to the codebase — add a comment documenting the graceful degradation convention.

### Cost Display

**Stateless stderr display only (no persistent tracking in Plan 1):**

```
[cost] ~$0.05 (10 tweets, cache miss)
[cost] $0.00 (5 tweets, cache hit)
```

```rust
// src/cost.rs — purely functional estimation + stderr display

const COST_PER_TWEET_READ: f64 = 0.005;
const COST_PER_USER_READ: f64 = 0.010;

pub struct CostEstimate {
    pub tweets_read: u32,
    pub users_read: u32,
    pub estimated_usd: f64,
    pub cache_hit: bool,
}

/// Count objects in a JSON response body and estimate cost.
/// Pure function — no I/O.
pub fn estimate_cost(body: &serde_json::Value, endpoint: &str, cache_hit: bool) -> CostEstimate { ... }

/// Format and print cost to stderr. Separated from estimation for testability.
pub fn display_cost(estimate: &CostEstimate, use_color: bool) { ... }
```

#### Research Insights: Cost Tracking

**Deferred to Plan 4:** The `usage` table, `log_usage()`, `query_usage()`, and persistent cost accumulation. The stateless stderr display provides immediate value without database complexity.

**What gets counted:**

- `data` array items: tweets or users depending on endpoint
- `includes.users`: additional user objects (billed separately)
- `includes.tweets`: referenced tweets (billed separately)

**Open question for Plan 4:** Are expanded objects (`includes.*`) billed separately? Verify empirically by comparing estimated vs. actual usage.

### Config Changes

**`config.toml` additions (deferred):** The simplicity review found that config.toml cache settings are premature — the defaults (enabled=true, 100MB) work for virtually all users. The `--no-cache` flag and `BIRD_NO_CACHE=1` env var cover the "disable" case. Add `[cache]` config section when a user asks for it.

**`ResolvedConfig` additions:**

```rust
pub struct ResolvedConfig {
    // ... existing fields ...
    pub cache_path: PathBuf,      // ~/.config/bird/cache.db
    pub cache_enabled: bool,      // from BIRD_NO_CACHE env var
    pub cache_max_size_mb: u64,   // hardcoded 100 for now
}
```

`BIRD_NO_CACHE=1` is resolved in `ResolvedConfig::load()` alongside existing env var resolution.

### Cache Management Commands

**`bird cache clear`** — Delete all cache entries:

```
$ bird cache clear
Cleared 847 cache entries (42.3 MB).
```

Note: Confirmation message goes to **stderr** (it's a side-effect confirmation, not data).

**`bird cache stats`** — Show cache status (JSON by default, matching `doctor` convention):

```json
{
  "path": "~/.config/bird/cache.db",
  "size_mb": 42.3,
  "max_size_mb": 100,
  "entries": 847,
  "oldest_seconds_ago": 7200,
  "newest_seconds_ago": 180,
  "healthy": true
}
```

With `--pretty`:

```
Cache: ~/.config/bird/cache.db
Size:  42.3 MB / 100 MB limit
Entries: 847
Oldest: 2h ago | Newest: 3m ago
```

#### Research Insights: Command Registration

**CRITICAL FIX:** The `cache` command must NOT register with `AuthType::None` in `requirements_for_command()`. The `resolve_token_for_command()` function has no handler for `AuthType::None` in its accepted array iteration — it would fall through to `Err(auth_required_error("cache"))`.

Instead, follow the `login`/`doctor` pattern: return `None` from `requirements_for_command()` and handle the `Cache` command in `run()` without auth resolution.

**Do NOT add `"cache"` to `command_names_with_auth()`** — the cache command is always available, so it doesn't belong in the auth-availability matrix shown by `bird doctor`.

**Use nested subcommand enum** (first in the project, but idiomatic clap):

```rust
#[derive(clap::Subcommand)]
enum CacheAction {
    Clear,
    Stats {
        #[arg(long)]
        pretty: bool,
    },
}

// In Command enum:
Cache {
    #[command(subcommand)]
    action: CacheAction,
},
```

**`bird doctor` integration:** Add a `cache` section to doctor output:

```json
{
  "cache": {
    "path": "~/.config/bird/cache.db",
    "exists": true,
    "size_mb": 42.3,
    "max_size_mb": 100,
    "entries": 847,
    "healthy": true
  }
}
```

### Auto-Pruning Strategy

**Trigger:** Every 20th cache write (not every write — avoids `SUM(body_size)` scan on every insert).

```rust
self.write_count += 1;
if self.write_count % 20 == 0 {
    self.prune_if_needed(max_bytes)?;
}
```

**Algorithm:**

1. Check total `SUM(body_size)` from cache table
2. If under `max_size_mb`, done
3. If over, delete oldest entries by `created_at ASC` until under 90% of limit (single SQL statement)
4. Never run `PRAGMA incremental_vacuum` on normal writes — only during `bird cache clear`

#### Research Insights: Pruning Performance

**Original plan:** Check `SUM(body_size)` on every write. For 10,000 entries, this is 10-50ms per write — exceeds the <5ms latency target.

**Better approach: Counter-cache** (architecture + performance reviewers agree):

```sql
-- Track total size in a lightweight counter
-- Updated atomically on INSERT/DELETE via the BirdDb methods
```

Or the simpler Nth-write approach above. Either eliminates per-write scans.

**`PRAGMA incremental_vacuum` takes 50-200ms** — only run during `bird cache clear`, not on normal pruning.

**TTL-based expiry is sufficient** for v1 without LRU. Most entries expire via TTL (15min-1hr) long before hitting 100MB. Simple `DELETE FROM cache WHERE created_at + ttl_seconds < unixepoch()` on startup or Nth-write is good enough.

### Graceful Degradation

Single pattern for all SQLite failures:

```rust
// If any SQLite operation fails:
// 1. Log warning: eprintln!("[cache] warning: {reason}");
// 2. Proceed without cache (equivalent to --no-cache)
// 3. If persistent, suggest: "Run `bird cache clear` to reset the cache database."
```

Specific recovery: On `ErrorCode::DatabaseCorrupt` or `ErrorCode::NotADatabase`, delete the DB file (and WAL/SHM sidecars) and recreate. The cache is not primary data — deletion is safe.

### DB File Permissions

The cache DB file is created with Unix `0o600` permissions, matching the `tokens.json` precedent in `src/auth.rs:245-257`.

#### Research Insights: SECURITY (HIGH) — WAL/SHM Sidecar Files

**Finding F-01 (security-sentinel):** SQLite's WAL mode creates `cache.db-wal` and `cache.db-shm` files. These contain raw database page images including cached response bodies. With the default umask (`0o022`), these files may be created as `0o644` (world-readable), undermining the `0o600` on the main DB.

**Remediation:**

1. **Pre-create the DB file** with `OpenOptions::new().mode(0o600)` before passing to `rusqlite::Connection::open()` (rusqlite creates files internally, so the auth.rs pattern can't be used directly).
2. **Verify permissions** on all three files (`cache.db`, `cache.db-wal`, `cache.db-shm`) after opening.
3. **Alternative:** Set `umask(0o077)` before opening, restore after. Safer but process-wide.

```rust
// Pre-create with restrictive permissions, then open
fn open_cache_db(path: &Path) -> Result<Connection> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::OpenOptions::new()
            .write(true).create(true).truncate(false)
            .mode(0o600)
            .open(path)?;
    }
    let conn = Connection::open(path)?;
    // ... set pragmas ...
    Ok(conn)
}
```

### Additional Security Findings

**F-02 (MEDIUM): Stale cache after token revocation.** On `bird login` (new OAuth2 authorization), auto-clear the cache. A new authorization grant is a strong signal that previous cached data may belong to a different context.

**F-03 (HIGH): Usage/stats endpoint column.** When the `usage` table is added in Plan 4, the `endpoint` column must store the URL **path only** (e.g., `/2/tweets/search/recent`), with all query parameters stripped. Search queries in URLs (e.g., `query=from%3Ajournalist+corruption`) would create a permanent, queryable log of sensitive research.

**F-07 (MEDIUM): Malicious cache.db.** On open:

- Reject databases with SQLite triggers (`SELECT count(*) FROM sqlite_master WHERE type='trigger'`)
- Cap individual entry reads at 50MB (prevents OOM from crafted BLOBs)
- If `PRAGMA user_version` > current version, treat as corrupted (not read-only)

**F-10 (MEDIUM): DM endpoints.** Document that DM content fetched via `bird get /2/dm_conversations/...` will be cached for 15 minutes. Users should use `--no-cache` for DM operations. Add DM exclusion if a dedicated DM command is ever built.

## Implementation Phases

### Phase 1: BirdDb + Schema (Foundation)

- [x] Add `rusqlite` and `rusqlite_migration` to `Cargo.toml`
- [x] Add `strip = true` and `lto = true` to `[profile.release]`
- [x] Create `src/cache.rs` with `BirdDb` struct
  - [x] `open(path)` — pre-create file with 0o600, open connection, set pragmas, run migrations
  - [x] Verify WAL/SHM file permissions after open
  - [x] Reject databases with triggers (anti-tampering)
  - [x] `get(key)` — lookup by cache key, check TTL via `prepare_cached`
  - [x] `put(key, entry)` — insert/replace cache entry via `prepare_cached`
  - [x] `delete_expired()` — remove entries past TTL
  - [x] `prune_if_needed(max_bytes)` — delete oldest by `created_at` until under limit
  - [x] `stats()` — entry count, total size, health check
  - [x] `clear()` — delete all cache entries + `PRAGMA incremental_vacuum`
  - [x] Add passive WAL checkpoint on `Drop`:

    ```rust
    impl Drop for BirdDb {
        fn drop(&mut self) {
            let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
        }
    }
    ```

- [x] Unit tests (using `Connection::open_in_memory()`): open, put/get, TTL expiry, pruning, migration validation
- [x] Integration tests (using `tempfile::tempdir()`): WAL mode, file permissions, corruption recovery

### Phase 2: CachedClient Wrapper + Handler Migration

- [x] Add `CachedClient`, `CacheOpts`, `CacheContext`, `ApiResponse` to `src/cache.rs`
  - [x] `new()` — wraps reqwest::Client + BirdDb (graceful degradation if DB unavailable)
  - [x] `get()` — cache-aware GET with clear pipeline steps
  - [x] `request()` — pass-through for non-GET methods
  - [x] `http()` — expose inner reqwest::Client for auth operations
  - [x] Cache key computation with SHA-256 + URL normalization
  - [x] Endpoint TTL matching (prefix-based, most-specific first)
  - [x] Cache exclusion checks (non-GET, auth endpoints, pagination_token)
- [x] **Migrate all handlers (single atomic commit):**
  - [x] `src/raw.rs` — restructure request building (see before/after example above)
  - [x] `src/bookmarks.rs` — restructure; `/2/users/me` call should go through cached path
  - [x] `src/main.rs` — `Me` command handler
  - [x] Auth functions (`auth.rs`, `login.rs`) use `cached_client.http()` — **no signature changes**
- [x] Update `src/main.rs` to create `CachedClient` and pass to `run()`
- [x] Add global cache flags (`--refresh`, `--no-cache`, `--cache-ttl`) to `Cli` struct
- [x] Support `BIRD_NO_CACHE=1` in `ResolvedConfig::load()`
- [x] Unit tests: cache hit/miss, flag precedence, exclusion rules, graceful degradation

### Phase 3: Cost Display

- [x] Create `src/cost.rs` with stateless cost estimation
  - [x] `estimate_cost()` — pure function: count objects in JSON response, compute cost
  - [x] `display_cost()` — format and print to stderr (respects `use_color`)
- [x] Wire cost display into `CachedClient::get()` (after each response)
- [x] Unit tests: cost estimation for various response shapes (empty data, array, includes)

### Phase 4: Cache CLI + Doctor

- [x] Add `CacheAction` subcommand enum to `src/main.rs`
- [x] Add `Cache` variant to `Command` enum
- [x] Handle `Cache` in `run()` without auth resolution (like `Doctor`)
- [x] Implement `bird cache clear` (confirmation to stderr)
- [x] Implement `bird cache stats` (JSON to stdout, `--pretty` for human-readable)
- [x] Update `bird doctor` to include cache status section
- [x] Add `cache_path`, `cache_enabled`, `cache_max_size_mb` to `ResolvedConfig`
- [x] Auto-clear cache on `bird login` (security: stale data after re-auth)
- [x] Integration test: full cache lifecycle (miss -> hit -> stats -> clear -> miss)

## Alternative Approaches Considered

### 1. reqwest-middleware for caching

**Rejected.** Cannot support future request packing (splitting multi-ID requests). Adds an external dependency for something implementable in ~200 lines.

### 2. redb instead of rusqlite

**Considered seriously.** redb is pure Rust with zero unsafe code and ~200-500KB binary impact. However, future usage tracking (Plan 4) requires SQL aggregation queries that are trivial in SQL but require 50-100 lines of manual iteration per query in redb.

### 3. Helper function instead of CachedClient wrapper

**Rejected.** A `cached_get()` helper would be simpler but makes caching opt-in per handler. The wrapper ensures all handlers benefit automatically.

### 4. In-memory cache (no persistence)

**Rejected.** CLI processes are short-lived — no benefit across invocations.

### 5. Full feature set in Plan 1

**Rejected after simplicity review.** The original plan included request packing, field profiles, a full rate limiter, and persistent usage tracking — all for a codebase with 3 commands that make HTTP calls. Deferred to later plans per YAGNI.

## Acceptance Criteria

### Functional Requirements

- [x] All existing commands (`get`, `me`, `bookmarks`, `doctor`, `login`) work identically
- [x] GET requests are transparently cached with correct per-endpoint TTLs
- [x] Cache key includes method, normalized URL, auth type, and username
- [x] `--refresh` bypasses cache read but writes the response
- [x] `--no-cache` disables cache entirely
- [x] `--cache-ttl <seconds>` overrides endpoint default TTL
- [x] `BIRD_NO_CACHE=1` environment variable works like `--no-cache`
- [x] Non-GET requests, auth endpoints, and paginated requests are never cached
- [x] Cost estimate displayed on stderr for every API call
- [x] `bird cache clear` removes all cache entries
- [x] `bird cache stats` shows entry count, size (JSON default, `--pretty` for human)
- [x] `bird doctor` includes cache status section
- [x] Cache DB file and WAL/SHM sidecars have 0o600 permissions
- [x] Corrupted DB degrades to no-cache mode with stderr warning
- [x] Cache auto-cleared on `bird login`

### Non-Functional Requirements

- [x] Cache lookup adds < 5ms latency to requests
- [x] Binary size increase < 3MB (rusqlite bundled)
- [x] No regression in existing command behavior
- [x] Concurrent bird processes do not corrupt the DB (WAL mode + busy_timeout)

### Quality Gates

- [x] Unit tests: BirdDb operations, cache key normalization, cost estimation, TTL matching, flag precedence, pruning logic, migration validation
- [x] Integration test: full cache lifecycle, WAL mode, file permissions, corruption recovery
- [x] All existing tests pass
- [x] `cargo clippy` clean
- [x] All SQL uses parameterized queries (`params![]`) — no string interpolation
- [x] `ApiResponse` Debug impl redacts body content
- [x] No `reqwest::Client::new()` calls outside `main.rs`
- [x] No secrets in cached data (auth endpoints excluded, tokens not in cache keys)

## Success Metrics

- Cache hit rate > 50% during typical research sessions
- Cost visibility: every API call shows estimated cost on stderr
- Zero regressions in existing command behavior
- Cache layer adds < 5ms overhead per request

## Dependencies & Prerequisites

- `rusqlite 0.38` with `bundled` feature
- `rusqlite_migration 2.4`
- No changes to existing X API scopes
- No external services or accounts required

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| rusqlite adds compile time | Medium | Low | Only initial build; cached builds fast |
| Cache DB corruption | Low | Low | WAL mode + graceful degradation + auto-recreate |
| Cost estimates diverge from actual billing | Medium | Medium | `bird usage --sync` (Plan 4) compares estimated vs. actual |
| Binary size increase | Low | Medium | strip + lto brings it to 7.5-8.5MB |
| WAL sidecar permissions | Medium | High | Pre-create file with 0o600, verify after open |

## Future Considerations (Deferred to Later Plans)

| Feature | Target Plan | Why Deferred |
|---------|------------|--------------|
| Request packing (multi-ID cache splitting) | Plan 2-3 | Zero current consumers; highest-risk code |
| Field profiles (minimal/standard/full) | Plan 2 | No current command uses them |
| Persistent usage tracking (SQLite table) | Plan 4 | `bird usage` command doesn't exist yet |
| Full adaptive rate limiter | Plan 2 | 2-line sleep is sufficient for 1 paginated command |
| Config.toml `[cache]` section | When requested | Defaults work for all users |
| Response compression (zstd) | If 100MB limit frequently hit | Optimization, not needed for v1 |
| Limbo migration (pure-Rust SQLite) | When Limbo reaches 1.0 | `BirdDb` abstraction makes this a drop-in |

## Documentation Plan

- [x] Update `docs/CLI_DESIGN.md` with cache architecture section
- [x] Update `--help` text for new global flags
- [x] Add cache troubleshooting to `bird doctor` output
- [x] Document that DM content accessed via `bird get` will be cached (use `--no-cache`)

## References & Research

### Internal References

- Brainstorm: `docs/brainstorms/2026-02-11-research-commands-and-caching-brainstorm.md`
- Security audit: `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md`
- Command pattern template: `src/raw.rs`
- Streaming pagination: `src/bookmarks.rs`
- Config loading: `src/config.rs:78-158`
- Auth requirements: `src/requirements.rs:40-81`
- File permissions pattern: `src/auth.rs:245-257`
- Shared client creation: `src/main.rs:292-296`

### External References

- X API billing: $0.005/tweet, $0.010/user, 24hr UTC dedup
- X API rate limit headers: `x-rate-limit-limit`, `x-rate-limit-remaining`, `x-rate-limit-reset`
- rusqlite docs: https://docs.rs/rusqlite/0.38.0
- rusqlite_migration: https://github.com/cljoly/rusqlite_migration
- SQLite WAL mode: https://www.sqlite.org/wal.html
- SQLite pragmas: https://www.sqlite.org/pragma.html
- SQLite performance tuning: https://phiresky.github.io/blog/2020/sqlite-performance-tuning/

### Review Agent Findings (Incorporated)

| Agent | Key Findings | Action Taken |
|-------|-------------|-------------|
| architecture-strategist | CachedClient SRP, auth.rs coupling, PRAGMA user_version | Split concerns, kept auth.rs unchanged, use user_version |
| code-simplicity-reviewer | 4 YAGNI phases, 5 files -> 2, schema 3 tables -> 1 | Cut phases 4-6, deferred usage table |
| performance-oracle | No spawn_blocking, SHA-256 in deps, Nth-write pruning | All adopted |
| security-sentinel | 2 HIGH (WAL perms, query params), 4 MEDIUM | WAL fix added, query param stripping noted for Plan 4 |
| pattern-recognition-specialist | AuthType::None bug, &str type safety, JSON output convention | All fixed |
| best-practices-researcher | rusqlite_migration, prepare_cached, passive checkpoint | All adopted |
| framework-docs-researcher | Comprehensive rusqlite API patterns | Informed implementation details |
| security-audit-learnings | 8 of 15 prior findings apply, custom Debug, parameterized SQL | Quality gates updated |
