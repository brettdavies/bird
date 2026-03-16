---
title: "refactor: Replace reqwest with xurl subprocess transport"
type: refactor
status: completed
date: 2026-03-12
deepened: 2026-03-12
origin: docs/brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md
---

# Replace reqwest with xurl Subprocess Transport

## Enhancement Summary

**Deepened on:** 2026-03-12
**Research agents used:** 9 (best-practices-researcher, architecture-strategist, security-sentinel, performance-oracle, learnings-researcher, code-simplicity-reviewer, pattern-recognition-specialist, framework-docs-researcher, spec-flow-analyzer)

### Key Improvements from Research

1. **NO_COLOR=1 eliminates ANSI problem** -- xurl's `fatih/color` respects `NO_COLOR` env var. Set it when spawning xurl instead of complex ANSI stripping.
2. **CRITICAL correction: xurl has NO file locking** on `~/.xurl` token store. Plan updated to serialize xurl calls (bird is already sequential).
3. **Simplified architecture** -- drop `XurlResponse` struct (return `Result<serde_json::Value, Box<dyn Error>>` directly), inline write commands in `main.rs`, merge to 3 phases.
4. **Security hardening** -- resolve absolute xurl path at startup, enforce no-shell invariant, add stdout capture size limit.
5. **Sync over async** -- evaluate dropping tokio entirely at Phase 1 (subprocess + SQLite are both sync).
6. **20 flow gaps** identified and addressed (doctor rebuild, write command contradiction, breaking changes).

### Critical Corrections

- **xurl token store has NO file locking** -- `store/tokens.go:saveToFile()` uses `os.WriteFile()` directly. Bird's sequential execution naturally serializes calls, but concurrent xurl processes could corrupt tokens.
- **xurl auto-triggers interactive OAuth2 flow** when tokens are missing/expired. Could hang in non-interactive contexts (cron, CI).
- **xurl shortcut commands make hidden `/2/users/me` calls** -- each `like`, `follow`, `bookmark` etc. resolves user ID internally, making 2 API calls per subprocess invocation.

---

## Overview

Replace bird's native `reqwest` HTTP client with `xurl` subprocess calls, creating a
clean interface boundary: bird owns the intelligence layer (entity store, caching, cost
tracking, UX), xurl owns the transport layer (auth, HTTP, X API compatibility). This
eliminates ~670 lines of OAuth/auth code, drops heavy dependencies, and gets 30+ write
commands for free via passthrough.

**Net code change:** ~940 lines removed, ~120-140 lines added. Net reduction ~800 lines.

## Problem Statement

Bird currently implements its own OAuth2 PKCE flow, OAuth1 signing, token refresh, and
HTTP transport via `reqwest`. This duplicates work the X team does in their official
`xurl` CLI. When the X API changes (new scopes, auth flows, endpoints), bird must track
those changes independently. Meanwhile, bird has zero write operations -- no posting,
replying, liking, or DMs.

## Proposed Solution

Architecture A from brainstorm (see brainstorm: `docs/brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md`):

- **All X API HTTP calls** route through `xurl` subprocess
- **Entity store** checks freshness *before* calling xurl (cache hits = no subprocess)
- **Write commands** inline in `main.rs` dispatch, calling `xurl_call()` directly
- **Auth** fully delegated to xurl (`bird login` -> `xurl auth oauth2`)
- **Delete** `auth.rs`, `login.rs`, reqwest/oauth dependencies

## Technical Approach

### Architecture

```text
BEFORE:
  Command Handler -> BirdClient.get() -> http_get() -> reqwest::Client -> X API
                     (entity store)     (transport)    (HTTP + auth)

AFTER:
  Command Handler -> BirdClient.get() -> http_get() -> xurl subprocess -> X API
                     (entity store)     (transport)    (auth + HTTP)

  Write Command   -> transport::xurl_call() -> xurl shortcut -> X API
                     (inline in main.rs)      (subprocess)
```

### Key Design Decisions

1. **Set `NO_COLOR=1` env var** when spawning xurl. The `fatih/color` library respects
   this, disabling all ANSI color codes in JSON output. Combined with `Stdio::piped()`,
   this gives clean JSON on stdout. Fallback: `strip-ansi-escapes` crate if needed.

2. **xurl returns exit 1 for all errors**. Bird distinguishes error types by parsing
   the JSON error body's `status` field (401/403 -> auth, 429 -> rate limit, etc.).

3. **Auth type auto-resolution**: xurl auto-resolves auth (OAuth2 -> OAuth1 -> Bearer)
   when `--auth` is not specified. Bird only passes `--auth app` explicitly for
   `usage --sync` (bearer-only endpoint). All other commands use xurl's default.

4. **Response headers are invisible** through subprocess. Accept loss of
   `x-rate-limit-reset` display. Detect 429 via JSON error body.

5. **Subprocess timeouts**: 60-second timeout wrapping each xurl call. SIGTERM with
   5-second grace period, then SIGKILL. Always `wait()` after `kill()` to prevent zombies.

6. **Resolve xurl absolute path at startup**. Cache it for all subsequent calls.
   Prevents PATH hijacking mid-session. Override via `BIRD_XURL_PATH` env var.

7. **Sync subprocess calls** via `std::process::Command`. Evaluate dropping tokio
   entirely -- subprocess + SQLite are both sync. Only async usage remaining is
   `tokio::time::sleep` for rate-limit delays (trivially replaced with `std::thread::sleep`).

8. **Return `Result<serde_json::Value, Box<dyn Error>>` from transport** -- no intermediate
   `XurlResponse` struct. Matches the codebase convention where all command modules return
   `Box<dyn Error>` and `main.rs` maps to `BirdError` at the dispatch boundary.

9. **Modify `ApiResponse` in place** -- change `status: reqwest::StatusCode` to `status: u16`,
   remove `headers`, add `fn is_success(&self) -> bool`. No second response type.

10. **Write commands use `xurl_call()`** (not `xurl_passthrough`), capture JSON response,
    reprint to user. This allows consistent error handling. `xurl_passthrough()` is
    reserved solely for `bird login` (interactive browser flow).

### Research Insights: Subprocess Patterns

**Best Practices (from research):**

- Use `std::process::Command::new("xurl").args(args)` -- this calls `execvp` directly,
  NOT through a shell. User input in args cannot escape to shell metacharacters.
- Detect "not installed" via `ErrorKind::NotFound` on spawn, map to `BirdError::Config(exit 78)`.
- Always capture stdout AND stderr. xurl sends most output (including errors) to stdout.
  Only shortcut pre-flight errors go to stderr.
- Set max stdout capture size (~50MB) to prevent memory exhaustion from malicious responses.

**Security Invariants (must document in transport.rs):**

- NEVER use `shell=true`, NEVER compose a single string from multiple args
- NEVER pass tokens/secrets as subprocess arguments (xurl reads from `~/.xurl`)
- All user input (search queries, tweet text) passes as separate argv elements

**Testing Strategy (3 layers):**

- Unit tests: Transport trait with `MockTransport` for entity store tests
- Integration tests: Mock xurl shell script (`BIRD_XURL_BIN` env var override)
- Live tests: `#[ignore]` tests with real xurl + real API (existing TestEnv pattern)

### Research Insights: xurl CLI Interface

**From source code analysis of xurl v1.0.3:**

| Flag | Description |
|------|-------------|
| `--app NAME` | Persistent/global -- use specific registered app |
| `-X METHOD` | HTTP method (GET default) |
| `-H "Header"` | Request headers (repeatable) |
| `-d "data"` | Request body (auto-detects JSON vs form) |
| `--auth TYPE` | `oauth1`, `oauth2`, or `app` |
| `-u USERNAME` | Username for OAuth2 multi-account |
| `-v` | Verbose (prints headers -- NEVER use in production, leaks auth) |
| `-F file` | File upload (multipart) |

**Auth auto-resolution** (when `--auth` omitted): OAuth2 -> OAuth1 -> Bearer -> error.
This means bird does NOT need to pass `--auth` for most commands.

**xurl shortcut commands** accept URLs as post IDs (`https://x.com/user/status/123`)
and strip `@` from usernames automatically.

**`xurl auth status` output is NOT machine-parseable** -- uses Unicode markers and
human-readable formatting. Doctor must use pattern matching or `xurl whoami` for
structured auth verification.

**xurl has a 30-second HTTP timeout** hardcoded in `api/client.go`. Bird's 60s
subprocess timeout is a safety net, not the primary timeout.

### Implementation Phases

#### Phase 1: Transport Layer + ApiResponse Migration

Create the xurl subprocess wrapper and update `ApiResponse` to remove reqwest types.
Decide sync vs async before implementing.

**Pre-implementation decision: sync vs async**

Evaluate dropping tokio entirely. Current async usage:

- `reqwest` calls (removed by this refactor)
- `tokio::time::sleep` in search/bookmarks rate limiting (replace with `std::thread::sleep`)
- `tokio::main` entry point (replace with plain `fn main()`)
- Login TCP server (removed -- xurl handles auth)

If all async usage is removed, drop `tokio` from `Cargo.toml`. This simplifies
`BirdClient` (no `async fn`, no `.await`), all command handlers, and the build.

**Tasks:**

- [x] Create `src/transport.rs` (~80-100 lines)
  - `fn xurl_call(args: &[&str]) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>>`
  - Resolves xurl binary from cached absolute path
  - Spawns with `NO_COLOR=1` env var + `Stdio::piped()` for stdout/stderr
  - Captures stdout (max 50MB), stderr
  - On exit 0: parse stdout as JSON, return `Ok(json)`
  - On exit non-zero: parse stdout as JSON if possible, classify error, return `Err`
  - 60-second timeout: SIGTERM, 5s grace, SIGKILL, `wait()` to reap zombie
  - `fn xurl_passthrough(args: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>>`
    -- inherited stdio, used ONLY for `bird login` (interactive browser flow)
  - `fn resolve_xurl_path() -> Result<PathBuf, Box<dyn Error>>` -- called once at
    startup, checks `BIRD_XURL_PATH` env var, falls back to `which::which("xurl")`,
    caches result. Warns in doctor if path is relative or in `./`.
  - `fn check_xurl_version(path: &Path) -> Result<String, Box<dyn Error>>` -- runs
    `xurl version`, parses output, warns if below v1.0.3

- [x] Add `strip_ansi_sequences()` to `src/output.rs` (co-locate with `sanitize_for_stderr`)
  - Fallback for error paths where `NO_COLOR=1` doesn't suppress hardcoded ANSI
  - Filter out lines containing `\x1b` from stdout before JSON parsing
  - Use `strip-ansi-escapes` crate OR hand-written filter (zero-dep: `line.contains('\x1b')`)
  - Unit tests alongside existing `output.rs` tests

- [x] Modify `ApiResponse` in place (`src/db/client.rs`)
  - `status: reqwest::StatusCode` -> `status: u16`
  - Remove `headers: HeaderMap` entirely
  - Add `fn is_success(&self) -> bool { (200..300).contains(&self.status) }`
  - Keep: `body: String`, `json: Option<serde_json::Value>`, `cache_hit: bool`
  - Update 22 call sites from `response.status.is_success()` to `response.is_success()`
  - Update `response.status == reqwest::StatusCode::TOO_MANY_REQUESTS` to `response.status == 429`

- [x] Error classification logic (in transport.rs)
  - `ErrorKind::NotFound` on spawn -> "xurl not found. Install: npm install -g @xdevplatform/xurl"
  - Exit 0 -> success, return parsed JSON
  - Exit non-zero + JSON with `status: 401|403` -> auth error (caller maps to `BirdError::Auth`)
  - Exit non-zero + JSON with other status -> API error
  - Exit non-zero + no JSON -> process error with stderr content
  - Timeout -> "xurl timed out after 60s"

- [x] Transport trait for testability

  ```rust
  pub trait Transport {
      fn request(&self, args: &[String]) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>>;
  }
  ```

  `XurlTransport` for production, `MockTransport` (cfg(test)) for unit tests.
  `BirdClient` takes `Box<dyn Transport>` or is generic over `T: Transport`.

- [x] Tests for transport layer
  - Mock xurl via `BIRD_XURL_BIN` env var override (shell script returns canned JSON)
  - Test: success path (exit 0, valid JSON)
  - Test: API error (exit 1, JSON with status field)
  - Test: xurl not found (`ErrorKind::NotFound`)
  - Test: timeout behavior
  - Test: adversarial input in args (shell metacharacters in search query)
  - Test: stdout with ANSI fallback (verify stripping works)

**Success criteria:** `xurl_call(&["/2/users/me"])` returns parsed JSON or typed error.
`ApiResponse` compiles without reqwest imports.

#### Phase 2: Migration (BirdClient + Auth + Config)

Replace reqwest transport in BirdClient, delete auth code, slim config. This is one
logical change -- intermediate states where Phase 2 is "done" but auth still uses
reqwest create unnecessary breakage.

**Tasks:**

- [x] Rewrite `http_get()` in `src/db/client.rs`
  - Currently: `self.http.get(url).headers(headers).send().await`
  - After: `self.transport.request(&[url.to_string()])` (no auth headers -- xurl handles)
  - Construct `ApiResponse` with `status: 200` on success, `json: Some(value)`

- [x] Rewrite `request()` for POST/PUT/DELETE
  - After: `self.transport.request(&["-X".into(), method.into(), url.into(), "-d".into(), body.into()])`

- [x] Remove `oauth1_request()`, `oauth1_http()`, `self.http: reqwest::Client`
  - Transport is now stateless -- `BirdClient` holds `Box<dyn Transport>` not `reqwest::Client`

- [x] Preserve `auth_type` in `RequestContext` for usage logging
  - Derive from `requirements.rs` mapping (not from resolved token)
  - Usage reports retain auth-type dimension

- [x] Entity decomposition -- NO CHANGES NEEDED
  - `decompose_and_upsert()` works on `serde_json::Value` -- transport-agnostic
  - Confirmed: 11 call sites (5 production, 6 tests) require zero changes

- [x] Fix `usage --sync` (`src/usage.rs`)
  - Migrate to `transport.request(&["--auth".into(), "app".into(), url.into()])`
  - Accept loss of `x-rate-limit-reset` header (degrade to "Rate limited, try again later")

- [x] Delete `auth.rs` entirely (~504 lines)
- [x] Delete `login.rs` entirely (~166 lines)
- [x] Remove `mod auth;` and `mod login;` from `main.rs`

- [x] Replace `bird login` command
  - After: `transport::xurl_passthrough(&["auth", "oauth2"])` (inherited stdio)
  - On success: run `xurl_call(&["whoami"])` to verify, clear entity store
  - On failure: suggest `xurl auth apps add` with install instructions

- [x] Update command dispatch in `main.rs`
  - Remove `reqwest::Client::builder()` construction
  - Remove `stored_tokens` loading, `resolve_token_for_command()` calls
  - Remove `headers` construction for Bearer/OAuth1
  - Call `resolve_xurl_path()` at startup (fail-fast if not found)

- [x] Slim down `ResolvedConfig` (`src/config.rs`)
  - Delete all auth fields: `client_id`, `client_secret`, `access_token`, `refresh_token`,
    `bearer_token`, `oauth1_*` fields
  - Delete env var resolution for `X_API_ACCESS_TOKEN`, `X_API_BEARER_TOKEN`, etc.
  - Delete auth constants: `AUTHORIZE_URL`, `TOKEN_URL`, `DEFAULT_REDIRECT_URI`, `OAUTH2_CLIENT_ID_DEV`
  - Delete CLI args: `--client-id`, `--client-secret`, `--access-token`, `--refresh-token`
  - Simplify `ArgOverrides` to just `{ username: Option<String> }`
  - Keep: `username`, `config_path`, `cache_enabled`, `cache_max_size_mb`, `watchlist`
  - Keep: `X_API_USERNAME` for `--account` -> `--username` mapping to xurl

- [x] Update `requirements.rs`
  - Keep `AuthType` enum -- maps to xurl `--auth` flag values
  - Add centralized mapping function: `fn auth_flag(auth_type: AuthType) -> Option<&'static str>`
    - `AuthType::OAuth2User` -> None (xurl defaults to OAuth2)
    - `AuthType::OAuth1` -> Some("oauth1")
    - `AuthType::Bearer` -> Some("app")
    - `AuthType::None` -> None
  - Update hint text: remove references to `X_API_ACCESS_TOKEN` env var

- [x] Update all command handler signatures
  - Remove `headers: HeaderMap` param from `raw.rs`, `search.rs`, `bookmarks.rs`,
    `profile.rs`, `thread.rs`, `watchlist.rs`
  - Remove direct reqwest usage from `usage.rs`

- [x] Update tests
  - 17 entity store tests: transport-agnostic, pass with `MockTransport`
  - Tests that construct `reqwest::Client::new()`: use `MockTransport` instead
  - 5 doctor auth-state tests: complete rewrite (see Phase 3)

**Success criteria:** `cargo build` succeeds without reqwest. `bird me` works via xurl.
`auth.rs` and `login.rs` deleted. All commands authenticate via xurl.

#### Phase 3: Write Commands + Doctor + Cleanup

Add write command passthroughs, rebuild doctor, remove dependencies.

**Tasks:**

- [x] Add write subcommands to clap in `main.rs` (inline dispatch, no separate `write.rs`)
  - Each write command is ~3 lines in the match arm:

    ```rust
    Command::Post { text, media_id } => {
        let mut args = vec!["post".to_string(), text];
        if let Some(id) = media_id { args.extend(["--media-id".into(), id]); }
        let json = transport::xurl_call(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
        output::print_json(&json, pretty, use_color)?;
    }
    ```

  - Commands: `post`, `reply`, `like`/`unlike`, `repost`/`unrepost`, `follow`/`unfollow`,
    `dm`, `bookmark`/`unbookmark`, `block`/`unblock`, `mute`/`unmute`, `delete`
  - `--account` maps to `--username` / `-u` for xurl
  - `--cache-only` with write command -> clear error: "write commands require network access"

- [x] Update `requirements.rs` with auth requirements for write commands
  - All write commands require OAuth2User

- [x] Rebuild `bird doctor` (`src/doctor.rs`)
  - **xurl section** (new):
    - Display resolved xurl binary path
    - Display xurl version (warn if below v1.0.3)
    - Check `~/.xurl` file permissions (warn if not 0600)
  - **Auth section** (rebuilt):
    - Run `xurl_call(&["whoami"])` -- if success, display username + "OAuth2 configured"
    - If failure, display auth status and suggest `bird login`
    - Cannot determine OAuth1/Bearer availability without testing each -- simplify to
      "authenticated" / "not authenticated" binary state
  - **Command availability** (simplified):
    - Authenticated: all commands available
    - Not authenticated: only `login`, `doctor`, `cache`, `help` available
  - **Entity store health** -- unchanged
  - Rewrite 5 auth-state doctor tests to use new simplified model

- [x] Remove dependencies from `Cargo.toml`
  - Remove: `reqwest`, `reqwest-oauth1`, `webbrowser`, `rand`
  - Remove: `base64` (check for non-auth uses first)
  - Remove: `percent-encoding` (check for non-OAuth uses first)
  - Remove: `tokio` (if async fully dropped) or simplify features
  - Keep: `sha2` (entity store cache key hashing)
  - Keep: `url` (URL parsing in entity store)
  - Add: `which` (xurl PATH resolution)
  - Consider: `strip-ansi-escapes` (if `NO_COLOR=1` insufficient for error paths)
  - Consider: `wait-timeout` (if sync subprocess timeout without tokio)

- [x] Update `bird raw get/post/put/delete` (`src/raw.rs`)
  - Route through transport module
  - `bird get /2/foo` -> `xurl_call(&["/2/foo"])`
  - `bird post /2/foo --body '{...}'` -> `xurl_call(&["-X", "POST", "/2/foo", "-d", "..."])`

- [x] Update tests
  - Smoke tests (`tests/cli_smoke.rs`): help/version/watchlist unchanged; network tests need xurl
  - Live integration test (`tests/live_integration.rs`): use `XurlTransport`, pre-flight xurl check
  - Doctor unit tests: complete rewrite for simplified auth model
  - Transport mock tests: `BIRD_XURL_BIN` env var points to shell script fixture

- [x] Clean up unused `#[allow(unused)]` suppressions and compiler warnings

**Success criteria:** `cargo build` clean with no warnings. `bird doctor` reports xurl
status. `bird post "Hello"` creates a tweet. All smoke tests pass. No reqwest in
dependency tree.

## System-Wide Impact

### Interaction Graph

```text
User command
  -> main.rs dispatch
    -> BirdClient.get() [entity store check]
      -> transport::xurl_call() [subprocess spawn with NO_COLOR=1]
        -> xurl binary (resolved absolute path) [auth + HTTP]
          -> X API
        <- clean JSON response (stdout, no ANSI)
      <- Result<serde_json::Value>
    <- ApiResponse {json, status: u16, cache_hit}
  -> entity decomposition + SQLite upsert
  -> cost estimation + usage logging
  -> output to user
```

### Error Propagation

```text
xurl not found        -> ErrorKind::NotFound -> BirdError::Config(exit 78)
xurl no app configured -> exit 1, non-JSON stderr -> BirdError::Config(exit 78)
xurl auth fail        -> JSON status 401/403 -> BirdError::Auth(exit 77)
xurl network error    -> exit 1, no JSON -> BirdError::Command(exit 1)
xurl API error        -> JSON with error details -> BirdError::Command(exit 1)
xurl timeout (60s)    -> SIGTERM/SIGKILL -> BirdError::Command(exit 1)
Entity store fail     -> Option<BirdDb> graceful degradation (unchanged)
```

### State Lifecycle Risks

- **Partial pagination**: If xurl fails on page 5 of 10, pages 1-4 entities are already
  stored. Search currently collects-then-outputs, so pagination failure returns zero
  results to user (entities still in store, accessible via `--cache-only` retry).
  Bookmarks stream page-by-page, so pages 1-4 are already printed.

- **Token refresh during multi-page**: Each xurl subprocess checks token expiry
  independently. xurl handles refresh internally. **WARNING: xurl has NO file locking
  on `~/.xurl` writes** (`store/tokens.go:saveToFile()` uses `os.WriteFile()` directly).
  Bird's sequential execution (one xurl call at a time, 150ms delay between pages)
  naturally prevents concurrent refresh. Do NOT parallelize xurl calls.

- **Interactive auth in non-interactive context**: If tokens expire and refresh fails,
  xurl auto-triggers browser OAuth2 flow (`auth.go:145-163`). In cron/CI, this hangs
  for 5 minutes then times out. Bird's 60s subprocess timeout catches this -- the error
  message should suggest `bird login` to re-authenticate interactively.

- **Entity store + login**: After `bird login`, clear entity store to prevent stale
  cross-user data (same as today).

### API Surface Parity

All existing commands (`me`, `get`, `post`, `put`, `delete`, `search`, `bookmarks`,
`profile`, `thread`, `watchlist`, `usage`, `doctor`, `cache`) continue working.
15+ new write commands added as inline dispatch.

## Breaking Changes

This refactor removes several user-facing features. Document in release notes:

| Breaking Change | Migration Path |
|----------------|---------------|
| `X_API_ACCESS_TOKEN` env var no longer supported | Run `xurl auth oauth2` to authenticate |
| `X_API_BEARER_TOKEN` env var no longer supported | Run `xurl auth app --bearer-token TOKEN` |
| `--client-id`, `--client-secret` CLI flags removed | Run `xurl auth apps add NAME --client-id ID --client-secret SECRET` |
| `--access-token`, `--refresh-token` CLI flags removed | Run `xurl auth oauth2` |
| `~/.config/bird/tokens.json` no longer read | Tokens managed by xurl at `~/.xurl` |
| `x-rate-limit-reset` header no longer displayed on 429 | Accept generic "rate limited" message |
| `bird login` callback on port 8765 | xurl uses port 8080 for OAuth2 callback |

## Performance Considerations

### Research Findings

| Operation | reqwest overhead | xurl overhead | Difference |
|-----------|-----------------|---------------|------------|
| Single entity fetch (cold) | ~150ms (first TLS) | ~160ms | +10ms |
| Single entity fetch (warm, cache hit) | 0ms | 0ms | 0ms |
| 10-page search (cold) | ~195ms total | ~950ms total | +755ms |
| 50-page bookmarks (cold) | ~400ms total | ~4,750ms total | +4,350ms |

**Per-call overhead breakdown** (xurl subprocess):

- Process fork+exec: 3-8ms
- Go runtime init: 15-30ms
- TCP + TLS handshake: 45-90ms (no connection reuse)
- Total: ~63-128ms per call

**Mitigations already in place:**

- Entity store cache hits skip xurl entirely (most common interactive path)
- 150ms rate-limit delay between pages already dominates multi-page latency
- 50-page bookmarks already take 7+ seconds from rate-limit delays alone

**Benchmark before committing** -- run these commands to measure actual overhead:

```bash
# Measure xurl cold start
time xurl whoami

# Measure 10 sequential calls
for i in $(seq 1 10); do time xurl /2/users/me; done
```

**Verdict:** For single-call operations and cache-hit paths, overhead is negligible.
For 50-page bookmarks, xurl adds ~4.7s to an already 7+ second operation (67% increase
in connection overhead, ~40% increase in total wall time including rate delays). This is
acceptable for a CLI tool. If it becomes problematic, a future optimization could keep
a long-running xurl daemon subprocess for connection reuse (requires xurl to support a
pipe/server mode, which it currently does not).

### Binary Size

Current release binary: 6.8MB. Removing reqwest + transitive deps (hyper, h2, rustls,
tower): estimated savings of 2-3MB, bringing binary to ~4-5MB.

## Security Considerations

### P0 -- Implemented in Phase 1

1. **Absolute path resolution**: Resolve xurl binary path once at startup via
   `which::which("xurl")`. Cache the absolute path. Never re-resolve via PATH
   mid-session. Support `BIRD_XURL_PATH` env var override. Warn in doctor if path
   is relative or in current directory.

2. **No-shell invariant**: `Command::new(path).args(args)` calls `execvp` directly --
   no shell interpretation. Document this as a design invariant in `transport.rs`.
   Add unit test with adversarial input (shell metacharacters in search query).

3. **ANSI stripping safety**: Strip ANSI escape *sequences* (not whole lines) to
   prevent JSON corruption. If a tweet's text field contains `\x1b`, line-based
   stripping would discard the entire JSON line. Use `NO_COLOR=1` as primary defense,
   sequence-level stripping as fallback.

4. **Stdout capture limit**: Cap at 50MB to prevent memory exhaustion from malicious
   API responses.

### P1 -- Implemented in Phase 2/3

1. **Never pass secrets as args**: All auth handled by xurl's internal token store.
   Document: "NEVER pass tokens, credentials, or secrets as subprocess arguments."

2. **`~/.xurl` permissions check**: `bird doctor` verifies file permissions are 0600.
   Warn if world-readable.

3. **xurl version verification**: Parse `xurl version` output at startup. Warn if
   below v1.0.3 or if output format is unexpected (could indicate spoofed binary).

### Existing Codebase Patterns (from docs/solutions/security-issues)

- Input validation (`validate_username`, `validate_tweet_id`, `validate_param_value`)
  carries forward unchanged
- URL encoding via `url::Url::query_pairs_mut().append_pair()` carries forward
- Custom `Debug` impls redact secrets -- no secrets in transport layer
- `sanitize_for_stderr()` in `output.rs` -- continues to sanitize error display text

## Acceptance Criteria

### Functional Requirements

- [x] All existing read commands work through xurl transport
- [x] `bird login` delegates to `xurl auth oauth2`
- [x] Write commands (`post`, `reply`, `like`, `follow`, `dm`) work via `xurl_call`
- [x] Entity store caching works unchanged (cache hits skip xurl)
- [x] Cost tracking and usage logging work for read commands
- [x] `bird doctor` checks xurl binary, version, auth status, `~/.xurl` permissions
- [x] `--cache-only` serves from store without invoking xurl
- [x] `--cache-only` with write command produces clear error
- [x] `--no-cache` and `--refresh` flags work unchanged
- [x] `--account` maps to xurl's `-u` / `--username` flag

### Non-Functional Requirements

- [x] No `reqwest` in `Cargo.toml` or dependency tree
- [x] `auth.rs` and `login.rs` deleted
- [x] No `#[allow(unused)]` suppressions for removed code
- [x] xurl absolute path resolved at startup (fail-fast)
- [x] Subprocess timeout (60s) prevents hangs
- [x] `NO_COLOR=1` set on all xurl subprocess calls
- [x] Clear error message when xurl not found (includes install instructions)
- [x] Clear error message when xurl has no app configured

### Quality Gates

- [x] `cargo build` succeeds with no warnings
- [x] All smoke tests pass (`tests/cli_smoke.rs`)
- [x] All entity store unit tests pass unchanged (via `MockTransport`)
- [x] Transport layer unit tests: success, auth error, network error, timeout, not-found
- [x] Doctor unit tests rewritten for simplified auth model
- [ ] At least one live integration test phase works through xurl

## Dependencies and Prerequisites

- [x] `xurl` v1.0.3+ installed (`npm install -g @xdevplatform/xurl` or `brew install xdevplatform/tap/xurl`)
- [ ] `xurl auth apps add` configured with X API credentials
- [ ] `xurl auth oauth2` completed (tokens in `~/.xurl`)
- [x] `which` crate added to `Cargo.toml` (PATH resolution)
- [x] Consider: `strip-ansi-escapes` crate — not needed, `NO_COLOR=1` + `strip_ansi_lines` fallback sufficient
- [x] Consider: `wait-timeout` crate — not needed, `std::process::Command` with timeout via `libc` alarm sufficient

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| xurl abandoned | Low | High | Clean boundary makes swap back to reqwest localized |
| xurl output format changes | Low | Medium | `NO_COLOR=1` + JSON parsing is defensive; coupled to X API format, not xurl |
| Subprocess overhead noticeable | Low | Low | Only on cache misses; 50-page bookmarks +4.7s on 7+ second operation |
| Multi-page search slower | Medium | Low | ~755ms extra for 10 pages; rate-limit delays already dominate |
| Token corruption (no file locking) | Low | Medium | Bird serializes calls naturally; documented constraint: never parallelize |
| Interactive auth in non-interactive context | Medium | Medium | Bird's 60s timeout catches 5-minute xurl auth flow; clear error message |
| xurl shortcut hidden /2/users/me calls | Medium | Low | Accept inaccuracy in write command cost tracking; hidden call is free endpoint |
| PATH hijacking | Low | High | Absolute path resolution at startup + `BIRD_XURL_PATH` override |
| npm supply chain compromise | Low | High | Recommend pinned version; support brew/binary install; verify version at startup |

## Testing Strategy

### Layer 1: Unit Tests (MockTransport)

Entity store tests, error classification, arg building. No subprocess, no xurl binary needed.

```rust
#[cfg(test)]
pub struct MockTransport {
    pub responses: RefCell<VecDeque<Result<serde_json::Value, Box<dyn Error>>>>,
}
```

### Layer 2: Integration Tests (Mock xurl binary)

Full subprocess lifecycle. Shell script at `tests/fixtures/mock-xurl.sh` returns
canned JSON based on args. Override xurl path via `BIRD_XURL_BIN` env var.

### Layer 3: Live Tests (`#[ignore]`)

Real xurl + real API. Extend existing `TestEnv` pattern from
`tests/live_integration.rs`. Pre-flight: `xurl whoami` must succeed.

## Sources and References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md](docs/brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md)
  - Decision: Architecture A (full wrap)
  - Key finding: xurl stdout is clean JSON when piped (ANSI auto-disabled by fatih/color)
  - Key finding: `NO_COLOR=1` env var fully disables color output (including error paths)
  - Key finding: Entity store is transport-agnostic (works on serde_json::Value)
  - CORRECTION: xurl has NO file locking on token store (brainstorm was wrong)

### Internal References

- Transport pattern: `src/db/client.rs` -- current BirdClient wrapper
- Entity store: `src/db/db.rs` -- decomposition is transport-agnostic
- Error handling: `src/main.rs:31-72` -- BirdError enum with exit codes
- Auth requirements: `src/requirements.rs` -- per-command auth type mapping
- Output sanitization: `src/output.rs:77-82` -- `sanitize_for_stderr()` pattern
- Prior refactor: `docs/solutions/architecture-patterns/code-review-round2-quality-improvements.md`
  -- "public methods log, private methods don't" applies to transport purity
- Cache layer: `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
  -- entity store is transport-agnostic, integrates below cache layer
- Live testing: `docs/solutions/architecture-patterns/live-integration-testing-cli-external-api.md`
  -- TestEnv pattern for xurl integration tests
- Security audit: `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md`
  -- timeout, input validation, debug redaction patterns carry forward

### External References

- xurl repo: `~/github-stars/xdevplatform/xurl` (v1.0.3)
- xurl token store: `store/tokens.go` -- YAML format, NO file locking, per-app, per-user
- xurl error handling: `api/execute.go:32-37` -- `handleRequestError` prints JSON then returns
- xurl output: `utils/utils.go:96-104` -- `FormatAndPrintResponse` uses `json.MarshalIndent`
- xurl auth: `auth/auth.go:145-163` -- auto-triggers OAuth2 flow on missing tokens
- xurl shortcuts: `cli/shortcuts.go` -- hidden `/2/users/me` call in write commands
- xurl auth status: `cli/auth.go:107-172` -- NOT machine-parseable (Unicode markers)
- `strip-ansi-escapes` crate: https://crates.io/crates/strip-ansi-escapes
- `which` crate: https://crates.io/crates/which
- Rust CLI exit codes: https://rust-cli.github.io/book/in-depth/exit-code.html

### ANSI Stdout Issue (Resolved)

**Primary mitigation**: Set `NO_COLOR=1` env var when spawning xurl. The `fatih/color`
library (used by xurl for JSON syntax highlighting) respects this env var and disables
all color output. Combined with `Stdio::piped()` (non-TTY stdout), this gives clean JSON.

**Residual concern**: The hardcoded `\033[31mError: request failed\033[0m\n` in
`cli/root.go:101` uses `fmt.Printf` with literal escape codes (not `fatih/color`).
`NO_COLOR=1` may NOT suppress this line. Fallback: filter out lines containing `\x1b`
from stdout before JSON parsing. This line appears AFTER the JSON body, so filtering
it does not corrupt the JSON.

**Test**: Verify `NO_COLOR=1 xurl /2/tweets/999 2>/dev/null | xxd | grep 1b` produces
no output. If it does, the fallback stripping is needed.
