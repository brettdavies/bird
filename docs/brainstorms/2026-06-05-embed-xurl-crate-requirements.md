---
date: 2026-06-05
topic: embed-xurl-crate
---

# Embed xurl v2.0.0 as a library — eliminate the subprocess transport

## Summary

Replace bird's `xr`/`xurl` subprocess transport with a direct dependency on the xurl v2.0.0 crate, so bird ships as a
single self-contained binary. Drop the `Transport` trait in favor of a held `ApiClient`, dissolve bird's per-command
auth table into call-site lookups against xurl's generated `auth_matrix`, and grow a parity flag surface so every
behavior subprocess users can reach today stays reachable. The refactor stays synchronous; full async + multi-threaded
bird is a planned fast-follow gated on xurl-rs v3.0.0.

---

## Problem Frame

The previous transport migration (PR #8, March 2026) moved bird off embedded `reqwest` + `tokio` + OAuth1/OAuth2 onto an
`xr`/`xurl` subprocess delegation. That eliminated 1,683 lines of duplicated auth/transport code and cut `Cargo.lock`
from 303 to 166 crates. The pattern was the right call at the time because xurl exposed no library API — bird either
re-implemented X auth or shelled out.

Two things have changed since:

- **xurl-rs v2.0.0 ships a deliberately ergonomic library surface.** PR #21 (`feat(api): library ergonomics for crate
  consumers`) reshaped `ApiClient` for embedding: no lifetime parameter (`ApiClient` owns `Auth` by value), `from_env()`
  one-liner constructor, 29 shortcut methods on the client, structured `XurlError::Api { status, body }` for pattern
  matching, and a generated `auth_matrix` from the X OpenAPI spec. The shape was designed specifically for the
  bird-as-consumer case.
- **The subprocess install dep is the remaining adoption cost.** Bird users currently must install `xr`/`xurl`
  separately (Homebrew tap or `cargo binstall`). With v2.0.0 making embedding viable, the install dep is no longer a
  forced trade-off — it's now a choice, and the choice can move.

The motive is dep removal, not implementation cleanliness. Anything that keeps the subprocess pathway as a fallback
(e.g., `BIRD_XURL_PATH` escape hatch) doesn't deliver the win — single-binary distribution requires hard-cutting the
subprocess transport.

---

## Key Decisions

- **Approach: drop the `Transport` trait, hold `ApiClient` directly.** The alternative — keeping bird's `Transport`
  trait shape and swapping the subprocess impl for an `ApiClient`-backed one — works but routes typed responses through
  a `serde_json::Value` interface, throwing away the typed `ApiResponse<T>` and structured-error wins the v2.0.0 surface
  exists to deliver. The trait was shaped by subprocess constraints (string args in, raw JSON out); the embedded surface
  is method-shaped.

- **Auth-method selection consolidates into `xurl::api::auth_matrix`.** Bird's `requirements.rs` and xurl's generated
  matrix declare the same `(method, path) → AuthScheme` mapping today; bird's table is hand-maintained and xurl's is
  generated from the X OpenAPI spec. STAR principle resolves the duplication toward xurl. Bird's `requirements.rs` fully
  dissolves rather than shrinking to a thin shim, because the things it carries (per-command `AuthType` declarations,
  `auth_flag` CLI-flag mapping) are all subprocess-shaped concerns that the embedded path doesn't need.

- **Parity flag principle: every subprocess behavior stays reachable.** Not minimum-viable (only add what we strictly
  need), not exhaustive mirroring (every xr flag becomes a bird flag). Match what subprocess users can do today via
  xr-the-binary. Streaming and media-upload xr flags are out of scope only because bird has no commands that consume
  them yet — when those commands are added in future work, their flags come with the command.

- **Stay sync now; async as fast-follow.** xurl v2.0.0 exposes a blocking public API only (`reqwest::blocking::Client`
  internally; async appears only in private streaming and OAuth2-callback wrappers). Going async requires xurl-rs to
  ship v3.0.0 with an async public surface — a real refactor with its own scope. This refactor stays sync but avoids
  introducing patterns that would obstruct the async migration: no new global mutable state, no hand-rolled
  synchronization primitives, the storage layer stays behind its current small interface so a later sqlx /
  `spawn_blocking` swap is local.

- **xurl keeps owning auth + credential storage.** OAuth1, OAuth2 PKCE, Bearer fallback, the `~/.xurl` token store, and
  the headless OAuth2 pending-state file all remain xurl's responsibility. Bird's `login` command becomes a thin wrapper
  over `xurl::auth::Auth::oauth2_flow()` / `remote_oauth2_step1/step2` rather than a subprocess passthrough; bird does
  not re-implement or duplicate any auth logic.

---

## Requirements

### Transport replacement

- R1. Bird depends on the `xurl` crate at a version that exposes the v2.0.0 ergonomic surface (`ApiClient` with no
  lifetime, shortcut methods, structured `XurlError::Api { status, body }`, `auth_matrix`).
- R2. Bird's `Transport` trait, `XurlTransport`, `MockTransport`, `xurl_call`, `xurl_passthrough`, `xurl_write_call`,
  `resolve_xurl_path`, `verify_xurl_binary`, `check_xurl_version`, and `XURL_INSTALL_HINT` are removed. No subprocess
  pathway survives, including the `BIRD_XURL_PATH` env-var escape hatch.
- R3. `BirdClient` holds an `ApiClient` (or owns construction of one) and routes every API call through it. Per-call
  options (auth scheme, username, app, no-auth) flow through `CallOptions` at the call site.
- R4. Error mapping downcasts the xurl crate's `XurlError` directly: `Api { status, body }` for HTTP errors,
  `Validation` for shape errors, `AuthMethodMismatch` for matrix violations. Bird's existing exit-code contract (78
  config, 77 auth, 1 command) is preserved — auth failures map to exit code 77 via the same `BirdError::Auth` downcast
  path, shaped against the new `XurlError` variants instead of the JSON-parsing classifier.
- R5. `bird raw` builds its request via `ApiClient::send_request` (or the equivalent typed-request entry point) rather
  than through any subprocess shim. The user-facing surface of `bird raw` (method, path with `-p` substitution, `-q`
  query pairs, body, `--pretty`) is preserved exactly.

### Auth-method consolidation

- R6. Bird's `requirements.rs` is removed in its entirety. Per-command `AuthType` declarations, the `auth_flag` mapping,
  and the `command_names_with_auth` registry no longer exist as bird state.
- R7. At each API call site, bird resolves the auth scheme by calling `xurl::api::auth_matrix::supported_auth(method,
  path)` (or `endpoint`-typed equivalent) and selects one of the supported schemes according to bird's preference order.
  The user can override the selection via the new `--auth` flag (R12).
- R8. Commands that do not hit the API (`watchlist` local ops, `cache` ops, `schema`, `completions`, `skill`, `doctor`,
  `examples`, `version`, bare `bird`) do not construct an `ApiClient` and do not consult `auth_matrix`. Lazy
  construction is the discipline: build the client only in code paths that need it.
- R9. `XurlError::AuthMethodMismatch` is a bird-side bug condition, not an expected runtime error. When it fires, bird
  surfaces it with full diagnostic detail (the endpoint, the supported schemes, the scheme bird selected) so the gap
  between bird's call site and xurl's matrix can be fixed at the source.

### CLI flag parity

- R10. `--app <name>` becomes a global bird flag (env `BIRD_APP`), wired into `Auth::with_app_name()` before `ApiClient`
  construction. Today's subprocess users who set `XURL_APP` continue to work because the env fallback is read by xurl's
  `Config`; the bird flag is the explicit surface.
- R11. `bird login` accepts the same multi-app context (`--app`) so users can run multiple OAuth2 PKCE flows against
  different app credentials.
- R12. `--auth <type>` per-call override is added (accepting the values in xurl's `AuthScheme`: `oauth2`, `oauth1`,
  `bearer`, plus `none` for bird-internal use). When provided, it overrides the auth_matrix preference selection from
  R7.
- R13. `bird raw` accepts `-H`/`--header` (repeatable) for arbitrary header injection, mirroring xr's `bird raw`
  equivalent. Headers flow through to the `ApiClient` request builder.
- R14. Bird's existing global flags retain their semantics unchanged: `--username`/`-u`, `--timeout`, `--verbose`/`-v`
  (multi-level), `--quiet`, `--no-browser`, `--output`/`--json`/`--jsonl`/`--raw`, `--color`, `--no-interactive`,
  `--refresh`/`--no-cache`/`--cache-only`, `--limit`/`--cursor`, `--examples`. The `BIRD_*` env-var bindings remain.
- R15. xr flags that bird does not adopt because they target commands bird does not have are explicitly out of scope for
  this refactor: `-s`/`--stream`, `-F`/`--file`, `--media-*`, `--max-results`, `--consumer-key` / `--consumer-secret` /
  `--access-token` / `--token-secret` / `--bearer-token`, `--oauth2-username`, `--client-id` / `--client-secret` /
  `--redirect-uri`. These come with their commands in future work.

### Diagnostics

- R16. `bird doctor` is rebuilt around the embedded library. The "is xurl installed and version-compatible" probe is
  removed (the library is linked, so the question is meaningless). The new diagnostics check auth state per configured
  app via xurl's whoami / token-store introspection, validate that `CLIENT_ID` / `CLIENT_SECRET` env vars are set where
  the configured auth flow requires them, and report the linked xurl crate version for support visibility.
- R17. `bird doctor <command>` retains its per-command availability check, now resolved against the consolidated auth
  path: it reports which `AuthScheme`s the endpoint accepts (from `auth_matrix`), which schemes the user has credentials
  for (from xurl's token store), and whether the command is reachable.

### Testing

- R18. The in-memory `MockTransport` is replaced. Tests that exercised bird logic against canned subprocess JSON
  responses move to one of two replacement patterns: (a) HTTP-level mocking via `wiremock` against a real `ApiClient`
  pointed at the mock server (matches xurl's own test pattern), or (b) a hand-rolled bird-side fake at the `ApiClient`
  method-call boundary where wiremock would be overkill. The agent picks per test based on what's actually being
  verified.
- R19. The existing contract tests for exit codes (78 / 77 / 1), CLI flag behavior, and JSON output envelopes pass
  unchanged after the refactor. Test count does not decrease. New tests cover the parity flag surface (R10, R12, R13),
  the matrix consolidation (R7), `AuthMethodMismatch` surfacing (R9), and the `bird doctor` pivot (R16, R17).

### Migration invariants

- R20. Bird's public CLI surface (subcommand names, flag names, exit codes, JSON output shapes) is preserved exactly
  except for the additions in R10-R13 and the removal of the `BIRD_XURL_PATH` env var. No user-visible breaking changes
  beyond the dependency removal itself.
- R21. The refactor ships as a single PR / single version bump. Approach B is not phaseable — dropping the `Transport`
  trait is a cutover event. The PR includes the matrix consolidation (R6-R8) and the parity flag surface (R10-R13) so
  the doctor and call-site logic are consistent at every commit on the branch.
- R22. The refactor avoids patterns that would obstruct the planned async migration: no new module-level `OnceLock` /
  `Mutex` / `RwLock` guarding shared state, no hand-rolled synchronization primitives, the storage layer's small
  interface is preserved (so a later sqlx or `spawn_blocking` swap is local), and no synchronous file or network I/O is
  introduced outside paths that already had it.

---

## Scope Boundaries

### Deferred for later

- **Full async + multi-threaded bird.** Gated on xurl-rs v3.0.0 shipping an async public API (a meaningful 1-2 week
  refactor of every shortcut
- every test in xurl-rs). The bird-side follow-up adds tokio, converts every command handler to `async`, decides between
    `spawn_blocking` and `sqlx` for the storage layer, and unlocks concurrency wins (parallel watchlist scans, parallel
    profile fetches, fan-out at rate-limit budget). This work has its own scope conversation and is not folded into this
    refactor.

- **Streaming and media-upload bird commands.** xr exposes streaming (`-s`) and multipart media upload (`-F`); bird has
  no commands that consume those today. When bird gains a streaming command or media upload command, the corresponding
  flags and code paths come with the command — not as part of the transport migration.

### Outside this refactor's identity

- **Re-implementing OAuth.** Bird does not own OAuth1, OAuth2 PKCE, Bearer, the token store, or the headless OAuth2
  pending-state file. Those are xurl's responsibility and stay there. The transport swap does not become an auth
  rewrite.
- **Changing bird's intelligence layer.** The entity store, SQLite cache, cost-tracking ledger, watchlist persistence,
  JSON output envelope, JSON Schema generation, agent-skill bundle, and reused global flags are out of scope. Anything
  that isn't transport, auth selection, or the parity flag surface stays untouched.
- **Re-litigating the previous subprocess decision.** The March 2026 subprocess migration was correct for the
  constraints at the time (no xurl library API). This refactor doesn't argue against that prior choice; it adapts to the
  new constraint (xurl now exposes an embedding-shaped library).

---

## Dependencies / Assumptions

- **xurl-rs v2.0.0 (or later, pre-v3) remains stable on the embedding surface.** `ApiClient`, `CallOptions`, the 29
  shortcut methods, `XurlError::Api { status, body }`, `XurlError::Validation`, `XurlError::AuthMethodMismatch`,
  `Auth::oauth2_flow`, `Auth::remote_oauth2_step1` / `step2`, `Auth::with_app_name`, and the `api::auth_matrix` module
  are load-bearing. The bird-xurl version contract is pinned at the patch level; xurl minor bumps that touch these
  surfaces are coordinated.
- **Cargo.lock growth is acceptable.** Embedding xurl re-introduces `reqwest` + `tokio` + `hyper` + `rustls` + their
  transitive trees. Lock-file size lands somewhere in the 240-280 crate range (up from 166 today, still below the
  303-crate pre-subprocess baseline). Bird's own API surface stays sync; tokio is transitive, not exposed.
- **xurl's `auth_matrix` accurately reflects the X API auth contract.** The matrix is generated from the vendored
  OpenAPI spec; bird trusts the spec via xurl. When the spec drifts from real X API behavior, the fix is in xurl
  (regenerate the matrix) and bird picks it up via a minor bump.
- **No existing bird users depend on the `BIRD_XURL_PATH` env var as a stable interface.** The variable was introduced
  for test isolation and developer-machine override; removing it is acceptable fallout from the transport swap.

---

## Success Criteria

- `cargo install --locked bird` (or `brew install brettdavies/tap/bird`) produces a working install with no separate
  xurl install required. A user who has never installed `xr` or `xurl` can run `bird login`, authenticate, and execute
  the full read/write command surface.
- Every bird subcommand the user could run via the subprocess transport still works after the refactor, with identical
  exit codes, JSON output shapes, and behavioral semantics for non-auth-related flows.
- The `--app`, `--auth`, and `bird raw -H` additions cover the parity gap: a user who relied on xurl's multi-app
  routing, auth-type override, or header injection has a bird-flag path to the same behavior.
- `cargo clippy --all-targets -- -D warnings` and `cargo test` pass green on a single PR. Test count does not decrease
  relative to pre-refactor baseline.
- `bird doctor` correctly reports auth state per configured app and surfaces the linked xurl crate version. Per-command
  doctor (`bird doctor <command>`) reports reachability against the consolidated matrix.

---

## Sources / Research

- `docs/solutions/architecture-patterns/xurl-subprocess-transport-layer.md` — origin of the current subprocess pattern,
  the three-phase migration it shipped, and the six bugs caught in review (exit-code preservation, semver comparison,
  etc.) whose patterns this refactor must respect.
- `docs/solutions/best-practices/rust-library-ergonomics-api-design.md` — design rationale for the v2.0.0 library
  surface (lifetime elimination, owned `Auth`, structured errors, methods on client, `from_env()`). This refactor is the
  consumer-side adoption of that ergonomics work.
- `src/transport.rs` — current subprocess transport, including the `Transport` trait at line 394, `XurlTransport` /
  `MockTransport`, `xurl_call` / `xurl_passthrough` / `xurl_write_call`, the `XurlError` classifier, and
  `resolve_xurl_path` / `verify_xurl_binary` validation logic. All of this is removed by R2.
- `src/requirements.rs` — bird's per-command auth declarations, removed by R6.
- `src/cli/commands/login.rs` — current subprocess-passthrough login, rebuilt against `xurl::auth::Auth` library methods
  per the Key Decision on xurl owning auth.
- `src/raw.rs` — `bird raw` request building, ported to `ApiClient` per R5 with the addition of header injection per
  R13.
- `src/doctor.rs` — diagnostic surface rebuilt per R16-R17.
- xurl-rs `src/api/request.rs` (`ApiClient`, `CallOptions`, `RequestTarget`, `send_request`, `send_multipart_request`),
  `src/api/shortcuts.rs` (29 typed shortcut methods), `src/api/auth_matrix.rs` (generated `(method, path) → AuthScheme`
  table and `supported_auth` lookup), `src/auth/mod.rs` (`Auth`, `oauth2_flow`, `remote_oauth2_step1` / `step2`,
  `with_app_name`), `src/error.rs` (`XurlError` variants, `exit_code_for_error`).
- xurl-rs git history: PR #21 (library ergonomics, v1.2.0), #56 (v2.0.0 client-side auth-method enforcement), #51-#52
  (multi-app credential routing).
