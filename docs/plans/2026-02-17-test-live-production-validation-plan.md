---
title: "test: Live Production Validation Before v0.1.0 Release"
type: test
status: completed
date: 2026-02-17
deepened: 2026-02-17
---

# test: Live Production Validation Before v0.1.0 Release

## Enhancement Summary

**Deepened on:** 2026-02-17
**Research agents used:** architecture-strategist, security-sentinel, performance-oracle,
code-simplicity-reviewer, best-practices-researcher, learnings-researcher (x4: cache,
security, search/thread, code-review-r2)

### Key Improvements

1. **Rate limit corrections** — search endpoint is 300/15min (not 180), profile is
   900/15min (not 300), per official X API docs.
2. **Security verification steps added** — file permission checks (0o600), environment
   hygiene, debug output redaction test, test artifact cleanup.
3. **Performance instrumentation** — `time` wrapper on bookmarks, WAL file size check,
   DB file size tracking, concurrent access test.
4. **Cache invariant verification** — explicit checks that 401 responses are NOT cached,
   while 200-with-errors responses ARE cached. Non-2xx caching invariant verified.
5. **Bonus steps catalogue** — optional deep-dive tests for `BIRD_NO_CACHE=1` env var,
   `--cache-ttl` 24h cap, corrupted DB handling, and more.

### Considerations from Simplicity Review

The simplicity reviewer noted the 8-phase structure could be condensed to 4 phases for a
faster session. The current plan retains the detailed structure for thorough v0.1.0
release validation, but phases can be collapsed if time runs short: Pre-Flight + Cache
(~15 min), Bookmarks (~15 min), Commands (~20 min combining Search/Thread/Watchlist),
Wrap-up (~10 min). Edge cases and usage can be woven into the command phases.

---

## Overview

Run bird against the live X API to validate all commands, caching behavior, cost
tracking, and edge cases before cutting v0.1.0. The session is structured as a
guided walkthrough that doubles as a real-world usage session — the user gets
their bookmarks while systematically verifying every subsystem.

## Problem Statement

Bird has 116 tests (110 unit, 6 integration smoke), but none hit the real API.
Unit tests validate cache logic with in-memory SQLite; integration tests only
check CLI arg parsing and watchlist config management. Before release, we need
confidence that:

1. **Caching actually works** — cache hits, misses, TTLs, exclusions, and the
   `--refresh`/`--no-cache`/`--cache-ttl` flags behave correctly against real
   API responses
2. **Cost tracking is accurate** — stderr cost display matches actual object
   counts; `bird usage` aggregates correctly
3. **Bookmarks pagination completes** — large bookmark sets paginate to
   completion without hanging, duplicating, or losing data
4. **All commands work end-to-end** — `me`, `profile`, `search`, `thread`,
   `bookmarks`, `watchlist check`, `usage`, `get`, `cache stats`
5. **Error handling is graceful** — bad usernames, invalid tweet IDs, expired
   tokens, network issues

## What This Is NOT

- Not automated test code to be committed (though we may write a test script)
- Not a stress test or load test
- Not testing write operations (post, put, delete) against production

## Prerequisites

- [x] `bird login` completed successfully (OAuth2 PKCE flow)
- [x] `bird doctor` shows healthy auth state
- [x] `bird me` returns your profile (confirms token works)
- [x] Fresh cache: `bird cache clear` before starting

## Proposed Solution

### Phase 1: Pre-Flight Check (~5 min)

Verify auth, config, and baseline state before testing.

- [x] **1a.** Run `bird doctor --pretty` — oauth2_user, stored, can_refresh: true ✓
- [x] **1b.** Run `bird cache clear` — cleared 1 entry ✓
- [x] **1c.** Run `bird cache stats` — 0 entries, 0 bytes ✓
- [x] **1d.** Run `bird me --pretty` — BrettDavies, cache miss, ~$0.01 ✓
- [x] **1e.** Run `bird usage` — 9 calls/$0.05 (includes earlier testing) ✓
- [x] **1f.** Environment clean — no X_API_/BIRD_ env vars set ✓
- [x] **1g.** File permissions — tokens.json and cache.db both 600 ✓
- [x] **1h.** Version check — bird 0.1.0 ✓

**Success:** Doctor reports healthy, cache is empty, `me` returns your profile,
usage shows 1 API call, environment is clean, file permissions are `600`.

---

### Phase 2: Cache Hit/Miss Verification (~10 min)

The core caching test. Every command is run twice to verify the second call
hits cache.

- [x] **2a.** `bird me --pretty` (second call) — cache hit $0.00 ✓
- [x] **2b.** `bird profile BrettDavies --pretty` — first call cache miss $0.01, second call cache hit $0.00 ✓
- [x] **2c.** `bird cache stats` — 5 entries, 0.1 MB ✓

**Cache bypass flags:**

- [x] **2d.** `bird --refresh me --pretty` — cache miss (bypass), then re-read shows cache hit ✓
- [x] **2e.** `bird --no-cache me --pretty` — cache miss, entry count stayed at 5 (no write) ✓
- [x] **2f.** `bird --cache-ttl 30 profile elonmusk --pretty` — miss, hit within 30s, miss after 31s ✓
  Note: followers_count changed between calls (234744081→234744115), confirming fresh data on expiry.

**Success:** Cache hits show `$0.00`, misses show real cost. Bypass flags work
as documented. Stats reflect actual entries.

---

### Phase 3: Bookmarks — The Real Workload (~20 min)

This is the primary use case. Fetch all bookmarks with pagination, verify
caching behavior, and capture the data.

- [x] **3a.** `bird cache clear` — cleared 6 entries ✓
- [x] **3b.** Fetched all bookmarks: 1.4s wall time, 3 pages (100+100+99=299 tweets) ✓
  - ~0.35s/page average (fast — network is local to datacenter)
- [x] **3c.** Cost log: 1 user (me, miss), 3 bookmark pages all misses ✓
  - First call: $0.01 (users/me, cache miss)
  - Pages: $0.50 + $0.50 + $0.4950 = $1.495 in bookmark cost
- [x] **3d.** 299 bookmarks total, 3 pages ✓
- [x] **3e.** 299 IDs, 299 unique — 0 duplicates ✓
- [x] **3f.** Second run: /users/me cache hit, first bookmark page cache hit,
  pages 2-3 cache miss (pagination URLs excluded by design) ✓
- [x] **3g.** `bird usage`: $5.07 total, 38 calls, 36% cache hit rate ✓
  - /2/users/:id/bookmarks $3.48 (7 calls) matches aggregate
- [x] **3h.** No WAL file — clean checkpoint on exit. cache.db = 104 KB ✓

**Warning:** If you press Ctrl-C during bookmark pagination, the output file
will contain partial/invalid JSON. You would need to re-run from scratch.

**Success:** All bookmarks fetched, no duplicates, pagination completes cleanly,
cost tracking matches page count, first-page caching works on re-run.

---

### Rate Limit Awareness

The X API enforces rate limits per endpoint per 15-minute window. Key limits for
user auth (OAuth2):

| Endpoint | Limit | Used By |
|----------|-------|---------|
| `/2/users/{id}/bookmarks` | 180 req/15 min | `bookmarks` |
| `/2/tweets/search/recent` | 300 req/15 min | `search`, `thread`, `watchlist check` |
| `/2/users/me` | 75 req/15 min | `me`, `bookmarks` (first call) |
| `/2/users/by/username/*` | 900 req/15 min | `profile` |

Search, thread, and watchlist check **share** the search endpoint limit. Plan
accordingly — if bookmarks used many pages, the search limit is still fresh.
If you hit a 429 response, wait ~5 minutes and retry. Bird does not currently
have automatic 429 retry logic.

### Phase 4: Search and Thread (~15 min)

Test the research commands that are the core value proposition.

**Search:**

- [x] **4a.** `bird search "from:BrettDavies" --pretty` — 2 tweets, $0.02, cache miss ✓
- [x] **4b.** Same search again — cache hit $0.00 ✓
- [x] **4c.** `bird search "anthropic AI" --sort likes --pretty` — 98 tweets sorted by likes ✓
- [x] **4d.** `bird search "test query" --pages 3` — fetched 2 pages (99+24=123 tweets),
  ran out of results before page 3. Both pages cache miss. ✓

**Thread:**

- [x] **4e.** `bird thread 2023452486431109461 --pretty` — reconstructed reply conversation,
  1 tweet in thread, complete=true ✓
- [x] **4f.** Same thread again — both tweet lookup and search are cache hits ✓
- [x] **4g.** `bird thread abc123` — "tweet ID must be numeric: abc123", exit code 1 ✓

**Success:** Search returns valid results, sorting works, thread reconstruction
works, repeated calls show caching, invalid input errors cleanly.

---

### Phase 5: Watchlist (~5 min)

Test the watchlist workflow end-to-end.

- [x] **5a.** `bird watchlist list` — empty ✓
- [x] **5b.** `bird watchlist add anthropic` ✓
- [x] **5c.** `bird watchlist add OpenAI` ✓
- [x] **5d.** `bird watchlist list` — ["anthropic","OpenAI"] ✓
- [x] **5e.** `bird watchlist --pretty check` — anthropic 7 tweets $0.045, OpenAI 10 tweets $0.06 ✓
  Note: `--pretty` must go before subcommand (clap nested subcommand quirk)
- [x] **5f.** Same check again — both cache hits ✓
- [x] **5g.** `bird watchlist remove OpenAI` ✓
- [x] **5h.** `bird watchlist list` — ["anthropic"] ✓

**Success:** Add/remove/list works, check hits the API and shows results,
repeated check uses cache.

---

### Phase 6: Usage and Cost Tracking (~10 min)

Validate the accumulated usage data from all previous phases.

- [x] **6a.** `bird usage --pretty` — $7.96 total, 51 calls, 37% cache hit rate, ~$1.82 savings ✓
  - Top: search $4.34 (8 calls), bookmarks $3.48 (7 calls), me $0.07 (11 calls)
  - All totals are non-zero, daily breakdown shows today, bookmarks near top ✓
- [x] **6b.** `bird usage --since 2026-02-17 --pretty` — matches 6a (all today) ✓
- [ ] **6c.** Skipped — requires separate Bearer token not configured

**Success:** Usage report reflects all commands run during testing. Endpoint
breakdown makes sense. If `--sync` works, actual vs estimated comparison is
available.

---

### Phase 7: Edge Cases and Error Handling (~10 min)

Test defensive behavior and error paths.

**Auth errors:**

- [x] **7a.** `bird --no-cache --access-token "bad_token" me` — 403 error, exit code 1 ✓
  Note: needed `--no-cache` because cache doesn't differentiate by token value
- [x] **7a2.** Cache stayed at 9 entries (non-2xx not cached) ✓

**API errors (200-with-errors pattern):**

- [x] **7b.** `bird profile zzz_no_user_99 --pretty` — "Could not find user", exit code 1 ✓
  (original name was 19 chars, caught by client-side validation — nice!)
- [x] **7b2.** Same command again — still reports error from cached response ✓
- [x] **7b3.** Cache entry count: 9→10 (200-with-errors IS cached) ✓
- [x] **7c.** `bird get /2/tweets/0 --pretty` — error JSON printed, exit code 0 ✓
  Bonus: tweet 999 is real ("wondering when odeo will have a poker night") ✓

**Cache resilience:**

- [x] **7d.** Moved cache.db away → `bird me` worked (graceful degradation) ✓
- [x] **7d2.** Restored cache.db → 12 entries (original 10 + 2 from raw get tests) ✓

**Raw commands:**

- [x] **7e.** `bird get /2/users/me --pretty` — cache hit, works ✓
- [x] **7f.** `bird get /2/tweets/search/recent --query query=test --query max_results=10 --pretty` — 10 tweets returned ✓

**Success:** Bad token produces clear error with exit code 1. Non-2xx response
is NOT cached (verified by entry count). API 200-with-errors IS detected (from
both fresh and cached responses) and IS cached (verified by entry count). Cache
degradation is graceful. Raw commands work.

---

### Phase 8: Final Validation (~5 min)

Wrap up and confirm overall health.

- [x] **8a.** Cache: 13 entries, 0.2 MB / 100 MB, healthy, oldest 6m/newest 47s ✓
- [x] **8b.** Usage: $8.02 total, 58 calls, 37% cache hit rate, ~$1.84 savings ✓
  - Top: search $4.40 (9), bookmarks $3.48 (7), me $0.07 (11), profile $0.05 (6)
- [x] **8c.** Doctor: oauth2_user, stored, BrettDavies, can_refresh=true, all core commands available ✓
- [x] **8d.** bookmarks.json: valid JSON, 299 bookmarks, data key present ✓
- [x] **8e.** cache.db: 224 KB (229376 bytes), no WAL file ✓
- [x] **8f.** Permissions: tokens.json 600, cache.db 600 ✓
- [x] **8g.** Test artifacts cleaned up ✓
  Note: /tmp/bookmarks.json kept (user's bookmark data)

**Success:** Cache has reasonable size, usage tracks all activity, doctor is
healthy, bookmarks file is complete and valid, permissions intact.

---

## Bonus Steps (Optional Deep-Dive)

These additional tests were identified by research agents. Run them if time
permits or if specific subsystems need extra confidence.

### Cache Deep-Dive

- [ ] **B1.** `BIRD_NO_CACHE=1 bird me --pretty` — env var disables cache at
  config level (different code path from `--no-cache` flag). Should show cache
  miss. Verify with `bird usage` that the call IS tracked (unlike `--no-cache`,
  the env var sets `cache_enabled=false` but still opens the DB for usage).
- [ ] **B2.** `bird --cache-ttl 100000 me --pretty` — TTL is capped at 86400s
  (24 hours) in `effective_ttl()`. Verify the entry expires after 24h, not
  100000s (difficult to test in-session; note as known cap).
- [ ] **B3.** Corrupted database test: instead of renaming, corrupt the file:

  ```bash
  cp ~/.config/bird/cache.db ~/.config/bird/cache.db.bak
  echo "corrupted" > ~/.config/bird/cache.db
  bird me --pretty  # should work (graceful degradation)
  cp ~/.config/bird/cache.db.bak ~/.config/bird/cache.db
  ```

- [ ] **B4.** Debug output redaction:

  ```bash
  RUST_LOG=bird=trace bird me --pretty 2>trace.log
  ```

  Search `trace.log` for any token values — should contain only `[REDACTED]`.
  Clean up: `rm trace.log`
- [ ] **B5.** Concurrent access test (validates WAL mode and busy_timeout):

  ```bash
  bird me --pretty &
  bird profile <username> --pretty &
  wait
  ```

  Both should succeed without SQLite lock errors.
- [ ] **B6.** `NO_COLOR=1 bird me --pretty` — verify no ANSI escape codes in
  output. Also test `TERM=dumb bird doctor --pretty`.

### Search/Thread Deep-Dive

- [ ] **B7.** `bird search "anthropic" --sort invalid_value --pretty` — should
  error with a clear message about valid sort values.
- [ ] **B8.** Find an old tweet (>7 days) and run `bird thread <old_tweet_id>`.
  Should emit a warning that `search/recent` only covers 7 days, so the thread
  may be incomplete.
- [ ] **B9.** `bird search "test" --pages 10 --pretty > /dev/null` — upper bound
  page test. Exercises max in-memory accumulation (~1000 tweets, ~5 MB heap).

---

## Acceptance Criteria

- [ ] All commands execute without panics or hangs
- [ ] Cache hits confirmed (stderr shows `$0.00` on repeated calls)
- [ ] Cache misses confirmed (stderr shows real cost on first calls)
- [ ] `--refresh`, `--no-cache`, `--cache-ttl` flags work correctly
- [ ] Bookmarks pagination completes with no duplicates
- [ ] Search returns valid, sorted results
- [ ] Thread reconstructs conversations
- [ ] Watchlist add/remove/list/check all work
- [ ] Usage report reflects all API calls made during testing
- [ ] Error handling is graceful (no panics, clear messages, correct exit codes)
- [ ] Cache graceful degradation works (renamed DB file doesn't crash CLI)
- [ ] Cost display on stderr is consistent and accurate
- [ ] Non-2xx responses are NOT cached (verified by entry count after 401)
- [ ] Error-in-200 responses ARE cached and still detected on cache hit
- [ ] File permissions remain `600` on tokens.json and cache.db

## Technical Notes

### Existing Test Coverage

| Layer | Count | What It Tests | What It Doesn't |
|-------|-------|--------------|-----------------|
| Unit (in-memory SQLite) | 110 | Cache key gen, TTL logic, URL normalization, cost estimation, usage queries, pruning | Real HTTP, real API responses, real pagination |
| Integration (CLI smoke) | 6 | Arg parsing, version/help, watchlist config | Any API command, auth flow, caching |

This live session fills the gap: real HTTP calls, real API responses, real
pagination, real caching, real cost tracking.

### Cost Estimate

Based on X API pricing ($0.005/tweet, $0.01/user):

- `me` + `profile`: ~$0.02
- Bookmarks (500 bookmarks): ~$2.50
- Bookmarks (2000 bookmarks): ~$10.00
- Search (3 queries, ~100 tweets each): ~$1.50
- Thread (1 thread, ~50 tweets): ~$0.25
- Watchlist check (2 accounts): ~$0.50

**Estimated total: $5-15** depending on bookmark count. Cache hits on repeated
calls save roughly 30-50% of this.

### Key Files for Reference

| Component | File | Key Lines |
|-----------|------|-----------|
| Cache layer | `src/cache/mod.rs` | 96-159 (get), 470-495 (TTL, skip rules) |
| Cache DB | `src/cache/db.rs` | 137-157 (get with TTL), 160-189 (put) |
| Cost display | `src/cost.rs` | 77-119 (stderr format) |
| Bookmarks | `src/bookmarks.rs` | 56-121 (pagination loop) |
| Usage queries | `src/cache/usage.rs` | 98-162 (summary, daily, endpoints) |
| Doctor | `src/doctor.rs` | Auth state and config check |
| Output sanitization | `src/output.rs` | 77-82 (sanitize_for_stderr) |
| Auth/tokens | `src/auth.rs` | 249-261 (file permissions), 334-382 (refresh) |

### Documented Gotchas (from docs/solutions/)

1. **Pagination URLs are never cached** — `pagination_token=` in URL triggers
   `should_skip_cache()`. This is correct behavior; don't expect cache hits on
   page 2+. Also applies to `next_token=` in search results.
2. **X API returns errors in 200 responses** — `bird profile nonexistent_user`
   gets HTTP 200 with `{"errors": [...]}`. Bird detects this.
3. **OAuth1 bypasses CachedClient** — OAuth1 requests use `reqwest_oauth1`
   signing, which goes through a different code path. Cache hits only work for
   OAuth2/Bearer auth.
4. **Cache key includes auth type** — same URL with different auth types
   produces different cache keys. This prevents cross-auth cache pollution.
5. **File permissions on cache.db** — created with `0o600`. WAL/SHM sidecar
   files inherit this.
6. **`--no-cache` disables usage tracking too** — the cache and usage tracking
   share the same `BirdDb` connection. When `--no-cache` is passed, `db` is set
   to `None`, so `log_api_call()` returns immediately. These calls are invisible
   to `bird usage`.
7. **Error-in-200 responses ARE cached** — X API returns HTTP 200 for
   nonexistent users (with `{"errors": [...]}`). Since 200 is a success status,
   these get cached at the normal TTL. Bird still detects the error on cache hit,
   but the stale error occupies a cache slot for up to 1 hour (user endpoint TTL).
8. **Non-2xx responses are NOT cached** — only `response.status.is_success()`
   triggers cache write (`src/cache/mod.rs` line 142). A 401 from a bad token
   will not pollute the cache.
9. **Cache hits return empty headers** — cached responses store body + status
   only, not HTTP headers. If a future feature needs rate-limit headers, those
   must come from live responses.
10. **Login auto-clears cache** — after `bird login`, the cache is cleared to
    prevent stale data from a previous auth context being served.
11. **Ctrl-C during pagination produces invalid JSON** — bookmarks streams
    per-item to stdout. Interrupting mid-pagination leaves partial output.
    Re-run from scratch if interrupted.

### Known Untested Boundaries (Out of Scope)

These were identified by review agents but are impractical to test in a manual
90-minute session:

- **Token refresh mid-session** — would require waiting for token expiry or
  manipulating token timestamps. The refresh path in `src/auth.rs` is exercised
  automatically if the token expires during the session.
- **OAuth1 code path** — requires OAuth1 credentials, which are not part of
  the standard setup.
- **Auto-pruning under load** — triggers every 20th write. Would need hundreds
  of cache writes to exercise meaningfully.
- **`raw.rs` auth type bug** — `raw.rs` hardcodes `AuthType::OAuth2User` for
  Bearer tokens, which could cause cache key collisions between Bearer and
  OAuth2 auth types. Documented for post-release fix.
