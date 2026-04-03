---
date: 2026-04-03
topic: xurl-crate-import-migration
---

# xurl-rs Crate Import Migration

## Problem Frame

Bird currently shells out to the xurl CLI binary (`xr`) for all X API calls. The subprocess transport layer
(`transport.rs`) spawns xurl as a child process, captures stdout/stderr, parses JSON from string output, and classifies
errors via string matching on exit codes and response bodies. This architecture was the right call in March 2026 when
xurl had no library API (see `docs/brainstorms/2026-03-12-wrap-xurl-vs-native-api-client.md`).

xurl-rs v1.1.0 now ships a full library API: 29 typed shortcut functions returning `ApiResponse<T>` with compile-time
type safety, an `Auth` struct for in-process OAuth flows, and `send_request()` for untyped passthrough. The subprocess
layer is now unnecessary complexity that adds ~50-100ms latency per cache miss, fragile string-based error handling, and
runtime coupling to the xurl binary's PATH availability.

This migration replaces the subprocess transport with direct Rust library calls, giving bird typed responses, in-process
auth, and zero runtime dependency on the xurl CLI binary.

## Requirements

**Transport Replacement**

- R1. Replace all subprocess calls to xurl CLI with direct method calls on `xurl::api::ApiClient` (e.g.,
  `client.create_post(text, &[], &call_opts)` instead of spawning `xr post`)
- R2. Remove the `Transport` trait, `XurlTransport`, and `MockTransport` entirely
- R3. Remove `transport.rs` and its subprocess infrastructure (`resolve_xurl_path`, `verify_xurl_binary`, `xurl_call`,
  `xurl_passthrough`, `wait_with_timeout`, `classify_error`)
- R4. Remove dependencies only needed for subprocess management (`which`, `semver`); retain `libc` for SIGPIPE handling
  in `main.rs` (the only non-transport usage)

**Typed Responses**

- R5. Command handlers receive typed `ApiResponse<T>` from xurl shortcut methods (`Tweet`, `User`, `DmEvent`, action
  confirmations like `LikedResult`, `FollowingResult`, etc.) via `client.method(&call_opts)` pattern using xurl's
  `CallOptions` type (not `RequestOptions`, which is internal to xurl's raw request path)
- R6. Entity store stores typed structs serialized to `raw_json` via `serde_json::to_string()`; no SQLite schema
  migration required. Existing decomposed index columns (`id`, `author_id`, etc.) populated from typed struct fields.
  Further column decomposition deferred to follow-up if a concrete query need arises.
- R7. `bird raw` (GET/POST/PUT/DELETE) uses xurl's `client.send_request(&request_opts)` which returns `Value`
  (inherently untyped). This is the only path that uses `RequestOptions` directly — all other commands use shortcut
  methods with `CallOptions`
- R8. Usage sync (`usage.rs`) routes through xurl's `get_usage()` shortcut via `ApiClient`, replacing the current
  Transport-based path. (Bird has no direct reqwest dependency today; reqwest returns as a transitive dependency of
  xurl-rs.)

**Auth Delegation**

- R9. `bird login` calls `xurl::auth::Auth::oauth2_flow()` directly instead of `xurl_passthrough()`
- R10. Auth type selection (`requirements.rs`) refactored to call xurl library auth methods; exact mapping TBD during
  planning (currently generates `--auth` flag strings that have no library equivalent)
- R11. Bird delegates token storage, refresh, and write operations to xurl's `Auth`/`TokenStore`. Read-only access to
  token state is permitted for diagnostics (e.g., `bird doctor` auth status check).

**User ID Resolution**

- R22. Bird resolves the authenticated user's `user_id` lazily via `get_me()` on first need, cached in `BirdClient`
  (e.g., `OnceCell<String>`) for the session. Commands that don't need it (search, raw, me, doctor) pay zero cost. The
  `bird doctor` auth check naturally primes this cache when it runs `get_me()`.
- R23. Bird resolves target usernames to `user_id` via `lookup_user()` for user-targeting commands (`follow`,
  `unfollow`, `block`, `unblock`, `mute`, `unmute`). Results are cached in the entity store — subsequent lookups for the
  same username hit cache. User ID resolution was deliberately kept as consumer responsibility in xurl v1.2.0 (Layer 3
  composition) — bird implements its own resolution with caching rather than using xurl helpers.

**Error Handling**

- R12. Map `xurl::error::XurlError` variants to bird's `BirdError` variants at command boundaries. xurl v1.2.0 ships
  `Api { status: u16, body: String }` for HTTP errors and `Validation(String)` for non-HTTP application errors (input
  validation, errors-only 200 responses, media processing failures, user-not-found). Bird maps both: HTTP errors use
  status-code pattern matching (`match XurlError::Api { status: 401, .. } => ...`), `Validation` errors map to
  `BirdError::Command`.
- R13. Preserve existing exit codes: 78 (config), 77 (auth), 1 (command)
- R14. User-facing error messages remain equivalent to current behavior

**Doctor Command**

- R15. Replace "xurl binary found/version" check with xurl config validity check (env vars, token store accessibility)
- R16. Auth status check calls xurl library methods directly (e.g., `Auth::refresh_oauth2_token()` or equivalent probe)
- R17. Per-command availability logic unchanged (same `requirements.rs` mapping, different underlying check)

**Behavioral Invariants**

- R18. CLI output semantically equivalent for all commands (same fields and values; key ordering and null-vs-absent may
  differ due to typed round-trip serialization — JSON consumers use proper parsing, not string comparison)
- R19. Exit codes identical for all error conditions
- R20. Same command set — no new commands, no removed commands
- R21. Cache behavior functionally equivalent (same hit/miss semantics; no schema migration — typed structs serialized
  to existing `raw_json` column)

## Success Criteria

- All existing bird commands produce functionally equivalent output before and after migration
- Zero runtime dependency on xurl CLI binary — `xr` not required on PATH
- All tests pass with typed internals (new test strategy replaces `MockTransport`)
- `which`, `semver` removed from Cargo.toml; `libc` retained for SIGPIPE handling only
- `transport.rs` deleted
- `cargo clippy` and `cargo test` pass clean

## Scope Boundaries

- No new CLI commands or flags in this migration
- No UX improvements (richer output, better error messages, new display fields) — follow-up work
- No `value_hint = Url` addition to URL args — follow-up TODO (see
  `.context/compound-engineering/todos/003-pending-p3-value-hint-url-after-xurl-crate-import.md`)
- No changes to cache eviction or expiry logic
- No changes to cost estimation logic
- No changes to watchlist functionality (local-only, no API calls)
- No changes to output formatting (`output.rs`) or color handling

## Key Decisions

| Decision | Rationale |
|---|---|
| Full typed integration — remove Transport trait | The trait was a subprocess abstraction; typed library calls don't fit `fn request(&self, args: &[String]) -> Result<Value>`. Removing it eliminates indirection without losing testability. |
| `bird raw` uses `send_request()` (Value) | Raw is inherently untyped. Using the library's untyped path eliminates the last subprocess call while matching the command's semantics. |
| `bird login` calls `Auth::oauth2_flow()` directly | Eliminates subprocess for the interactive OAuth flow. Same UX (browser open + callback) via in-process execution. |
| Entity store: typed at API boundary, serialized for storage | Command handlers receive typed structs; entity store serializes them to `raw_json`. Avoids a SQLite schema migration on the critical path while still getting type safety at the API call site. Column decomposition deferred to follow-up. |
| Pure infrastructure swap — no UX changes | Keeps the migration reviewable and testable. UX improvements ship as separate follow-up PRs. |
| Depend on xurl-rs via crates.io | Standard crate dependency. No path deps, no git deps, no local machine coupling. |
| Parallel xurl v1.2.0 + bird migration | xurl v1.2.0 ships owned `ApiClient` (no lifetime), `from_env()`, `CallOptions` for shortcut methods, shortcuts as methods on ApiClient, structured `Api { status, body }` errors for HTTP responses, `Validation(String)` for non-HTTP errors, and body-only Display format preservation. Breaking changes acceptable in v1.x since bird coordinates releases. Bird targets v1.2.0 directly. |
| Bird uses `ApiClient::new()`, not `from_env()` | Bird needs `Auth::with_app_name()` for app_name override. `from_env()` is convenience for simple consumers with no customization. Bird constructs `Config::new()` + `Auth::new(&cfg)` + `auth.with_app_name()` + `ApiClient::new(config, auth)`. |
| Bird constructs `CallOptions` for shortcut calls | `CallOptions { auth_type, username, no_auth, verbose }` replaces `RequestOptions` for all shortcut methods. `bird raw` is the only path using `send_request(&RequestOptions)` directly. |
| Typed fixtures + wiremock for testing | Unit tests construct `ApiResponse<T>` directly (fast, no network). Integration tests use wiremock to mock HTTP at server level. Two layers replace MockTransport. |
| Semantically equivalent JSON output | Key ordering and null-vs-absent may differ from raw API JSON due to typed round-trip serialization. JSON consumers use proper parsing. Not byte-identical. |
| Remove bird's `ApiResponse`, use xurl's | Bird's `ApiResponse` (status, body, json, cache_hit) was a subprocess artifact. Removed entirely; xurl's `ApiResponse<T>` used directly. Cache-hit tracking moves to a separate mechanism (e.g., `CacheResult<T>` wrapper or return metadata). |
| User ID resolution is bird's responsibility | xurl v1.2.0 deliberately keeps resolve helpers as consumer-owned Layer 3 composition. Bird implements its own resolution with `OnceCell` caching (my_id) and entity store caching (username lookups). |
| 5-PR incremental migration | PR1: foundation + `bird me` proof-of-concept. PR2: read commands. PR3: write commands + user_id resolution. PR4: raw + usage + login + doctor. PR5: cleanup (delete transport.rs, old deps, old types). |

## Dependencies / Assumptions

- xurl-rs v1.2.0 ships in parallel with this migration: owned `ApiClient` (no lifetime), `from_env()` returning
  `Result<ApiClient>`, `CallOptions` type for shortcut methods, 29 shortcuts as methods on ApiClient, structured `Api {
  status: u16, body: String }` for HTTP errors, `Validation(String)` for non-HTTP errors (errors-only 200s, input
  validation, media failures), body-only Display format preservation, `exit_code_for_error()` as public library
  function, and resolve helpers excluded (consumer responsibility). See
  `~/dev/xurl-rs/docs/brainstorms/2026-04-03-library-ergonomics-requirements.md`.
- xurl's `Auth::oauth2_flow()` provides equivalent interactive OAuth UX to the current `xurl_passthrough` flow (browser
  open, callback listen, token storage)
- xurl's `Config` env vars (`CLIENT_ID`, `CLIENT_SECRET`, etc.) are compatible with bird's deployment context. Bird must
  initialize xurl's `Config` and `Auth` at startup — this is new infrastructure, not a swap.
- Both repos (`bird`, `xurl-rs`) are maintained by the same developer with coordinated releases. Breaking changes
  acceptable in xurl v1.x since bird is the only crate consumer.
- xurl-rs provides only a blocking (synchronous) API via `reqwest::blocking`. Async would require xurl-rs changes.
- Adding xurl-rs as a dependency transitively pulls in `reqwest`, `tokio`, `hyper`, `rustls`, and related crates. Bird
  previously removed these during the subprocess migration; they return through xurl-rs. This is acceptable — the
  subprocess overhead they replaced is worse than the compile-time cost.
- Bird will depend on `xurl-rs = "1.2"` (targeting v1.2.0 with the ergonomic improvements).

## Outstanding Questions

### Resolve Before Planning

(none)

### Deferred to Planning

- ~~(Affects R1) How does bird initialize xurl's Config and Auth?~~ **RESOLVED:** Bird calls `Config::new()` +
  `Auth::new(&cfg)` + `auth.with_app_name()` + `ApiClient::new(config, auth)`. Bird does NOT use `from_env()`.
- (Affects R10, Technical) How does `requirements.rs` auth type selection translate to `CallOptions.auth_type`?
  Currently generates `--auth` flag strings. Now sets `call_opts.auth_type = "oauth2"` (or "oauth1", "app") directly.
- ~~(Affects R12) Exact mapping from XurlError to BirdError.~~ **RESOLVED:** `Api { status: 401, .. }` →
  `BirdError::Auth`, `Api { status: 429, .. }` → `BirdError::Command` (rate limit), `Validation(_)` →
  `BirdError::Command`, `Auth(_)` / `TokenStore(_)` → `BirdError::Auth`.
- (Affects R9, Needs research) Does `Auth::oauth2_flow()` handle all interactive UX that `xurl_passthrough` currently
  provides? (browser open, callback listen, error display to terminal)
- (Affects R9, Technical) Post-login, bird currently clears its entity store (`client.db_clear()`). This side effect
  must be preserved when migrating to `Auth::oauth2_flow()`.
- (Affects R15, Technical) What specific config validity checks should doctor perform? `from_env()` now validates
  non-empty client_id. Doctor can call `ApiClient::from_env()` and check result for config validity.
- (Affects R8, Technical) Can usage.rs parsing simplify when consuming `UsageData` struct vs raw `Value`?

## Follow-Up Work (post-migration TODOs)

- Add `value_hint = clap::ValueHint::Url` to positional URL args
- Entity store column decomposition — decompose typed fields into dedicated SQLite columns for richer queries
- Richer tweet/profile display using typed `public_metrics`, `created_at`, etc.
- Improved user-facing error messages leveraging typed `XurlError` variants
- Explore new commands enabled by typed API access (e.g., direct timeline, mentions)

## Next Steps

-> `/ce:plan` for structured implementation planning
