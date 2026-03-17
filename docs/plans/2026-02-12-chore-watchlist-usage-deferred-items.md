---
title: "chore: Watchlist & Usage Deferred Items"
type: chore
status: completed
date: 2026-02-12
revised: 2026-02-15
depends_on: "2026-02-11-feat-watchlist-usage-commands-plan"
review_status: "revised after multi-agent review"
---

# chore: Watchlist & Usage Deferred Items

Items deferred from the watchlist/usage implementation. Revised after multi-agent
review incorporating findings from performance, architecture, pattern recognition,
security, spec flow, and best practices analysis.

## Design Decisions (from review)

**D1: Full API coverage.** All API calls are charged. All call paths must be logged
— `CachedClient::get()` (all 3 exit paths), `CachedClient::request()`,
OAuth1 paths, and direct `http_get()` calls. Auth infrastructure calls (token exchange, refresh, initial user lookup during login in `auth.rs`) are excluded — they are not billable data operations and occur outside the `CachedClient` lifecycle.

**D2: Parse JSON inside the wrapper for logging.** Accept a double-parse. Extract a
shared helper so both the logging layer and command handlers use the same
estimation code. DRY over micro-optimization.

**D3: Record raw cost data, derive reports later.** Always compute `estimated_cost`
as if the request were fresh (pass `cache_hit: false` to the estimator). Store the
actual `cache_hit` boolean separately. Reporting queries derive savings from the
raw data: `SUM(estimated_cost) WHERE cache_hit = 1`.

**D4: Normalize endpoints at write time.** Store `/2/tweets/:id` not
`/2/tweets/1234567890`. This keeps the `top_endpoints` view meaningful and indexes
efficient.

**D5: Graceful degradation everywhere.** Usage logging failures are never fatal.
`--sync` errors show local data with a warning. Missing DB shows an informational
message, not an error.

---

## Item 1: Wire Usage Logging into All API Call Paths -- Implemented

**Priority:** High — without this, `bird usage` shows no data.

`BirdDb::log_usage()` and `UsageLogEntry` are implemented but not called. Wire
logging into every path that makes an API call or returns cached data.

### 1a. Add `normalize_endpoint()` function -- Implemented

**File:** `src/cache.rs` (new function, near `default_ttl_for_endpoint`)

Segment-based normalization that:

- Extracts URL path via `url::Url::parse(url).map(|u| u.path().to_string())`
  (same pattern as `default_ttl_for_endpoint` at line 817)
- Replaces numeric ID segments with `:id`
- Handles `/2/users/by/username/{name}` → `/2/users/by/username/:username`
- Uses a known-literal allowlist (`tweets`, `users`, `search`, `recent`,
  `bookmarks`, `me`, etc.) — no regex

Test cases:

- `/2/tweets/search/recent` → unchanged
- `/2/users/me` → unchanged
- `/2/tweets/1234567890` → `/2/tweets/:id`
- `/2/users/123/bookmarks` → `/2/users/:id/bookmarks`
- `/2/users/by/username/jack` → `/2/users/by/username/:username`
- Full URL with query params → path only

### 1b. Add `estimate_raw_cost()` wrapper *(superseded by `log_api_call` — see 1b2)* -- Implemented

**File:** `src/cost.rs` (new function)

```rust
/// Estimate cost assuming a fresh (non-cached) request.
/// Eliminates the "magic false" in D3 by giving the intent a name.
pub fn estimate_raw_cost(body: &serde_json::Value, endpoint: &str) -> CostEstimate {
    estimate_cost(body, endpoint, false)
}
```

This wrapper exists to eliminate the bare `false` literal that D3 requires at
every call site. Callers that need raw cost data use `estimate_raw_cost()`
instead of `estimate_cost(&json, endpoint, false)`.

**Note:** Direct callers of `estimate_raw_cost()` are not expected — the
`log_api_call` method (Item 1b2) calls it internally. This wrapper remains
public for any future call site that needs raw cost estimation without the
full logging side-effect.

### 1b2. Add `log_api_call()` method on `CachedClient` -- Implemented

**File:** `src/cache.rs` (new method on `CachedClient`)

This is the single instrumentation point for all API call logging. It
encapsulates: JSON parsing, `normalize_endpoint`, `estimate_raw_cost` (with
`cache_hit: false` per D3), `object_type` derivation, and `log_usage` — all
with non-fatal error handling.

```rust
/// Log an API call to the usage database. Non-fatal: errors are warned to stderr.
/// Handles JSON parsing, endpoint normalization, cost estimation, and DB insert.
pub fn log_api_call(&mut self, url: &str, method: &str, body: &str, cache_hit: bool, username: Option<&str>) {
    let Some(ref mut db) = self.db else { return };
    let endpoint = normalize_endpoint(url);
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let estimate = cost::estimate_raw_cost(&json, &endpoint);
    let object_type = if estimate.users_read > 0 && estimate.tweets_read == 0 {
        Some("user")
    } else {
        Some("tweet")
    };
    match db.log_usage(&UsageLogEntry {
        endpoint: &endpoint,
        method,
        object_type,
        object_count: (estimate.tweets_read + estimate.users_read) as i64,
        estimated_cost: estimate.estimated_usd,
        cache_hit,
        username,
    }) {
        Err(e) => eprintln!("[usage] warning: failed to log API call: {e}"),
        _ => {}
    }
}
```

- Used internally by `CachedClient::get()` (Item 1c) and
  `CachedClient::request()` (Item 1d), reducing the 3-path logging in
  `get()` to one-liners
- Used at each OAuth1 call site (Item 1e), reducing ~8 lines per site to:
  `client.log_api_call(&url, "GET", &text, false, config.username.as_deref());`
- Used by direct `http_get()` calls (Item 1f)
- Depends on Item 1a (`normalize_endpoint`)

### 1c. Log usage in all 3 paths of `CachedClient::get()` -- Implemented

**File:** `src/cache.rs`, inside `CachedClient::get()`

The method has 3 exit paths that must all log:

| Path | Lines | Current behavior | Change |
|------|-------|-----------------|--------|
| Skip-cache (pagination, `--no-cache`) | 608-609 | `http_get()` + return | Add logging after `http_get()` |
| Cache hit | 619-626 | Return cached body | Add logging with `cache_hit: true` |
| Cache miss | 637-654 | `http_get()` + cache write | Add logging after HTTP response |

For each path, call `self.log_api_call()` (Item 1b2) — a one-liner:

```rust
// Skip-cache path:
self.log_api_call(url, "GET", &response.body, false, ctx.username);

// Cache hit path:
self.log_api_call(url, "GET", &cached_body, true, ctx.username);

// Cache miss path:
self.log_api_call(url, "GET", &response.body, false, ctx.username);
```

For cache hits, the cost is computed from the cached body (reflecting what the
data would have cost). The `cache_hit: true` column records the truth.

The `username` field comes from `CacheContext.username` (already available as a
parameter).

All JSON parsing, endpoint normalization, cost estimation, `object_type`
derivation, and non-fatal error handling are encapsulated in `log_api_call`.

### 1d. Log usage in `CachedClient::request()` (POST/PUT/DELETE) -- Implemented

**File:** `src/cache.rs`, inside `CachedClient::request()`

Currently a pass-through with no caching (line 658-679). Add a one-liner
`log_api_call` (Item 1b2) after the HTTP response:

```rust
self.log_api_call(url, method, &response.body, false, ctx.username);
```

POST/PUT/DELETE responses may not have a `data` array — cost estimation will
return 0 objects / $0.00, which is correct (the X API charges for reads, not
writes). The log entry still provides visibility into mutation frequency.

`CacheContext` is not currently passed to `request()`. Either:

- Add `ctx: &CacheContext<'_>` parameter (preferred — matches `get()` signature)
- Or pass `username: Option<&str>` directly

### 1e. Log usage for OAuth1 paths -- Implemented

**Files:** `src/raw.rs`, `src/search.rs`, `src/thread.rs`, `src/profile.rs`,
`src/watchlist.rs`

OAuth1 requests bypass `CachedClient` entirely — they call
`client.http().clone().oauth1(secrets).get(&url).send().await`. These must be
logged at the call site.

Each OAuth1 call site uses the `log_api_call` method (Item 1b2) — a one-liner:

```rust
// After receiving the response body as `text`:
client.log_api_call(&url, "GET", &text, false, config.username.as_deref());
```

There are ~6 OAuth1 call sites across `raw.rs`, `search.rs`, `thread.rs`,
`profile.rs`, and `watchlist.rs`. Each becomes a single line instead of the
~8-line inline logging block.

### 1f. Log usage for direct `http_get()` calls -- Implemented

**File:** `src/usage.rs` (the `sync_actual_usage` function)

`sync_actual_usage()` calls `client.http_get()` directly for
`GET /2/usage/tweets`. Add a one-liner `log_api_call` (Item 1b2) after the
response:

```rust
client.log_api_call(&url, "GET", &response.body, false, username);
```

This is a single call site.

### 1g. Fix `maybe_prune_usage()` to use `write_count` -- Implemented

**File:** `src/cache.rs`, lines 312-322

Replace the `SELECT COUNT(*)` full table scan with the `write_count` modulo
pattern already proven by cache pruning at line 186-189:

```rust
// In log_usage(), after the INSERT:
self.write_count += 1;
if self.write_count.is_multiple_of(50) {
    self.prune_old_usage(now)?;
}
```

Delete `maybe_prune_usage()` and replace with:

```rust
fn prune_old_usage(&self, now_ts: i64) -> Result<(), rusqlite::Error> {
    let cutoff = now_ts - (90 * 24 * 60 * 60);
    self.conn.execute("DELETE FROM usage WHERE timestamp < ?1", [cutoff])?;
    Ok(())
}
```

The `write_count` is shared between `put()` (cache writes) and `log_usage()`
(usage writes). Both are write operations; the counter just needs to trigger
periodic cleanup.

### 1h. Use `prepare_cached` for `log_usage` INSERT -- Implemented

**File:** `src/cache.rs`, line 293

Change `self.conn.execute(...)` to `self.conn.prepare_cached(...)?.execute(...)`.
The INSERT runs on every API call, so statement caching avoids repeated SQLite
query planning during pagination loops.

### 1i. Remove `#[allow(dead_code)]` annotations -- Implemented

After wiring is complete, remove these annotations:

- Line 282: `log_usage()`
- Line 443: `UsageLogEntry`
- Line 707: `db_mut()`
- Line 547: `ApiResponse::headers` (if consumed by Item 4's rate limit parsing)

---

## Item 2: Fix Bugs Found During Review -- Implemented

### 2a. Fix `days_back` validation in `usage.rs` -- Implemented

**File:** `src/usage.rs`, line 70-71

The current code uses YYYYMMDD integer subtraction:

```rust
let days_back = now_ymd - since_ymd;
if days_back > 90_00_00 { ... }
```

`20260213 - 20251101 = 9112`, not 104 days. The threshold 900000 would only
trigger for ~90-year differences. The validation is non-functional.

Fix: use `chrono` to compute actual day difference:

```rust
let now = chrono::Utc::now().date_naive();
let since = chrono::NaiveDate::parse_from_str(&format!(
    "{}-{:02}-{:02}",
    since_ymd / 10000,
    (since_ymd % 10000) / 100,
    since_ymd % 100
), "%Y-%m-%d")?;
let days_back = (now - since).num_days();
if days_back > 90 {
    eprintln!("[usage] warning: X API only returns 90 days of history; --since may exceed that range");
}
```

### 2b. Fix `estimated_savings` always being $0.00 -- Implemented

**Root cause:** `estimate_cost()` returns `estimated_usd: 0.0` for cache hits
(cost.rs:17-23). The SQL `SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ...)`
sums zeros.

**Fix:** Per decision D3, `log_usage()` always records the real cost of the data
(computed with `cache_hit: false`). The `cache_hit` column stores the truth
separately. Then the existing SQL for `estimated_savings` works correctly:

```sql
SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ELSE 0 END)
```

— because `estimated_cost` now reflects what the request would have cost.

No SQL changes needed. The fix is entirely in how `log_usage()` is called (Item
1c: always pass `cache_hit: false` to the cost estimator).

### 2c. Fix `query_usage_summary` to use `prepare_cached` -- Implemented

**File:** `src/cache.rs`, line 326

`query_usage_summary()` uses `self.conn.query_row()` directly, while
`query_daily_usage()` (line 347) and `query_top_endpoints()` (line 374) correctly
use `prepare_cached`. Fix for consistency with the established pattern.

---

## Item 3: Graceful Handling of Missing/Unavailable Cache DB -- Implemented

**Priority:** Medium — affects first-run UX and `--no-cache` users.

**File:** `src/usage.rs`, lines 54-56 and 229-231

### Three distinct failure modes, three distinct messages

| Situation | Detection | Message | Exit code |
|-----------|-----------|---------|-----------|
| `--no-cache` flag active | `client.cache_disabled()` | "Usage tracking requires the cache. Remove `--no-cache` to enable usage tracking." | 0 |
| DB failed to open | `client.db().is_none()` and not `cache_disabled()` | "Cache database is unavailable. Run `bird cache clear` to reset, or check file permissions." | 0 |
| DB exists, zero rows | `summary.total_calls == 0` | "No usage data recorded yet. Run some API commands first." | 0 |

All three cases exit 0 — "no data" is an informational state, not a command
failure. Output an empty report to stdout (for machine consumers) and the
informational message to stderr.

### Implementation

Add a `pub fn cache_disabled(&self) -> bool` accessor to `CachedClient` that
returns `self.cache_opts.no_cache`.

Replace the `ok_or(...)` pattern with a match:

```rust
let db = match client.db() {
    Some(db) => db,
    None => {
        let msg = if client.cache_disabled() {
            "Usage tracking requires the cache. Remove --no-cache to enable."
        } else {
            "Cache database is unavailable. Run `bird cache clear` to reset."
        };
        eprintln!("[usage] {}", msg);
        // Output empty report for machine consumers
        if !pretty { println!("{}", serde_json::to_string(&empty_report(since_ymd))?); }
        return Ok(());
    }
};
```

Apply the same pattern at line 229-231 in `sync_actual_usage()`.

---

## Item 4: Graceful Degradation for `--sync` Errors -- Implemented

**Priority:** Medium — without this, any transient API error kills the entire
`bird usage --sync` command with zero output.

### Current problem

At `usage.rs:78`:

```rust
Some(sync_actual_usage(client, &token).await?)
```

The `?` propagates any error from `sync_actual_usage()` — including 429, 401,
403, 500, and network timeouts. The entire command aborts; the user sees no local
data.

### Design: show local data, warn about sync failure

Change `sync_actual_usage()` to catch non-2xx responses and return gracefully
instead of erroring. The local report always renders. Sync is best-effort
enrichment.

| Error | Warning to stderr | `actuals` in report |
|-------|------------------|-------------------|
| 429 Too Many Requests | "Rate limited. Resets at HH:MM UTC. Showing local data only." | `null` |
| 401 Unauthorized | "Auth token expired or invalid. Run `bird login` to refresh. Showing local data only." | `null` |
| 403 Forbidden | "Insufficient permissions for usage API. Showing local data only." | `null` |
| 500/502/503 | "X API error ({status}). Showing local data only." | `null` |
| Network timeout | "Request timed out. Showing local data only." | `null` |

When actuals are unavailable, the JSON report includes:

```json
{
  "actuals": null,
  "sync_warning": "Rate limited. Resets at 14:30 UTC. Showing local data only."
}
```

### Rate limit header parsing with validation

**Security requirement** (from security review): validate `x-rate-limit-reset`
before using it for display.

```rust
fn parse_rate_limit_reset(headers: &reqwest::header::HeaderMap) -> Option<i64> {
    let ts: i64 = headers.get("x-rate-limit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())?;
    let now = unix_now();
    // Reject timestamps in the past or more than 1 hour in the future
    if ts < now || ts > now + 3600 {
        return None;
    }
    Some(ts)
}
```

Format the reset time for display:

```rust
chrono::DateTime::from_timestamp(ts, 0)
    .map(|dt| dt.format("%H:%M UTC").to_string())
    .unwrap_or_else(|| "shortly".to_string())
```

### Implementation in `sync_actual_usage()`

Replace the blanket error return at line 215-221 with status-specific handling:

```rust
if response.status == reqwest::StatusCode::TOO_MANY_REQUESTS {
    let reset_msg = parse_rate_limit_reset(&response.headers)
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .map(|dt| format!("Resets at {}.", dt.format("%H:%M UTC")))
        .unwrap_or_default();
    eprintln!("[usage] Rate limited (429). {} Showing local data only.", reset_msg);
    return Ok(None);
}
if !response.status.is_success() {
    eprintln!(
        "[usage] Sync failed ({}: {}). Showing local data only.",
        response.status, response.body.chars().take(100).collect::<String>()
    );
    return Ok(None);
}
```

Change return type from `Result<Vec<ActualUsageDay>, ...>` to
`Result<Option<Vec<ActualUsageDay>>, ...>`. The caller in `run_usage()` unwraps
with `actuals.flatten()`.

### 4b. Sanitize API error bodies before stderr display -- Implemented

**Security finding:** The `eprintln!` calls above that include `response.body`
content display truncated API error bodies to stderr without sanitizing control
characters. A malicious or compromised API response containing ANSI escape
sequences (e.g., `\x1b[...`) could manipulate the user's terminal — rewriting
displayed text, changing colors, or triggering terminal-specific exploits.

**File:** `src/output.rs` (alongside existing `hyperlink()` which already
sanitizes ESC/BEL)

```rust
/// Sanitize untrusted text for stderr display: replace control chars with '?', truncate.
pub fn sanitize_for_stderr(s: &str, max_chars: usize) -> String {
    s.chars()
        .take(max_chars)
        .map(|c| if c.is_control() && c != '\n' { '?' } else { c })
        .collect()
}
```

Apply this to all `eprintln!` calls in Item 4 that include API response body
text. For example, the non-2xx error handler becomes:

```rust
eprintln!(
    "[usage] Sync failed ({}: {}). Showing local data only.",
    response.status, sanitize_for_stderr(&response.body, 100)
);
```

**Follow-up (out of scope for this plan):** Existing error paths in `raw.rs`,
`search.rs`, `thread.rs`, `profile.rs`, and `bookmarks.rs` that print API
response bodies to stderr have the same vulnerability. These should be audited
and updated to use `sanitize_for_stderr()` in a separate chore.

---

## Item 5: Integration Tests

**Priority:** Medium — establishes testing infrastructure for the project.

### Testing strategy: two layers

Following Rust CLI best practices, use a two-layer approach:

**Layer 1: Binary smoke tests with `assert_cmd`** (`tests/cli_smoke.rs`)

- Verify exit codes and basic output for each command
- No network, no mocking — tests against the compiled binary
- Fast, reliable, catches argument parsing regressions

**Layer 2: HTTP integration tests with `wiremock`** (`tests/api_integration.rs`)

- Stand up a local mock HTTP server per test
- Test full request/response flows including rate limits, pagination, errors
- Requires making the API base URL configurable

### New dev-dependencies

```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
wiremock = "0.6"
```

### 5a. Binary smoke tests (`tests/cli_smoke.rs`) -- Implemented

Tests that exercise the compiled binary via `assert_cmd`:

- `bird --version` → exit 0, stdout contains version
- `bird --help` → exit 0, stdout contains "Usage:"
- `bird` (no args) → exit 2, stderr contains usage info
- `bird me` with no config → exit 78 (EX_CONFIG) or 77 (EX_NOPERM)
- `bird usage` with no cache → exit 0 with informational message (after Item 3)
- `bird watchlist list` with empty config → exit 0, empty list
- `bird watchlist add alice` + `bird watchlist list` → list contains "alice"
- `bird watchlist remove alice` + `bird watchlist list` → list is empty

These tests use `env` overrides to point config/cache dirs at temp directories.

### 5b. HTTP integration tests (`tests/api_integration.rs`) -- DEFERRED

**DEFERRED (YAGNI):** Wiremock integration tests are over-engineering for a personal CLI. Unit tests with `in_memory_db()` cover the same logic paths. Revisit if unit tests prove insufficient.

Requires a configurable API base URL. Add `api_base_url: String` to
`ResolvedConfig` (default: `"https://api.x.com"`), and use it in all URL
construction.

Tests with `wiremock`:

**Watchlist check lifecycle:**

- Mock `GET /2/tweets/search/recent` → 200 with tweet data
- Run `execute_check()` against mock server
- Verify NDJSON output shape and content
- Verify usage is logged (check BirdDb after execution)

**Rate limit handling:**

- Mock endpoint → 429 with `x-rate-limit-reset` header
- Verify warning message is printed
- Verify local data is still shown (for `--sync` case)

**Pagination logging:**

- Mock endpoint → page 1 with `next_token`, page 2 without
- Verify both pages are logged to usage table

**Cache hit vs miss logging:**

- Make same request twice (first: cache miss, second: cache hit)
- Verify both are logged with correct `cache_hit` values
- Verify `estimated_cost` is non-zero for both (raw data recording)

### 5c. Usage query round-trip tests (in `src/cache.rs` unit tests) -- Implemented

Extend existing `#[cfg(test)] mod tests` with `in_memory_db()`:

- Insert varied usage entries via `log_usage()` (different endpoints, dates,
  cache hit/miss, object counts)
- Call `query_usage_summary()` → verify totals, cache hit rate, savings math
- Call `query_daily_usage()` → verify per-day grouping
- Call `query_top_endpoints()` → verify endpoint aggregation with normalized paths
- Test empty table → returns zero-value summary (not an error)
- Test 90-day pruning boundary → entries at day 91 are pruned

### 5d. Testability refactors -- DEFERRED

**DEFERRED:** Not needed without 5b.

**Refactor `run_watchlist_check` for stdout capture:**
Change `run_watchlist_check` to accept `impl Write` instead of writing to
`stdout()` directly. This allows integration tests to capture output without
process-level redirection.

**Add test config helper:**

```rust
#[cfg(test)]
fn assert_not_real_config(path: &Path) {
    let config_dir = dirs::config_dir().unwrap().join("bird");
    assert!(!path.starts_with(&config_dir),
        "test must not use real config directory: {:?}", path);
}
```

---

## Implementation Order

```
[x] Item 1a:  normalize_endpoint()           ← foundation, no dependencies
[x] Item 1b:  estimate_raw_cost() wrapper    ← foundation, no dependencies
[x] Item 1b2: log_api_call() on CachedClient ← depends on 1a, 1b
[x] Item 2a:  fix days_back validation       ← standalone bug fix
[x] Item 2c:  prepare_cached for summary     ← standalone consistency fix
[x] Item 1g:  fix prune to use write_count   ← standalone improvement
[x] Item 1h:  prepare_cached for log_usage   ← standalone improvement
[x] Item 1c:  log in CachedClient::get()     ← depends on 1b2
[x] Item 1d:  log in CachedClient::request() ← depends on 1b2
[x] Item 1e:  log in OAuth1 paths            ← depends on 1b2
[x] Item 1f:  log in http_get() call         ← depends on 1b2
[x] Item 1i:  remove #[allow(dead_code)]     ← depends on 1c-1f
[x] Item 2b:  estimated_savings fix          ← automatic via 1c (D3)
[x] Item 3:   graceful missing cache.db      ← independent
[x] Item 4:   graceful --sync degradation    ← independent (includes 4b sanitization)
[x] Item 5a:  binary smoke tests             ← independent
[ ] Item 5b:  HTTP integration tests         ← DEFERRED (YAGNI)
[x] Item 5c:  usage query round-trip tests   ← depends on 1g, 1h
[ ] Item 5d:  testability refactors          ← DEFERRED (not needed without 5b)
```

Parallelizable groups:

- **Group A** (foundations): 1a, 1b, 1b2, 2a, 2c, 1g, 1h — 1b2 depends on 1a+1b; rest independent
- **Group B** (wiring): 1c, 1d, 1e, 1f — depend on 1b2 (Group A), independent of each other
- **Group C** (UX): 3, 4 — independent of everything else
- **Group D** (tests): 5a (independent), 5d → 5b (depends on Group B), 5c

---

## Review Findings Incorporated

| Source | Finding | Disposition |
|--------|---------|-------------|
| Architecture | 3 exit paths in `get()`, not 2 | Addressed in 1c |
| Architecture | `estimated_savings` logic bug | Addressed in 2b via D3 |
| Architecture | `request()` needs `CacheContext` | Addressed in 1d |
| Performance | Use `write_count` not `COUNT(*)` | Addressed in 1g |
| Performance | Use `prepare_cached` for INSERT | Addressed in 1h |
| Pattern Recognition | Don't reuse `normalize_url()` for endpoint grouping | Addressed in 1a (separate function) |
| Pattern Recognition | Derive `object_type` from `CostEstimate` not endpoint sniffing | Addressed in 1c |
| Pattern Recognition | `ok_or()` violates graceful degradation pattern | Addressed in Item 3 |
| Security | Validate `x-rate-limit-reset` bounds | Addressed in Item 4 |
| Security | Strip query params before logging | Addressed in 1a |
| Spec Flow | `days_back` validation non-functional | Addressed in 2a |
| Spec Flow | OAuth1 paths unlogged | Addressed in 1e |
| Spec Flow | `--sync` error aborts entire command | Addressed in Item 4 |
| Best Practices | Two-layer test strategy | Addressed in Item 5 |
| Best Practices | `assert_cmd` + `wiremock` | Addressed in 5a, 5b |
| Learnings | Non-fatal logging pattern | Addressed in all logging items |
| Learnings | Single instrumentation point | Addressed via `log_api_call` in 1b2 |
| Security | Terminal escape injection in error output | Addressed in 4b |
