---
name: bird
binary: bird
description: Rust CLI for the X (Twitter) v2 API. Adds entity caching, watchlist, search, thread reconstruction, and structured agent output on top of xurl.
homepage: https://github.com/brettdavies/bird
repository: https://github.com/brettdavies/bird
---

# bird

## Running bird

The crate is `bird`. The installed binary is `bird`. All HTTP and authentication delegate to `xr` (xurl-rs) or `xurl`
(Go fallback) at runtime.

```bash
# Read commands — current user, bookmarks, profile lookup, search, thread reconstruction
bird me
bird me --pretty
bird bookmarks --output jsonl
bird profile @jack
bird search "rust" --output json
bird search "rust" --sort likes --min-likes 100 --pages 2
bird thread 1234567890

# Write commands — tweet, reply, like, follow, DM (each round-trips via xurl)
bird tweet "hello from bird"
bird like 1234567890
bird follow @jack
bird dm @jack "ping"

# Watchlist — track users and check recent activity without manual searches
bird watchlist add @x
bird watchlist list
bird watchlist check
bird watchlist remove @x

# Usage tracking and cache inspection
bird usage --local
bird usage --sync
bird cache stats --pretty
bird cache clear

# Raw API access (path templates with -p; query params with -q)
bird get /2/users/me -p id=123 -q expansions=author_id --pretty
bird post /2/tweets --body '{"text":"hi"}'

# Self-diagnostics, shell completions, raw xurl passthrough
bird doctor
bird doctor me
bird completions zsh
bird raw /2/users/me
```

Bare `bird` (no arguments) prints help and exits 2. Environment variables: `BIRD_OUTPUT=json` forces JSON envelope
output, `NO_COLOR=1` strips ANSI color, `BIRD_XURL_PATH=/path/to/xr` overrides transport discovery, and `X_API_USERNAME`
selects which xurl-stored account to use.

## Architecture

```text
bird (CLI + entity store + intelligence) --> xr/xurl (subprocess: auth + HTTP) --> X API
```

- `src/main.rs` — thin binary entrypoint (≤30 LOC): SIGPIPE restore, tracing init, delegates to `bird::cli::run_argv()`.
- `src/lib.rs` — library root. `#[doc(hidden)]` on `bird::cli`; the library surface is internal test infrastructure only
  (per KTD-8 of the lib-lift plan), not a public crate API.
- `src/cli/mod.rs` — clap derive definitions (`Cli`, `Command`, `CacheAction`, `WatchlistCommand`, `SkillAction`).
- `src/cli/runner.rs` — layered entrypoints: `run_argv()`, `run(args, &mut stdout, &mut stderr)`, `run_with_paths(args,
  &mut stdout, &mut stderr, paths, env)`. Tests call `run_with_paths` directly with TempDir-backed `ResolvedPaths` and
  `Vec<u8>` writers.
- `src/cli/dispatch.rs` — `fn run` top-level match plus the shared dispatcher helpers (`command_needs_xurl`,
  `require_confirmation`, `emit_dry_run`, `build_dry_run_url`, `clamp_limit`, `xurl_write_call`, etc.).
- `src/cli/argv.rs` — argv pre-scan helpers (`output_from_argv`, `explicit_output_from_argv`).
- `src/cli/clap_errors.rs` — clap → `BirdError` mapping for `try_parse_from`.
- `src/cli/commands/` — per-command modules: `login.rs`, `reads.rs` (Me, Get), `bookmarks.rs`, `profile.rs`,
  `search.rs`, `thread.rs`, `raw_write.rs` (Post, Put, Delete), `watchlist.rs` (Fetch only — Add/Remove/List are
  pre-dispatched), `usage.rs`, `cache.rs`, plus `writes/` (the 13 xurl-write verbs share a single `execute` helper).
- `src/transport.rs` — xurl subprocess transport, `Transport: Send + Sync` trait, `XurlError`, `MockTransport` for unit
  tests, `OnceLock<Mutex<Option<_>>>` wrappers around `XURL_PATH` and `TIMEOUT_OVERRIDE` with
  `reset_xurl_path_for_tests()` shim for in-process test isolation.
- `src/db/` — SQLite entity store: `db.rs` (tweets / users / raw rows + migrations; `Connection` wrapped in
  `std::sync::Mutex` for the Send + Sync gate), `client.rs` (entity-aware transport client wrapping `transport.rs`),
  `usage.rs` (per-call cost ledger).
- `src/bookmarks.rs`, `src/raw.rs`, `src/profile.rs`, `src/search.rs`, `src/thread.rs`, `src/watchlist.rs`,
  `src/usage.rs` — per-command handlers; streaming where the endpoint paginates.
- `src/doctor.rs` — diagnostic report (xurl status, auth, command availability, cache health).
- `src/requirements.rs` — per-command auth requirements (`AuthType` enum: `OAuth2User`, `OAuth1`, `Bearer`, `None`).
  Single source of truth consumed by both runtime and `bird doctor`.
- `src/output.rs` — `OutputConfig`, color helpers, `diag!` macro, ANSI sanitization for stderr envelopes.
- `src/error.rs` — `BirdError` enum, exit-code mapping, XurlError-to-BirdError downcast for the 77/78 contract.
- `src/config.rs` — `ResolvedConfig`, `ResolvedPaths`, `EnvOverrides`, file permissions (0644 config, 0600 DB; Unix-only
  `set_permissions`). `ResolvedConfig::load_with_paths(overrides, paths, env)` is the canonical injectable loader;
  `ResolvedConfig::load(overrides)` is a one-line shim.
- `src/schema.rs` — input validation (`validate_username` strips `@`, enforces X charset).

## Transport dependency

bird does **not** implement HTTP or OAuth itself. Every API call shells out to `xr` (xurl-rs) or `xurl` (Go fallback).
Discovery order: `BIRD_XURL_PATH` env override → `xr` on `PATH` → `xurl` on `PATH`. Missing xurl → exit 78 with an
install hint. Minimum xurl version: 1.0.3.

`bird login` is a passthrough to xurl's interactive OAuth2-PKCE flow; bird never owns tokens. Token storage, refresh,
and OAuth1 signing all live in xurl.

## Output formats

`OutputConfig` (`src/output.rs`) drives three formats, selected by `--output text|json|jsonl` (default `text` on a TTY,
`json` when stderr is non-TTY) and overridable via `BIRD_OUTPUT`. `--pretty` enables formatted text output with ANSI
color and OSC-8 hyperlinks; `--plain` strips color and hyperlinks; `--no-color` (or `NO_COLOR=1`) strips color only;
`-q` suppresses informational stderr diagnostics.

Stderr error envelope on `--output json`:

```json
{"error":"…", "kind":"config|auth|command", "code":78|77|1, "command":"…", "status":429}
```

`command` is present only for `kind: "command"`; `status` is present only when the upstream HTTP status is known.

## Cache modes

The entity store sits in front of every read command. Three modes:

- `--refresh` — bypass the store for the read, then write the fresh response back.
- `--no-cache` — neither read from nor write to the store.
- `--cache-only` — read from the store only; never invoke xurl. Errors with `kind: "command"` (code 1) if the entry is
  missing. Write commands (`tweet`, `like`, `follow`, etc.) reject `--cache-only` with the same error.

## Exit codes

| Code | Meaning                                                            |
| ---- | ------------------------------------------------------------------ |
| 0    | Success                                                            |
| 77   | Auth error (`XurlError::Auth` detected — HTTP 401/403)             |
| 78   | Config error (`EX_CONFIG`): missing xurl, invalid config, bad path |
| 1    | Command error: API, network, I/O; default for everything else      |

Note: this differs from xurl-rs's sequential 0–5 scheme. The 77/78 split is the BSD `sysexits.h` convention and lets
agent harnesses distinguish "fix your tokens" from "fix your config" from "API blew up."

## Token and file permissions

bird does **not** store API tokens (xurl does, under its own `~/.xurl` store). bird only owns:

- `~/.config/bird/config.toml` — minimal user config (`username`, `watchlist`). Created at mode 0644.
- `~/.config/bird/bird.db` — SQLite entity store. Created at mode 0600.

Permission enforcement lives in `src/config.rs` behind `#[cfg(unix)]` (`std::os::unix::fs::PermissionsExt`).

## Quality bar

- `cargo fmt --all --check` — clean, edition 2024 (`rustfmt.toml` pins `style_edition = "2024"`).
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test --all` — full unit + CLI smoke + transport integration suites green.
- `cargo deny check` — advisories, licenses, bans, sources all clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` — clean.
- MSRV: **1.94**. The pinned toolchain (`rust-toolchain.toml`, channel `1.94.1`) is the supply-chain anchor; rustup
  verifies component SHA256s from the distribution manifest, making the pin effectively a SHA pin.
- No `unwrap()` in production code paths.

The pre-push hook at `scripts/hooks/pre-push` mirrors CI 1:1: fmt, clippy, test, deny, shellcheck, MSRV verification,
rustdoc-as-warnings, Windows cross-clippy. Set `git config core.hooksPath scripts/hooks` once per clone.

## Testing

- Unit tests inline in each module.
- CLI smoke tests in `tests/cli_smoke.rs` exercise clap surface and exit codes.
- Transport integration in `tests/transport_integration.rs` uses `MockTransport` so the suite runs without a real xurl
  binary on `PATH`.
- `tests/live_integration.rs` exercises real X API endpoints; gated `#[ignore]` so it runs only via `cargo test --test
  live_integration -- --ignored`. Requires `BIRD_XURL_PATH` pointing at a logged-in xurl install.

`BIRD_XURL_PATH` is the canonical hook for wiring tests into a real xurl binary.

## Releasing

See [`RELEASES.md`](RELEASES.md) for the operational runbook, [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) for the
pre-cut go/no-go checklist, and [`RELEASES-RATIONALE.md`](RELEASES-RATIONALE.md) for the why behind every rule. The
short version: feature branch → PR to `dev` (squash) → cherry-pick to `release/v<version>` cut from `main` → PR to
`main` (squash) → annotated tag push triggers `release.yml`.

The release-doc trio lands in PR1 of the 2026-06-01 modernization sprint; the links above resolve once that PR merges.

## Known debt

- `src/db/db.rs` and `src/db/client.rs` both exceed the 200-line refactor trigger. Split candidates: per-entity table
  modules in `db.rs`; per-shape request wrappers in `client.rs`.
- `src/db/client.rs` carries a TODO to re-serialize bodies from JSON rather than re-parsing a string, to avoid the
  round-trip cost on cache hits.
- `out_println!` / `out_print!` / `diag!` macros still write to globally-locked stdout/stderr. The runner's writer
  parameters reach the dispatcher but bypass the macros — Plan 2 (`writer-injection-remove-macros`) replaces them with
  explicit `&mut dyn Write` threading and `OutputConfig::print_*` methods.

## Documented solutions

`docs/solutions/` is a symlink to `~/dev/solutions-docs/`, a shared, searchable archive of past solutions and best
practices organized by category with YAML frontmatter (`module`, `tags`, `problem_type`). Search with `qmd query
"<topic>" --collection solutions` before implementing or debugging in a documented area; the corpus crosses repos and
already captures known pitfalls.

## References

- [`README.md`](README.md) — install paths, command surface, agent usage.
- [`RELEASES.md`](RELEASES.md) — release runbook (cut-a-release steps only).
- [`RELEASES-RATIONALE.md`](RELEASES-RATIONALE.md) — the WHY behind release rules.
- [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) — bird-specific pre-cut checklist.
- [`docs/CLI_DESIGN.md`](docs/CLI_DESIGN.md) — auth requirements, doctor, error design.
- [`docs/DEVELOPER.md`](docs/DEVELOPER.md) — build, architecture, project layout.
-

[`docs/plans/2026-06-01-repo-modernization-mirror-xurl-rs.md`](docs/plans/2026-06-01-repo-modernization-mirror-xurl-rs.md)
— the modernization sprint plan this AGENTS.md targets.

- [xurl-rs](https://github.com/brettdavies/xurl-rs) — upstream transport dependency.
