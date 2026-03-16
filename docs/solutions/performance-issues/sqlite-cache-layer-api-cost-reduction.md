---
title: "SQLite Cache Layer for API Cost Reduction"
category: performance-issues
severity: high
tags: [caching, http, api-integration, cost-optimization, sqlite, rusqlite, bird-cli]
module: cache
symptom: "Every CLI invocation makes live API calls, incurring X API billing costs even for repeated identical requests"
root_cause: "No HTTP response caching — each `bird get`, `bird me`, `bird bookmarks` call hits the network"
date_resolved: 2026-02-11
files_changed:
  - src/cache.rs (created)
  - src/cost.rs (created)
  - src/main.rs
  - src/raw.rs
  - src/bookmarks.rs
  - src/config.rs
  - src/requirements.rs
  - src/doctor.rs
  - Cargo.toml
---

## Problem

The Bird CLI charged real money on every API call — $0.005/tweet, $0.010/user. Running `bird me` twice in a row costs the same as running it once. For interactive development, repeated queries during debugging, or scripted pipelines re-fetching the same data, costs add up with zero benefit.

## Solution

A transparent SQLite-backed HTTP response cache that intercepts all GET requests. The `CachedClient` wrapper replaces the raw `reqwest::Client` everywhere — callers don't know they're hitting cache.

### Architecture

```
┌──────────┐     ┌──────────────┐     ┌──────────┐     ┌─────────┐
│ bird CLI │────▶│ CachedClient │────▶│  BirdDb   │────▶│ SQLite  │
│ (main)   │     │ (wrapper)    │     │ (cache.rs)│     │ (WAL)   │
└──────────┘     └──────┬───────┘     └──────────┘     └─────────┘
                        │ cache miss
                        ▼
                  ┌──────────┐
                  │ reqwest  │──▶ api.x.com
                  │ (HTTP)   │
                  └──────────┘
```

### Key Design Decisions

1. **`Option<BirdDb>` graceful degradation** — Cache failures are never fatal. If the DB can't open (corrupted, disk full), the CLI falls back to direct HTTP. This is critical for a CLI tool where a broken cache should never block work.

2. **SHA-256 cache keys** — `hash(method + "\0" + normalized_url + "\0" + auth_type + "\0" + username)`. This ensures different users and auth contexts never collide, while URL normalization (sorted query params, sorted ID lists) maximizes cache hits.

3. **Per-endpoint TTL defaults** — User profiles (1h) change less often than tweets/bookmarks (15min). The `--cache-ttl` flag overrides for any request.

4. **`CachedClient` wrapper pattern** — Instead of modifying every handler, one wrapper intercepts at the HTTP layer. Handlers call `client.get()` the same way they called `client.http().get()`. Non-GET methods (`POST`, `PUT`, `DELETE`) pass through uncached.

5. **Single connection, no pool** — Bird is a short-lived CLI process with a single-threaded tokio runtime. A connection pool would be unnecessary complexity.

6. **WAL mode** — Allows concurrent reads from multiple bird processes (e.g., parallel shell commands) without blocking.

7. **Anti-tamper trigger check** — On open, rejects any database containing SQLite triggers. Prevents a class of supply-chain attacks where a malicious `.db` file could execute code.

### Cache Exclusions

Some URLs are never cached:

- `/oauth2/token` — Auth token exchange must always be fresh
- URLs containing `pagination_token=` — Each page cursor is unique, caching would return wrong pages

### CLI Interface

```bash
bird --no-cache get /2/users/me    # Bypass cache entirely
bird --refresh get /2/users/me     # Skip cache read, still write response
bird --cache-ttl 60 get /2/tweets/123  # Override TTL to 60 seconds
bird cache stats                   # JSON cache metrics
bird cache stats --pretty          # Human-readable cache info
bird cache clear                   # Delete all cached responses
```

### Cost Display

Every API response prints a cost estimate to stderr:

```
[cost] ~$0.0100 (1 user, cache miss)
[cost] $0.00 (cache hit)
[cost] ~$0.0150 (3 tweets, cache miss)
```

Cost is stateless — estimated per response from the JSON body, not tracked persistently (that's Plan 4).

## Implementation Details

### Files

| File | Role |
|------|------|
| `src/cache.rs` | `BirdDb` (SQLite ops), `CachedClient` (transparent wrapper), cache key computation, URL normalization, TTL defaults |
| `src/cost.rs` | Stateless cost estimation + stderr display |
| `src/main.rs` | CLI flags (`--refresh`, `--no-cache`, `--cache-ttl`), `bird cache` subcommand, `CachedClient` construction |
| `src/raw.rs` | Rewritten to use `CachedClient` instead of raw `reqwest::Client` |
| `src/bookmarks.rs` | Rewritten to use `CachedClient` with per-page cost display |
| `src/config.rs` | Added `cache_path`, `cache_enabled`, `cache_max_size_mb` to `ResolvedConfig` |
| `src/requirements.rs` | Added `Display` impl for `AuthType` (used in cache key computation) |
| `src/doctor.rs` | Added `CacheStatus` section to doctor report |

### Dependencies Added

```toml
rusqlite = { version = "0.38", features = ["bundled"] }
rusqlite_migration = "2.4"
```

`sha2` was already a dependency. The inline `hex` module avoids adding another crate.

### Auto-Pruning

Every 20th cache write, the system:
1. Deletes expired entries (past TTL)
2. If still over `max_size_mb`, deletes oldest entries until under 90% of limit

### Login Auto-Clear

After `bird login`, the cache is cleared automatically. This prevents stale data from a previous auth context being served after re-authentication.

### File Permissions

The cache DB file is pre-created with `0o600` permissions before SQLite opens it. This ensures WAL and SHM sidecar files inherit restrictive permissions — important since the cache may contain private API responses.

## Gotchas

1. **`const MIGRATIONS` doesn't work in Rust** — `Migrations::from_slice(&[M::up(...)])` fails with "destructor cannot be evaluated at compile-time" because `M<'_>` has a destructor. Use `fn migrations() -> Migrations<'static>` with `Migrations::new(vec![...])` instead.

2. **`rusqlite_migration 2.4` requires `&mut Connection`** — The `to_latest()` method takes `&mut conn`, not `&conn`. Easy to miss since older versions accepted `&conn`.

3. **OAuth1 GET requests go through the cache** — Since the cache-auth-layer-unification refactor, `oauth1_request()` checks the cache before OAuth1 signing for GET requests. POST/PUT/DELETE remain uncached. Cache check happens before credential extraction and HMAC-SHA1 signing to avoid wasted crypto on cache hits.

4. **`?` operator doesn't auto-convert to `BirdError`** — When using `serde_json::to_string()` inside `run()`, the error type doesn't implement `From<serde_json::Error>` for `BirdError`. Wrap with `.map_err(|e| BirdError::Command { name: "cache", source: e.into() })`.

5. **Cache hits return empty `HeaderMap`** — Cached responses don't store headers (only body + status). If a future feature needs rate-limit headers, those must come from live responses.

## Testing

28 tests total (14 cache + 7 cost + 7 doctor):

- `cache::tests` — migrations valid, put/get, expiry, pruning, clear, stats, cache key computation, URL normalization, skip logic, TTL defaults, hex encoding, debug redaction
- `cost::tests` — cache hit zero cost, tweet/user counting, includes counting, bookmarks endpoint, empty response
- `doctor::tests` — updated with `no_cache_client()` helper

## Migration Notice (2026-02-18)

This request-level cache (`CachedClient` + `cache.db`) has been replaced by an entity-level store (`BirdClient` + `bird.db`). The new system:

- Stores entities (tweets, users) by ID instead of full HTTP responses
- Uses UTC-day freshness instead of per-endpoint TTLs (aligned with X API billing)
- Splits batch requests to only fetch stale/missing entities
- Shares entity data across auth methods (OAuth1/OAuth2/Bearer)

See `src/db/` for the new implementation: `db.rs` (entity store), `client.rs` (transport layer), `usage.rs` (usage tracking).

The old `src/cache/` module has been removed. The concepts in this document (graceful degradation, WAL mode, anti-tamper, file permissions, cost display) carry forward into the new system.

## Prevention Strategies

- **Regression guard**: 49 entity store tests (32 db + 17 client) cover entity CRUD, freshness, batch splitting, decomposition, and cache key logic.
- **Cost visibility**: The stderr `[cost]` line makes billing impact visible on every request — developers notice cost problems immediately.
- **Graceful degradation test**: The `Option<BirdDb>` pattern means any store test that opens a real DB also implicitly tests the fallback path when `db = None`.
- **Permission test**: Verify `0o600` on bird.db in CI to catch permission regressions.
