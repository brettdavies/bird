---
title: "refactor: Unify cache layer across auth types and fix auth resolution order"
type: refactor
status: completed
date: 2026-02-17
deepened: 2026-02-17
---

# refactor: Unify Cache Layer Across Auth Types and Fix Auth Resolution Order

## Enhancement Summary

**Deepened on:** 2026-02-17
**Agents used:** architecture-strategist, security-sentinel, performance-oracle, code-simplicity-reviewer, pattern-recognition-specialist, 3 learnings-researchers

### Key Improvements from Research

1. **SECURITY: Custom Debug impl required** — `CommandToken::Bearer` with `#[derive(Debug)]` leaks raw bearer tokens. Must add manual `Debug` impl that redacts `token` field (every other secret-carrying struct in the codebase already does this).
2. **CORRECTNESS: `RequestContext.auth_type` is `&AuthType`** — Plan code examples show `auth_type,` in destructuring but `RequestContext` expects `&AuthType`. Must use `auth_type: &auth_type` in match arms.
3. **CORRECTNESS: Doctor must be updated in Phase 3** — `doctor.rs:build_auth_state()` has the same inverted resolution order. After Phase 3, `bird doctor` would report wrong primary auth type. Include in scope.
4. **SIMPLICITY: Consider collapsing to 2 independent steps** — Phase 2 (OAuth1 cache) has no dependency on Phases 1/3 (auth fix). Can be separate PRs in any order.
5. **PREVENTION: `log_api_call()` must fire on every path** — Per Prevention Rule #3, cache-hit early returns in `oauth1_request()` must still call `log_api_call()` exactly once.
6. **PREVENTION: Credential extraction after cache check** — Defer `ok_or()` credential extraction until after cache hit check to avoid wasted validation on hits.
7. **NAMING: Use consistent helper names** — Rename `has_oauth1_credentials` to `has_oauth1_available` to match `has_oauth2_available`.

## Overview

Two related architectural issues in bird's cache and auth layers:

1. **OAuth1 requests completely bypass the cache** — `oauth1_request()` neither reads
   from nor writes to the SQLite cache. Every OAuth1 request is a fresh HTTP call
   regardless of TTL, `--refresh`, or `--no-cache` flags. The cache infrastructure
   already supports `AuthType::OAuth1` in key generation; it's just not wired up.

2. **Auth resolution order is backwards** — The current order (Bearer → OAuth1 →
   OAuth2User) prioritizes app-only auth over user-context auth. The correct order
   is OAuth2User → OAuth1 → Bearer, and the system should skip auth methods that
   aren't configured rather than trying them speculatively.

A third latent bug surfaced during analysis: `CommandToken::Bearer` doesn't
distinguish OAuth2User bearer from app-only bearer, causing incorrect cache keys
in every command file.

## Problem Statement

### OAuth1 cache bypass

`oauth1_request()` at `src/cache/mod.rs:275-333` makes HTTP requests directly via
`self.http.clone().oauth1(secrets).get(url)`. It has no cache read before the
request and no cache write after. Every command's `match &token` has two branches:

```rust
// Cached path (Bearer/OAuth2):
CommandToken::Bearer(access) => {
    let ctx = RequestContext { auth_type: &AuthType::OAuth2User, ... };
    client.get(&url, &ctx, headers).await?      // ← cache-aware
}
// Uncached path (OAuth1):
CommandToken::OAuth1 => {
    client.oauth1_request("GET", &url, config, None).await?  // ← bypasses cache
}
```

This is visible in `src/profile.rs:35-46`, `src/search.rs:55-66`, `src/thread.rs:225-236`,
`src/watchlist.rs:274-285`, and `src/raw.rs:41-79`.

### Auth resolution order

`resolve_token_for_command()` at `src/auth.rs:302-331` tries auth in this order:

1. **Bearer** (line 311) — cheapest to resolve, but app-only
2. **OAuth1** (line 316) — checks 4 credentials present
3. **OAuth2User** (line 325) — calls `ensure_access_token()` (disk I/O + potential refresh)

This means if a user has both a bearer token and OAuth2 stored tokens, bearer wins.
Bearer is app-only auth — it can't access user-specific endpoints like `/2/users/me`
and returns less contextual data. The user wants: **OAuth2User → OAuth1 → Bearer**.

### CommandToken::Bearer doesn't carry its auth origin

Every command hardcodes `auth_type: &AuthType::OAuth2User` in the `RequestContext`
regardless of whether the `CommandToken::Bearer` came from `resolve_bearer_token()`
(app-only) or `ensure_access_token()` (OAuth2 user). This means:

- Cache keys for app-only Bearer include `"oauth2_user"` — wrong
- If a user switches between OAuth2 and Bearer, the cache can serve stale responses
  from the wrong auth context

## Proposed Solution

### Phase 1: Fix `CommandToken` to carry actual auth type

**Minimal change:** Modify `CommandToken::Bearer` to include the auth type it
originated from, so commands can set the correct `RequestContext`.

```rust
// src/auth.rs
#[derive(Clone)]
pub enum CommandToken {
    Bearer { token: String, auth_type: AuthType },
    OAuth1,
}

// SECURITY: Custom Debug impl to redact bearer token (every other secret-carrying
// struct in the codebase already does this — see OAuth2Account, ResolvedConfig, ApiResponse)
impl std::fmt::Debug for CommandToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandToken::Bearer { auth_type, .. } => f
                .debug_struct("Bearer")
                .field("token", &"[REDACTED]")
                .field("auth_type", auth_type)
                .finish(),
            CommandToken::OAuth1 => write!(f, "OAuth1"),
        }
    }
}
```

Update `resolve_token_for_command()` to tag the token:

```rust
// Bearer path (app-only):
return Ok(CommandToken::Bearer {
    token: t,
    auth_type: ReqAuthType::Bearer,
});

// OAuth2User path:
return Ok(CommandToken::Bearer {
    token: t,
    auth_type: ReqAuthType::OAuth2User,
});
```

Update every command file to extract auth_type from the token instead of hardcoding:

```rust
// Before (every command):
CommandToken::Bearer(access) => {
    let ctx = RequestContext {
        auth_type: &AuthType::OAuth2User,  // ← hardcoded, wrong for Bearer
        ...
    };
}

// After (note: &auth_type because RequestContext.auth_type is &AuthType):
CommandToken::Bearer { token, auth_type } => {
    let ctx = RequestContext {
        auth_type: &auth_type,  // ← from actual resolution, reference needed
        ...
    };
}
```

**Files changed:** `src/auth.rs`, `src/profile.rs`, `src/search.rs`, `src/thread.rs`,
`src/watchlist.rs`, `src/raw.rs`, `src/bookmarks.rs`, `src/doctor.rs`

### Phase 2: Make `oauth1_request()` cache-aware (GET only)

Add cache read/write to `oauth1_request()` for GET requests. POST/PUT/DELETE remain
uncached (mutations must never be cached).

```rust
// src/cache/mod.rs — oauth1_request() revised flow for GET:
//
// 1. If method != "GET" or should_skip_cache(url) or no_cache: direct HTTP (unchanged)
// 2. Compute cache key with AuthType::OAuth1 + config.username
// 3. If !refresh, check cache → return on hit (log_api_call with cache_hit=true FIRST)
// 4. Extract OAuth1 credentials (ok_or pattern — AFTER cache check to skip on hits)
// 5. Sign and send OAuth1 request
// 6. If 2xx, write to cache
// 7. Log with actual cache_hit value (cache_hit=false)
// 8. Return response
//
// CRITICAL per Prevention Rule #3: log_api_call() must execute exactly ONCE
// on every code path — both cache-hit early returns AND cache-miss HTTP paths.
// CRITICAL per Prevention Rule #4: reqwest_oauth1::Secrets::new must only appear
// in cache/mod.rs. This is a one-file change because of prior OAuth1 centralization.
```

The `RequestContext` is constructed internally — `oauth1_request()` always knows
it's `AuthType::OAuth1` and gets username from `config.username`:

```rust
let ctx = RequestContext {
    auth_type: &AuthType::OAuth1,
    username: config.username.as_deref(),
};
```

Key design decisions:
- **Cache check happens BEFORE OAuth1 signing** — signing involves crypto operations
  (HMAC-SHA1 with nonce/timestamp). Cache hits skip signing entirely.
- **Credential extraction happens AFTER cache check** — The four `ok_or()` credential
  extractions at lines 284-299 move below the cache-hit early return. On cache hits,
  credentials are never validated (acceptable — they were validated on the original
  request that populated the cache).
- **`--refresh` and `--no-cache` flags are respected** — read from `self.cache_opts`
  just like `get()` does.
- **Only GET requests are cached** — gate on `method == "GET"` at the top.
- **Graceful degradation preserved** — Gate cache operations on `if let Some(db) = &self.db`
  matching the existing `get()` pattern. When `db = None`, proceed with direct HTTP unchanged.
- **Cache-hit responses must populate `ApiResponse.json`** — Per Prevention Rule #2
  (parse JSON exactly once), parse the cached body on retrieval, same as `get()` does.
- **Pagination URLs excluded** — `should_skip_cache()` already handles `pagination_token=`
  and `next_token=` parameters. This applies to OAuth1 too.

**Files changed:** `src/cache/mod.rs`

### Phase 3: Reorder auth resolution with smart routing

Change `resolve_token_for_command()` to try **OAuth2User → OAuth1 → Bearer**:

```rust
pub async fn resolve_token_for_command(...) -> Result<CommandToken, ...> {
    let reqs = requirements_for_command(command_name)...;

    // 1. OAuth2User (preferred — user context, richest data)
    if reqs.accepted.contains(&ReqAuthType::OAuth2User) {
        if has_oauth2_available(config) {
            if let Ok(t) = ensure_access_token(client, config).await {
                return Ok(CommandToken::Bearer {
                    token: t,
                    auth_type: ReqAuthType::OAuth2User,
                });
            }
            // Full auth failed (expired + no refresh) — fall through
        }
    }

    // 2. OAuth1 (user context, no expiry)
    if reqs.accepted.contains(&ReqAuthType::OAuth1) {
        if has_oauth1_credentials(config) {
            return Ok(CommandToken::OAuth1);
        }
    }

    // 3. Bearer (app-only, fallback)
    if reqs.accepted.contains(&ReqAuthType::Bearer) {
        if let Some(t) = resolve_bearer_token(config) {
            return Ok(CommandToken::Bearer {
                token: t,
                auth_type: ReqAuthType::Bearer,
            });
        }
    }

    Err(auth_required_error(command_name))
}
```

The **lightweight pre-check** `has_oauth2_available()` avoids calling the heavy
`ensure_access_token()` when OAuth2 is clearly not configured:

```rust
/// Quick check: is there any OAuth2 credential source available?
/// Does NOT load tokens.json, does NOT check expiry, does NOT refresh.
fn has_oauth2_available(config: &ResolvedConfig) -> bool {
    config.access_token.is_some()
        || std::env::var("X_API_ACCESS_TOKEN").is_ok()
        || config.tokens_path.exists()
}
```

If the pre-check passes but `ensure_access_token()` fails (e.g., expired token,
refresh fails), execution falls through to OAuth1 → Bearer. This preserves the
current graceful degradation behavior.

Similarly, extract `has_oauth1_available()` (named to match `has_oauth2_available` —
both answer "can we attempt this auth method?"):

```rust
fn has_oauth1_available(config: &ResolvedConfig) -> bool {
    config.oauth1_consumer_key.is_some()
        && config.oauth1_consumer_secret.is_some()
        && config.oauth1_access_token.is_some()
        && config.oauth1_access_token_secret.is_some()
}
```

This also eliminates the DRY violation with `doctor.rs:265-268` which has the same
inline check.

**Files changed:** `src/auth.rs`, `src/doctor.rs` (update `build_auth_state()` to match
new resolution order — STAR principle requires single truth for auth priority)

### Phase 4: Simplify command auth branching (optional, separate PR)

After Phases 1-3, every command has the same pattern:

```rust
let response = match &token {
    CommandToken::Bearer { token, auth_type } => {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", token).parse()?);
        let ctx = RequestContext { auth_type: &auth_type, username: config.username.as_deref() };
        client.get(&url, &ctx, headers).await?
    }
    CommandToken::OAuth1 => client.oauth1_request("GET", &url, config, None).await?,
};
```

Both branches now go through the cache. The branching is only about how
the HTTP request is authenticated, not whether it's cached. Consider extracting
a helper to reduce the repeated pattern across 6 command files. This is optional
but would reduce ~60 lines of boilerplate.

**Files changed:** Optionally `src/cache/mod.rs` (new helper), all command files

## Technical Considerations

### Cache key stability

Changing auth resolution order means existing cache entries may be keyed under a
different `auth_type` than what will be used going forward. E.g., a profile cached
under `"bearer"` won't be found when the same profile is requested under `"oauth2_user"`.
These orphaned entries expire via TTL (max 1 hour for profiles, 15 min for most).
No migration needed — just a transient cache miss after upgrade.

**Security note (from security review):** There is a subtle "auth downgrade to stale
cache" scenario: if OAuth2 expires and the user falls back to Bearer, the old
bearer-keyed cache entry could serve stale data from before the auth change. The
simplest mitigation is a cache key version bump:

```rust
const CACHE_KEY_VERSION: &str = "v2";
// Include in compute_cache_key format string
```

This invalidates all old entries without requiring `bird cache clear`.

**Recommendation:** Recommend `bird cache clear` in release notes. Consider the
version bump if multi-auth users are common.

### OAuth1 signing and cache interaction

OAuth1 signatures include a per-request nonce and timestamp. The cache must check
BEFORE signing to avoid wasted crypto operations on cache hits. This is naturally
achieved by the proposed flow (cache check → sign only on miss).

### `--no-cache` invisible to usage tracking

When `--no-cache` is set, the entire DB connection is disabled, which means OAuth1
requests with `--no-cache` are invisible to `bird usage` — same as OAuth2 requests
with `--no-cache`. This is consistent behavior, not a new gap.

### Doctor report

The doctor's `build_auth_state()` at `src/doctor.rs:261-302` has the **same inverted
resolution order** (Bearer → OAuth1 → OAuth2User). After Phase 3, `bird doctor`
would report a different primary auth type than what commands actually use — a user
with both Bearer and OAuth2 configured would see `auth_type: "bearer"` in the doctor
but commands would use OAuth2User. This violates STAR (Single Truth, Authoritative
Record). **Include doctor.rs in Phase 3 scope.** Update `build_auth_state()` to
match the new OAuth2User → OAuth1 → Bearer order.

## Acceptance Criteria

### Functional

- [x] OAuth1 GET requests check cache before making HTTP calls
- [x] OAuth1 GET responses (2xx) are written to cache
- [x] OAuth1 POST/PUT/DELETE requests are NOT cached
- [x] `--refresh` bypasses cache read for OAuth1 (still writes)
- [x] `--no-cache` disables cache entirely for OAuth1
- [x] Auth resolution order is OAuth2User → OAuth1 → Bearer
- [x] Auth methods that aren't configured are skipped (no wasted `ensure_access_token()`)
- [x] If OAuth2 pre-check passes but full auth fails, fallback to OAuth1 → Bearer
- [x] `CommandToken::Bearer` carries the correct `AuthType` (OAuth2User vs Bearer)
- [x] Cache keys use the actual auth type, not hardcoded `OAuth2User`
- [x] `bird cache stats` shows OAuth1 cached entries
- [x] `bird usage` shows correct `cache_hit` for OAuth1 requests
- [x] `bird doctor` reports auth type consistent with actual resolution order

### Security (from deepening)

- [x] `CommandToken` has custom `Debug` impl that redacts `token` field
- [x] `reqwest_oauth1::Secrets::new` only appears in `cache/mod.rs` (Prevention Rule #4)
- [x] Cache-hit responses populate `ApiResponse.json` (not just `.body`)

### Tests

- [x] Unit test: OAuth1 cache key differs from OAuth2 cache key for same URL
- [x] Unit test: `has_oauth2_available` returns false when no sources exist
- [x] Unit test: `has_oauth1_available` returns false with partial creds
- [x] Unit test: `CommandToken::Debug` redacts bearer token
- [x] Doctor tests validate auth resolution order (6 tests)
- [x] Existing tests continue to pass (now 123: 117 unit + 6 integration)
- [ ] Future: `resolve_token_for_command` integration tests (require HTTP mock server)
- [ ] Future: `oauth1_request` cache behavior integration tests (require HTTP mock server)

### Prevention Rules (from deepening)

- [x] `log_api_call()` executes exactly once per `oauth1_request()` path (Rule #3)
- [x] Pagination URLs excluded from OAuth1 cache via `should_skip_cache()` (search pattern)
- [x] `cache_hit` propagated from `ApiResponse` to cost estimation (search pattern)
- [x] Dedicated auth constant per command preserved in `requirements.rs`

### Non-Functional

- [x] No performance regression — cache hits skip OAuth1 signing entirely
- [x] No breaking CLI flag changes (only `--account` from earlier, already done)

## Dependencies & Risks

**Risk: Behavioral change for multi-auth users.** Users with both Bearer and OAuth2
configured will switch from Bearer to OAuth2User. This is the intended fix, but
could surprise scripted pipelines. Mitigated by: this is pre-v0.1.0, and the new
order is correct semantically.

**Risk: OAuth1 cache coherence.** OAuth1 and OAuth2 responses for the same endpoint
should return the same data (both are user-context auth), but OAuth1 doesn't support
scope restrictions. If the API ever returns different data based on auth method,
separate cache keys prevent cross-contamination. The current cache key design
(includes `auth_type`) already handles this correctly.

**Dependency:** Phase 1 must land before Phase 3 to avoid the `CommandToken::Bearer`
auth_type bug becoming more visible with the new resolution order.

## Implementation Notes (from deepening)

### Pattern consistency fixes during implementation

- **raw.rs uses `match token` (by value)** while all other commands use `match &token`
  (by reference). Normalize to `match &token` during Phase 1 since the file is already
  being touched.
- **bookmarks.rs has a special case** — it only accepts OAuth2User, so the `auth_type`
  extracted from `CommandToken::Bearer` will always be `AuthType::OAuth2User`. The change
  is purely mechanical (no behavioral change), unlike other commands where it fixes a bug.
- **`AuthType` vs `ReqAuthType` naming** — In `src/auth.rs`, `AuthType` is imported as
  `ReqAuthType` via alias. Use `ReqAuthType` consistently in `auth.rs` code, `AuthType`
  everywhere else.

### Performance characteristics (from performance review)

- Cache lookup: ~10-50μs (primary key B-tree lookup on in-process SQLite with WAL)
- OAuth1 HMAC-SHA1 signing: ~1-10μs
- HTTP round-trip saved on cache hit: 50-400ms (3-4 orders of magnitude improvement)
- `has_oauth2_available()` pre-check: sub-microsecond for first two branches; `stat(2)`
  syscall for `tokens_path.exists()` only fires when neither in-memory nor env var is set
- All hot-path SQLite queries use `prepare_cached` — compiled statements are reused

### Post-implementation cleanup

- Update Gotcha #3 in `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
  (currently says "OAuth1 bypasses cache by design" — will be stale after Phase 2)
- Update Section 6 in `docs/solutions/architecture-patterns/search-command-paginated-api-pattern.md`
  (notes "OAuth1 bypasses cache" — will be stale)
- Check `src/cache/mod.rs` line count after Phase 2 — if over 200 lines of non-test code,
  evaluate splitting per Prevention Rule #5

## References & Research

### Internal References

- Architecture: `src/cache/mod.rs:275-333` (oauth1_request), `src/cache/mod.rs:95-159` (get)
- Auth: `src/auth.rs:302-331` (resolve_token_for_command)
- Doctor: `src/doctor.rs:261-302` (build_auth_state — same inverted order, must update)
- Requirements: `src/requirements.rs:47-55` (per-command auth constants)
- Cache key: `src/cache/mod.rs:344-357` (compute_cache_key)
- Brainstorm: `docs/brainstorms/2026-02-11-research-commands-and-caching-brainstorm.md`
- Solution: `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
- Solution: `docs/solutions/architecture-patterns/code-review-round2-quality-improvements.md`
- Solution: `docs/solutions/architecture-patterns/search-command-paginated-api-pattern.md`
- Live test: `docs/plans/2026-02-17-test-live-production-validation-plan.md` (Phase 2 confirmed cache works for OAuth2)

### Prevention Rules Applied (from learnings)

- Rule #2: Parse JSON exactly once — cache hits must populate `ApiResponse.json`
- Rule #3: Public methods log, private methods do not — `log_api_call()` once per path
- Rule #4: All OAuth1 goes through `oauth1_request()` — single-file cache change
- Rule #5: 200-line refactor trigger — check `mod.rs` after Phase 2
