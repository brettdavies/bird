---
title: "chore: Watchlist & Usage Deferred Items"
type: chore
date: 2026-02-12
depends_on: "2026-02-11-feat-watchlist-usage-commands-plan"
---

# chore: Watchlist & Usage Deferred Items

Items deferred from the watchlist/usage implementation that should be addressed in a follow-up.

## Deferred Items

### 1. Integrate `log_usage()` into `CachedClient::get()`

**Priority:** High — without this, `bird usage` shows no data.

`BirdDb::log_usage()` and `UsageLogEntry` are implemented and tested but not yet called. Wire it into `CachedClient::get()` so every API response (and cache hit) is logged to the `usage` table.

**Location:** `src/cache.rs`, inside `CachedClient::get()` after the cache hit path and after the HTTP response path.

**What to log:**
- `endpoint`: the URL path (strip query params for grouping)
- `method`: "GET"
- `object_type`: "tweet" or "user" based on endpoint (reuse `cost::estimate_cost` logic)
- `object_count`: count from response body
- `estimated_cost`: from `cost::estimate_cost`
- `cache_hit`: true/false
- `username`: from `CacheContext`

### 2. Integration tests for watchlist lifecycle

**Priority:** Medium — unit tests cover individual operations; this verifies end-to-end.

Write integration tests that:
- `bird watchlist add alice` → `bird watchlist list` shows `["alice"]`
- `bird watchlist add bob` → list shows both
- `bird watchlist remove alice` → list shows only bob
- `bird watchlist check` with mocked HTTP responses verifies NDJSON output shape

### 3. Integration tests for usage with populated data

**Priority:** Medium.

- Insert test rows into an in-memory `usage` table via `BirdDb::log_usage()`
- Call `query_usage_summary()`, `query_daily_usage()`, `query_top_endpoints()`
- Verify aggregation math (totals, cache hit rates, cost breakdowns)

### 4. Handle 429 rate limit on `--sync`

**Priority:** Low — the `/2/usage/tweets` endpoint allows 50 req/15min, unlikely to hit in normal use.

When `client.http_get()` returns HTTP 429 for the usage endpoint:
- Parse `x-rate-limit-reset` header
- Print a warning: `[usage] rate limited. Try again after {time}.`
- Return gracefully (show local data without actuals)

### 5. Graceful handling of missing cache.db for `bird usage`

**Priority:** Low — only happens if user has never run any cached command.

Currently `bird usage` errors with "cache database is not available" if `--no-cache` was used or cache.db doesn't exist. Should print a friendlier message: "No usage data recorded yet. Run some API commands first."
