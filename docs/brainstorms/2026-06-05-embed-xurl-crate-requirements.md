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
`xr`/`xurl` subprocess delegation. That eliminated 1,683 lines of duplicated auth/transport code. The pattern was the
right call at the time because xurl exposed no library API — bird either re-implemented X auth or shelled out.

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
- R3. `BirdClient` holds `Mutex<ApiClient>` and routes every API call through it. `BirdClient`'s public methods take
  `&self` and acquire the lock internally — sync code has no real contention, and when v3.0.0 async lands, the
  `std::sync::Mutex` becomes `tokio::sync::Mutex` (or interior mutability moves inside `ApiClient`) as a local change.
  Per-call options (auth scheme, username, app, no-auth) flow through `CallOptions` at the call site. Bird passes
  `RequestTarget::Template { path, path_params, query }` to xurl — bird does NOT pre-substitute path templates. xurl
  substitutes path params and does the `auth_matrix` lookup atomically inside `ApiClient`. Bird's existing
  `resolve_path` helper is removed; xurl owns path substitution.
- R4. Error mapping downcasts the xurl crate's `XurlError` directly. Bird inherits xurl's `exit_code_for_error` mapping
  verbatim — whatever exit codes xurl ships now and in the future are passed through to the user — with two
  bird-specific overrides:
- `XurlError::Validation` → 78 (bird's config exit code, distinct from xurl's mapping)
- `XurlError::AuthMethodMismatch` → 77 (bird's auth exit code, with diagnostic detail per R9) — overrides xurl's
  `EXIT_AUTH_MISMATCH = 2` to avoid colliding with clap's `EX_USAGE` Inheriting xurl's mapping means bird automatically
  picks up future xurl exit codes (e.g., if xurl adds 6 for `EXIT_TIMEOUT`) without requiring a bird-side change. Codes
  78 / 77 / 1 are preserved from the pre-refactor contract. The new xurl-inherited codes (3 / 4 / 5 today, more possible
  later) are documented in CHANGELOG as user-visible behavior changes — richer agent-parseable signals, not regressions.
- R5. `bird raw` builds its request via `ApiClient::send_request` (or the equivalent typed-request entry point) rather
  than through any subprocess shim. The user-facing surface of `bird raw` (method, path with `-p` substitution, `-q`
  query pairs, body, `--pretty`) is preserved exactly.

### Auth-method consolidation

- R6. Bird's `requirements.rs` is removed in its entirety. Per-command `AuthType` declarations, the `auth_flag` mapping,
  and the `command_names_with_auth` registry no longer exist as bird state.
- R7. At each API call site, bird passes a `RequestTarget::Template { path, path_params, query }` to xurl. xurl resolves
  the auth scheme by calling `auth_matrix::supported_auth(method, path)` on the template (NOT the rendered URL — the
  matrix is keyed on the spec-verbatim template) and selects one of the supported schemes according to bird's preference
  order. Bird does NOT call `supported_auth` directly with a rendered URL; the lookup happens inside xurl's `ApiClient`
  at the same time as path substitution. The user can override the selection via the new `--auth` flag (R12).
- R8. Commands that do not hit the API (`watchlist` local ops, `cache` ops, `schema`, `completions`, `skill`,
  `examples`, `version`, bare `bird`) do not construct an `ApiClient` and do not consult `auth_matrix`. Lazy
  construction is the discipline: build the client only in code paths that need it. `bird doctor` is a non-API command
  for runtime purposes but introspects `auth_matrix` and the xurl token store per R17 — it consults the matrix without
  issuing API requests.
- R9. `XurlError::AuthMethodMismatch` is a bird-side bug condition, not an expected runtime error. When it fires, bird
  surfaces it with full diagnostic detail (the endpoint, the supported schemes, the scheme bird selected) so the gap
  between bird's call site and xurl's matrix can be fixed at the source.

### CLI flag parity

- R10. `--app <name>` becomes a global bird flag (env `BIRD_APP`), wired into `Auth::with_app_name()` before `ApiClient`
  construction. The bird flag is the explicit surface; bird does not read `XURL_APP` (that env var is xr's; bird owns
  `BIRD_APP`). Precedence: `--app` flag > `BIRD_APP` env > xurl's token-store default. **Upgrade migration safeguard:**
  when `XURL_APP` is set in the environment and `BIRD_APP` is unset, `bird` emits a one-line stderr warning at startup
  naming the situation (`XURL_APP detected but bird does not read it; set BIRD_APP or pass --app to pin the app
  selection`) so subprocess users upgrading to embedded bird don't silently route writes to a different app. Listed in
  R20 as a documented migration concern.
- R11. `bird login` accepts the same multi-app context (`--app`) so users can run multiple OAuth2 PKCE flows against
  different app credentials.
- R12. `--auth <type>` per-call override is added. The accepted value set and the `none` semantics are deferred per the
  "`--auth none` is poorly bounded" entry in Open Questions; planning picks between xurl's wire strings (`app` /
  `oauth1` / `oauth2`) for transparent xr parity, or bird-owned vocabulary (`bearer` / `oauth1` / `oauth2`) with a
  documented translation layer. When provided, the flag overrides the auth_matrix preference selection from R7.
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
  the configured auth flow requires them, and report the linked xurl crate version for support visibility. **Credential
  reporting is presence-only**, covering both env-var and token-store sources:
- `CLIENT_ID` / `CLIENT_SECRET` (env vars) → `set` / `not set`, never the value or any truncation.
- Token-store `App.client_id` / `App.client_secret` (struct fields persisted by xurl) → `set` / `not set`, never the
  value.
- Token-store `OAuth2Token.access_token` / `OAuth2Token.refresh_token` / `OAuth1Token.access_token` /
  `OAuth1Token.token_secret` / `OAuth1Token.consumer_key` / `OAuth1Token.consumer_secret` → `present` / `absent` per
  scheme + app via `has_tokens()` or equivalent non-empty check, never the value, expiry timestamp, token prefix, or
  user-id decoded from the token body.

  No credential material appears in any doctor output field, JSON envelope, or log line. R17b's reporting honors this
  rule by name (`presence-only per R16, including App-level credential fields`).
- R17a (migration). `bird doctor <command>` retains its per-command availability check, now resolved against the
  consolidated auth path via library calls instead of subprocess probes — replacing the existing subprocess-based check
  with an equivalent semantic against the embedded `ApiClient` / `auth_matrix`.
- R17b (richer per-command output). `bird doctor <command>` additionally reports which `AuthScheme`s the endpoint
  accepts (from `auth_matrix`), which schemes the user has credentials for (from xurl's token store, presence-only per
  R16), and whether the command is reachable. Both R17a and R17b ship in this refactor; the split exists so future
  readers see the migration concern (a) and the richer-output concern (b) as distinct decisions.

### Testing

- R18. The in-memory `MockTransport` is replaced by a bird-side `XurlClient` trait that wraps the xurl methods bird
  actually calls — `send_request` plus the typed shortcut methods bird uses (`users_me`, `bookmarks`, `tweets`, etc.).
  The trait grows monotonically as bird adds new commands; this is accepted as the cost of preserving xurl's typed
  shortcut surface and the structured `ApiResponse<T>` / `XurlError` wins the v2.0.0 surface delivers. Trip-point: if
  the trait exceeds ~15 methods, revisit the typed-adapter alternative (the generic `Adapter<T>` shape that doesn't grow
  per-shortcut) captured in the Open Questions section.

  Production impl is `xurl::ApiClient` (held inside `BirdClient` via `Mutex<ApiClient>` per R3); tests substitute a
  hand-rolled fake that returns canned typed responses. The trait surface uses `&self` (the lock is acquired inside
  the production impl) so command handlers stay `&self` throughout.

  Bird tests stay at the bird/xurl boundary — they verify bird's command-handler logic, error mapping, `CallOptions`
  construction, and output envelopes given a known xurl response. Bird does NOT test HTTP shapes (request bodies,
  header propagation, response parsing) — that is xurl's responsibility and is already covered by xurl-rs's own test
  suite. No `wiremock` dev-dep is introduced bird-side; no real-API integration tests are written bird-side for HTTP
  correctness.
- R19. The existing contract tests for exit codes (78 / 77 / 1), CLI flag behavior, and JSON output envelopes pass
  unchanged after the refactor. Test count does not decrease. New tests cover the parity flag surface (R10, R12, R13),
  the matrix consolidation (R7), `AuthMethodMismatch` surfacing (R9), and the `bird doctor` pivot (R16, R17).

### Migration invariants

- R20. Bird's public CLI surface (subcommand names, flag names, JSON output shapes) is preserved exactly except for the
  additions in R10-R13. Documented breaking changes versus pre-refactor bird:
- (a) `BIRD_XURL_PATH` env var is removed (it was used by bird's integration tests and documented in README/AGENTS.md as
  a user-facing override — removal is a deliberate breaking change, recorded in CHANGELOG).
- (b) Bird's exit-code surface inherits xurl's `exit_code_for_error` mapping per R4 — adds 3 (rate-limited), 4
  (not-found), 5 (network), and any future codes xurl ships. Additive signal observable in scripts that assumed only 1 /
  77 / 78.
- (c) `bird doctor` and `bird doctor <command>` JSON output shapes change per R16/R17. The `xurl_installed` /
  `xurl_version_compatible` probe fields are removed; new fields `accepted_schemes`, `credentialed_schemes`,
  `reachable`, `linked_xurl_version`, and per-app auth state are added. The agent-skill bundle and any scripts consuming
  `bird doctor --json` must update field paths. `src/schema.rs` is regenerated as part of R21 PR2's doctor batch.
  Recorded in CHANGELOG.
- (d) Subprocess users with `XURL_APP` set in their shell and `BIRD_APP` unset see a one-line stderr warning at startup
  per R10's migration safeguard. The warning is the migration aid, not a behavioral break; users who pin `BIRD_APP` (or
  pass `--app`) suppress it.
- R21. The refactor ships as a 3-PR phased sequence rather than a single atomic cutover, preserving every-commit-green
  and bisectability under bird's release process:
- **PR1** adds the xurl crate dependency and introduces `BirdClient` holding `Mutex<ApiClient>` behind a Cargo feature
  flag (the subprocess transport still runs by default). Includes the new `XurlClient` trait per R18.
- **PR2** migrates command handlers in batches under the feature flag — auth/matrix consolidation (R6-R9), call-site
  `CallOptions` plumbing + `RequestTarget::Template` adoption (R3, R7), doctor rebuild (R16-R17), `bird raw` port (R5,
  R13). The feature flag still defaults to subprocess so each PR can land green.
- **PR3** flips the feature flag default to embedded, removes `transport.rs`, `requirements.rs`, the `MockTransport`
  boundary, the `BIRD_XURL_PATH` escape hatch, and the feature flag itself. This is the cutover PR. Each PR ships its
  own changelog entry; PR3 is tagged as the version bump that ships the user-visible behavior change.

  **Implementation invariants** for the phased sequence:
- The feature flag is a Cargo feature named `embedded-xurl`. Default-off in PR1 + PR2 (subprocess runs by default);
  default-on in PR3; the feature flag itself is removed in PR3 along with the subprocess transport. Compile-time,
  invisible to end users — no runtime toggle, no breaking-change callout in R20.
- The flag selects the transport at `BirdClient` construction time ONLY. No per-call-site `cfg!()` checks. PR2 batches
  migrate **whole handlers** — handler N is entirely embedded OR entirely subprocess, never mixed within one run. This
  avoids the mid-handler transport-switch bug class where a long-running command (bookmarks paginator, watchlist scan)
  could route some calls through one transport and others through another, causing token-store contention and
  inconsistent `--app` resolution.
- PR3's pre-merge gate runs the bird test suite with the flag default-flipped (i.e., `cargo test --features
  embedded-xurl` AND `cargo test` without the feature) and asserts both pass green. Otherwise PR3 ships a regression
  invisible to PR1/PR2 CI, which ran the embedded path only when the feature flag was explicitly enabled.
- R22. The storage layer's small interface is preserved so a later `sqlx` or `spawn_blocking` swap stays local to the
  storage module. No new synchronous file or network I/O is introduced outside paths that already had it.

---

## Scope Boundaries

### Deferred for later

- **Full async + multi-threaded bird.** Gated on xurl-rs v3.0.0 shipping an async public API (a meaningful 1-2 week
  refactor of every shortcut and every test in xurl-rs). The bird-side follow-up adds tokio, converts every command
  handler to `async`, decides between `spawn_blocking` and `sqlx` for the storage layer, and unlocks concurrency wins
  (parallel watchlist scans, parallel profile fetches, fan-out at rate-limit budget). This work has its own scope
  conversation and is not folded into this refactor.

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
  transitive trees. Bird's own API surface stays sync; tokio is transitive, not exposed.
- **xurl's `auth_matrix` accurately reflects the X API auth contract.** The matrix is generated from the vendored
  OpenAPI spec; bird trusts the spec via xurl. When the spec drifts from real X API behavior, the fix is in xurl
  (regenerate the matrix) and bird picks it up via a minor bump.

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

---

## Deferred / Open Questions

### From 2026-06-05 review

- **`--auth none` is poorly bounded** — CLI flag parity — R12 (P1, security-lens, adversarial, confidence 100)

  R12 exposes `--auth none` as a user-accessible CLI flag but never bounds it. Verification of xurl-rs v2.0.0: `AuthScheme`
  has exactly three variants (`Bearer`, `OAuth1User`, `OAuth2User`) — no `None`. `supported_auth(method, path)` returns
  `Option<&[AuthScheme]>` where `None` means "endpoint unknown to the matrix, treat as permissive (request goes out, X
  arbitrates)" — that is a spec-drift fallback, not an auth-free indicator. Every X v2 API endpoint in the matrix has a
  non-empty scheme list. The intuition that "some calls don't require auth" most likely conflates bird's local commands
  (R8 list — watchlist local ops, cache, schema, completions, doctor) with API-hitting commands; local commands don't
  construct an `ApiClient` at all, so `--auth none` is meaningless for them rather than a "valid" auth choice. To revisit
  before planning: confirm whether any user-facing API command should accept `--auth none`, or restrict the value to
  bird-internal `CallOptions { no_auth: true }` and reject it at clap parse time for commands whose `auth_matrix` entry
  has a non-empty supported-scheme list.

  <!-- dedup-key: section="cli flag parity r12" title="auth none is poorly bounded" evidence="r12 auth none" -->

- **`--app` flag adds new CLI surface beyond parity** — CLI flag parity — R10/R11 (P1, scope-guardian, confidence 75)

  R10/R11 add a global `--app <name>` flag and `BIRD_APP` env var that don't exist in bird's current source. R10 itself
  states subprocess users who set `XURL_APP` continue to work transparently through xurl's Config env-fallback, so the
  multi-app use case is already covered. The parity principle is "match what subprocess users can do today" — not "add an
  explicit bird flag for every xurl env var." No evidence is offered that any current bird user reaches xr's multi-app
  routing. To revisit before planning: either defer R10/R11 to a follow-up issue (transport swap ships without `--app`)
  or provide evidence of multi-app users + acknowledge `--app` as a new feature, not a parity requirement. Deferring
  shrinks the R21 cutover surface area.

  <!-- dedup-key: section="cli flag parity r10 r11" title="app flag adds new cli surface" evidence="r10 r11 app flag" -->

- **`bird raw -H` depends on xurl-rs honoring user-supplied `Authorization`** — CLI flag parity — R13 (P1, cross-repo
  dependency, confidence 100)

  R13's `-H/--header` surface has a known upstream gap: xurl's `ApiClient::send_request` appends its own `Authorization`
  header via `RequestBuilder::header` (which is `HeaderMap::append`, not `insert`), so any user-supplied `Authorization`
  via `-H` produces a duplicate-header request with undefined HTTP semantics. The fix belongs in xurl, not bird — filed
  as xurl-rs P0 todo `012-pending-p0-honor-user-supplied-authorization-header` in xurl-rs `.context/compound-
  engineering/todos/`. To confirm at planning time: gate bird's R13 acceptance on the xurl-rs fix landing first, and
  pin the bird-side xurl dependency to a version that includes it. The earlier security-lens framing of this as an
  "auth header override" is incorrect — reqwest doesn't overwrite, it appends — so the real concern is correctness/UX,
  not access bypass.

  <!-- dedup-key: section="r13" title="raw h xurl auth dep" evidence="r13 h xurl auth append" -->

- **Typed-adapter alternative dismissed against a strawman** — Key Decisions (P2, product-lens, confidence 75)

  The first Key Decision rejects keeping a Transport trait by pointing to the current JSON-shaped trait, then concludes
  the typed surface justifies dropping the trait. A typed adapter trait — generic over response type, returning
  structured xurl errors, with `ApiClient` as the production impl — is a viable third option the doc never engages with.
  R18's MockTransport→wiremock/hand-rolled-fake rebuild cost is real and would be reduced by a typed adapter. To
  revisit at planning time: either steelman the typed-adapter shape and refute it concretely (e.g., "duplicating xurl's
  27 shortcut signatures bird-side defeats the embedding ergonomic win"), or accept it as viable and re-evaluate
  trait-drop vs. typed-adapter against the R18 test rebuild cost.

  <!-- dedup-key: section="key decisions" title="typed adapter dismissed" evidence="alt trait routes json" -->

- **v2 → v3 xurl lifecycle pin** — Dependencies / Assumptions (P2, adversarial, confidence 75)

  Bird commits to xurl v2.0.0 surface. xurl v3.0.0 will reshape `ApiClient` (the doc itself describes it as a 1-2 week
  refactor of every shortcut and test in xurl-rs). When v3.0.0 ships, bird either (a) async-migrates on the same release
  cycle, (b) maintains a v2.x branch to absorb critical xurl fixes, or (c) pins-and-stales. The Dependencies/Assumptions
  clause "xurl-rs v2.0.0 (or later, pre-v3) remains stable on the embedding surface" bakes in option (c) without naming
  it. To revisit when v3 is announced (not before): pick a path explicitly so the bird patch policy is known. Single
  maintainer mitigates coordination cost (per `[[project-solo-dev-bird-xurl]]`) but does not remove the lifecycle
  decision.

  <!-- dedup-key: section="dependencies" title="v2 v3 xurl lifecycle pin" evidence="xurl v3 reshape apiclient" -->
