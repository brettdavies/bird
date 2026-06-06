---
name: bird
binary: bird
description: Rust CLI for the X (Twitter) v2 API with entity caching, watchlist, search, thread reconstruction, OAuth2 login (interactive and headless), structured JSON output envelope, and JSON Schema documents. Triggers on X API, Twitter API, tweet, bookmark, like, follow, watchlist, X search, X bookmarks, thread reconstruction.
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
bird profile jack
bird search "rust" --output json
bird search "rust" --sort likes --min-likes 100 --pages 2
bird thread 1234567890

# Write commands — tweet, reply, like, follow, DM (each round-trips via xurl)
bird tweet "hello from bird"
bird like 1234567890
bird follow jack
bird dm jack "ping"

# Write-op guards: --dry-run prints the would-be request without sending;
# --force / --yes bypass confirmation; without either, no-TTY callers get exit 2
# (`requires-confirmation`) instead of hanging on stdin.
bird delete /2/tweets/1234567890 --dry-run
bird like 1234567890 --force

# Pagination — global --limit and --cursor on list-style commands
bird bookmarks --limit 25
bird search "rust" --cursor <token-from-previous-meta.next_cursor>

# Watchlist — track users and check recent activity without manual searches
bird watchlist add x
bird watchlist list
bird watchlist fetch         # (alias: `check`, kept for backward compatibility)
bird watchlist remove x

# Usage tracking and cache inspection — `bird usage` syncs from the API by default;
# `--local` skips the API and shows only the local cost-estimate ledger
bird usage
bird usage --local
bird cache stats --pretty
bird cache clear

# Raw API access (path templates with -p; query params with -q)
bird get /2/users/me -p id=123 -q expansions=author_id --pretty
bird post /2/tweets --body '{"text":"hi"}'

# JSON Schema documents for every output shape (success envelope + per-command)
bird schema                  # default: success-envelope schema
bird schema bookmarks
bird schema --list           # all available names
bird schema --list --output json

# Self-diagnostics, shell completions, raw xurl passthrough
bird doctor
bird doctor me
bird completions zsh
bird raw /2/users/me

# Agent-skill bundle: install bird's SKILL.md to a host's skills directory
bird skill install                          # default host: claude-code
bird skill install --dry-run
bird skill update                           # refresh installed bundle
```

Bare `bird` (no arguments) prints help and exits 2. Run `bird --examples` for the curated examples block, or `bird <sub>
--help` for per-subcommand examples.

## Global flags and environment variables

Every subcommand inherits these global flags. Each binds to a `BIRD_*` env var so non-interactive callers can configure
bird without arguments.

| Flag                                | Env var                 | Purpose                                                                          |
| ----------------------------------- | ----------------------- | -------------------------------------------------------------------------------- |
| `--output {text,json,jsonl,ndjson}` | `BIRD_OUTPUT`           | Output format. Defaults to `text` on TTY, `json` when stderr is non-TTY.         |
| `--json`                            | `BIRD_JSON=1`           | Shorthand for `--output json`.                                                   |
| `--jsonl`                           | `BIRD_JSONL=1`          | Shorthand for `--output jsonl`.                                                  |
| `--color {auto,always,never}`       | `BIRD_COLOR`            | Color mode. `NO_COLOR=1` forces `never`.                                         |
| `-q`, `--quiet`                     | `BIRD_QUIET=1`          | Suppress informational stderr; keep fatal errors.                                |
| `-v`, `--verbose`                   | `BIRD_VERBOSE`          | Repeatable: `-v` info, `-vv` debug, `-vvv` trace.                                |
| `--timeout <secs>`                  | `BIRD_TIMEOUT`          | Network timeout threaded into `xurl::api::ApiClient::with_timeout` (default 30). |
| `--no-interactive`                  | `BIRD_NO_INTERACTIVE=1` | Refuse anything that would block on stdin.                                       |
| `--raw`                             |                         | Emit pipe-safe undecorated text. Ignored in JSON modes.                          |
| `--examples`                        |                         | Print the curated examples block and exit zero.                                  |
| `--refresh`                         |                         | Bypass the entity store read; still write the response.                          |
| `--no-cache`                        |                         | Disable the entity store entirely (no read, no write).                           |
| `--cache-only`                      |                         | Serve from the local store only; never hit the API.                              |
| `--limit <N>`                       |                         | Cap for list-style commands (default 100, ceiling 1000).                         |
| `--cursor <TOKEN>`                  |                         | Pagination cursor (alias `--page`); responses surface `meta.next_cursor`.        |
| `-u`, `--username <name>`           | `X_API_USERNAME`        | Multi-user token selection threaded into the embedded request options.           |

Additional env vars without flag pairs:

- `CLIENT_ID` / `CLIENT_SECRET` — X API app credentials. Alternative to a stored app via the xurl-rs CLI (`xr auth
  app`).
- `NO_COLOR=1` — strip ANSI color (industry-standard).

## Architecture

```text
bird (CLI + entity store + intelligence) --> xurl-rs (embedded library: auth + HTTP) --> X API
```

- `src/main.rs` — thin binary entrypoint (≤30 LOC): SIGPIPE restore, tracing init, delegates to `bird::cli::run_argv()`.
- `src/lib.rs` — library root. `#[doc(hidden)]` on `bird::cli`; the library surface is internal test infrastructure
  only, not a public crate API.
- `src/cli/mod.rs` — clap derive definitions (`Cli`, `Command`, `CacheAction`, `WatchlistCommand`, `SkillAction`,
  `SchemaArgs`).
- `src/cli/runner.rs` — layered entrypoints: `run_argv()`, `run(args, &mut stdout, &mut stderr)`, `run_with_paths(args,
  &mut stdout, &mut stderr, paths, env)`. Tests call `run_with_paths` directly with TempDir-backed `ResolvedPaths` and
  `Vec<u8>` writers.
- `src/cli/dispatch.rs` — `fn run` top-level match plus shared dispatcher helpers (`command_is_api_hitting`,
  `require_confirmation`, `emit_dry_run`, `build_dry_run_url`, `clamp_limit`, `validate_auth_value`, etc.).
- `src/cli/argv.rs` — argv pre-scan helpers (`output_from_argv`, `explicit_output_from_argv`).
- `src/cli/clap_errors.rs` — clap → `BirdError` mapping for `try_parse_from`.
- `src/cli/commands/` — per-command modules: `login.rs`, `reads.rs` (Me, Get), `bookmarks.rs`, `profile.rs`,
  `search.rs`, `thread.rs`, `raw_write.rs` (Post, Put, Delete), `watchlist.rs` (Fetch only — Add/Remove/List are
  pre-dispatched), `usage.rs`, `cache.rs`, plus `writes/` (the 13 xurl-write verbs share a single `execute` helper).
- `src/xurl_client/` — bird's seam over the embedded `xurl-rs` library. `mod.rs` declares the `XurlClient` trait (one
  method per typed xurl shortcut bird uses plus a generic `send_request`), `mock.rs` is the `MockXurlClient` test
  double, `ConstructionStub` is the no-credentials fallback.
- `src/db/` — SQLite entity store: `store/` (per-entity table modules + migrations; `Connection` wrapped in
  `std::sync::Mutex` for the Send + Sync gate), `client/` (entity-aware client wrapping `XurlClient`, split into
  per-shape request modules including the `embedded` seam), `usage.rs` (per-call cost ledger).
- `src/bookmarks.rs`, `src/raw.rs`, `src/profile.rs`, `src/search.rs`, `src/thread.rs`, `src/watchlist/`, `src/usage/` —
  per-command handlers; streaming where the endpoint paginates.
- `src/doctor.rs` — diagnostic report (linked xurl crate version, per-app auth state, per-command accepted vs.
  credentialed schemes, cache health).
- `src/cli/auth_scheme.rs` — `AuthType` enum (`OAuth2User`, `OAuth1`, `Bearer`, `None`); bird's request-time scheme
  identifier rendered to xurl's wire vocabulary by `db::client::embedded::auth_type_to_xurl_wire`.
- `src/output.rs` — `OutputConfig`, color helpers, ANSI sanitization for stderr envelopes, `print_*` methods that take
  an injected `&mut dyn Write`.
- `src/error.rs` — `BirdError` enum, exit-code mapping, exhaustive `xurl::error::XurlError → BirdError` translation for
  the 77 / 78 / 1 / 3 / 4 / 5 contract.
- `src/config.rs` — `ResolvedConfig`, `ResolvedPaths`, `EnvOverrides`, file permissions (0644 config, 0600 DB; Unix-only
  `set_permissions`). `ResolvedConfig::load_with_paths(overrides, paths, env)` is the canonical injectable loader.
- `src/schema.rs` — input validation (`validate_username` strips `@`, enforces X charset).
- `src/schema_print.rs` — `bird schema` subcommand: prints embedded JSON Schema 2020-12 documents from `schema/`.
- `src/skill_install/` — `bird skill install <host>` / `update <host>` orchestrate a hardened `git clone --depth 1` of
  the [`brettdavies/bird-skill`](https://github.com/brettdavies/bird-skill) bundle repo. `skill.json` is the build-time
  manifest; `build.rs` codegens `SkillHost`, `KNOWN_HOSTS`, and `resolve_host` from it.

## Transport dependency

bird does **not** implement HTTP or OAuth itself. It embeds the [`xurl-rs`](https://github.com/brettdavies/xurl-rs)
crate as a library; every API call goes through `xurl::api::ApiClient` constructed once per CLI invocation. App
credentials come from `CLIENT_ID` / `CLIENT_SECRET` env vars or a stored app written by the optional `xr auth app`
command (xurl-rs's CLI shares the token store with the embedded client).

`bird login` calls `xurl::auth::Auth::oauth2_flow` directly; bird never owns tokens. `bird login --no-browser` (alias
`--headless`) drives `remote_oauth2_step1` / `remote_oauth2_step2` so agents and headless machines get the authorization
URL on stdout and feed the redirect URL back on stdin. Token storage, refresh, and OAuth1 signing all live in xurl-rs.

## Output formats

`OutputConfig` (`src/output.rs`) drives three formats, selected by `--output text|json|jsonl|ndjson` (default `text` on
a TTY, `json` when stderr is non-TTY) and overridable via `BIRD_OUTPUT`. `--pretty` enables formatted text output with
ANSI color and OSC-8 hyperlinks; `--color never` (or `NO_COLOR=1`) strips color; `-q` / `--quiet` suppresses
informational stderr diagnostics. `--plain` and `--no-color` survive as hidden aliases for `--color never`.

Stdout success envelope on `--output json`:

```json
{"data": ..., "meta": ...}
```

Stderr error envelope on `--output json`:

```json
{"error": "...", "kind": "config|auth|command", "exit_code": 78|77|1, "message": "...", "meta": {}}
```

`command` is present only for `kind: "command"`; `status` is present only when the upstream HTTP status is known. The
envelope's exit-code key is `exit_code` (matches the anc canonical form).

## JSON Schema documents

`bird schema` prints stable JSON Schema 2020-12 documents for every output shape: `success-envelope`, `error-envelope`,
`bookmarks`, `search`, `thread`, `profile`, `doctor`, `usage`, `watchlist`, `raw-get`. External consumers pin against
`https://bird.dev/schema/<name>-v1.json` `$id`s.

```bash
bird schema                       # default: success-envelope schema (text)
bird schema bookmarks --output json
bird schema --list                # all available names
```

## Cache modes

The entity store sits in front of every read command. Three modes:

- `--refresh` — bypass the store for the read, then write the fresh response back.
- `--no-cache` — neither read from nor write to the store.
- `--cache-only` — read from the store only; never invoke xurl. Errors with `kind: "command"` (exit_code 1) if the entry
  is missing. Write commands (`tweet`, `like`, `follow`, etc.) reject `--cache-only` with the same error.

## Exit codes

| Code | Meaning                                                                         |
| ---- | ------------------------------------------------------------------------------- |
| 0    | Success                                                                         |
| 2    | `requires-confirmation`: no-TTY caller hit a write op without `--force`/`--yes` |
| 77   | Auth error (`XurlError::Auth` detected — HTTP 401/403)                          |
| 78   | Config error (`EX_CONFIG`): missing xurl, invalid config, bad path              |
| 1    | Command error: API, network, I/O; default for everything else                   |

The 77/78 split follows the BSD `sysexits.h` convention and lets agent harnesses distinguish "fix your tokens" from "fix
your config" from "API blew up." Exit 2 lets non-interactive callers gate destructive operations explicitly instead of
hanging.

## Token and file permissions

bird does **not** store API tokens (xurl does, under its own `~/.xurl` store). bird only owns:

- `~/.config/bird/config.toml` — minimal user config (`username`, `watchlist`). Created at mode 0644.
- `~/.config/bird/bird.db` — SQLite entity store. Created at mode 0600.

Permission enforcement lives in `src/config.rs` behind `#[cfg(unix)]` (`std::os::unix::fs::PermissionsExt`).

## Agent-skill bundle

The skill bundle lives in a dedicated repo: [`brettdavies/bird-skill`](https://github.com/brettdavies/bird-skill). `bird
skill install <host>` runs a hardened `git clone --depth 1` of that repo into the host's canonical skills directory;
`bird skill update <host>` removes the destination and re-clones. The supported hosts and their target paths are the
single source of truth in `src/skill_install/skill.json`; `build.rs` codegens the `SkillHost` enum, `KNOWN_HOSTS`, and
the `resolve_host` / `host_envelope_str` lookup at build time.

| Host          | Destination                      |
| ------------- | -------------------------------- |
| `claude_code` | `~/.claude/skills/bird`          |
| `codex`       | `~/.codex/skills/bird`           |
| `cursor`      | `~/.cursor/skills/bird`          |
| `factory`     | `~/.factory/skills/bird`         |
| `kiro`        | `~/.kiro/skills/bird`            |
| `opencode`    | `~/.config/opencode/skills/bird` |

Usage:

```bash
bird skill install claude_code              # clone into ~/.claude/skills/bird
bird skill install claude_code --dry-run    # print the planned `git clone` without spawning git
bird skill install --all                    # install into every supported host
bird skill update claude_code               # remove destination + re-clone
```

Without `<host>` or `--all`, `bird skill install` returns exit 2 (`requires-confirmation`-style `missing-host` envelope)
and lists the supported hosts. Output under `--output json` is the install/update envelope (`action`, `host`,
`install_dir`, `command_preview`, `destination_status`, `status`, `exit_code`, optional `reason`).

The clone is hardened: `credential.helper=`, `core.askPass=`, `protocol.allow=never`, `protocol.https.allow=always`,
`http.followRedirects=false` are applied via `-c key=value`; `GIT_SSH`, `GIT_SSH_COMMAND`, `GIT_PROXY_COMMAND`,
`GIT_ASKPASS`, and `GIT_EXEC_PATH` are stripped from the spawn environment; and `GIT_CONFIG_GLOBAL=/dev/null`,
`GIT_CONFIG_SYSTEM=/dev/null`, `GIT_TERMINAL_PROMPT=0` block user-config rewriting and credential prompts. Mirrors the
`xurl-rs` / `agentnative-cli` pattern.

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

- Unit tests inline in each module. Exit-code coverage for the full `xurl::error::XurlError → BirdError` mapping (78 /
  77 / 1 / 3 / 4 / 5) lives in `src/error/mod.rs::tests`.
- CLI smoke tests in `tests/cli_smoke.rs` exercise the clap surface, exit codes, and JSON envelope shape via the
  in-process `run_with_paths` entry point.
- `tests/json_envelope.rs`, `tests/envelope_consistency.rs`, `tests/schema_parity.rs` lock the JSON envelope shape and
  ensure the embedded `schema/` documents stay in sync with the runtime emitters.
- `tests/doctor_schema_runtime.rs` round-trips a fixture `DoctorReport` through `serde_json::to_value` and asserts every
  runtime-emitted key matches `schema/doctor.schema.json`, plus a sentinel-string negative check that no credential
  value can leak into the JSON output.
- `tests/diag_quiet_lazy_eval_test.rs` regression-locks the R24 zero-allocation `if !quiet` diag guard via a
  `PanicOnWrite` writer backed by `ConstructionStub`.

Bird tests only the bird/xurl boundary via `MockXurlClient`; xurl-rs owns the X API HTTP shapes upstream. No bird-side
wiremock; no bird-side real-API integration suite.

## Releasing

See [`RELEASES.md`](RELEASES.md) for the operational runbook, [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) for the
pre-cut go/no-go checklist, and [`RELEASES-RATIONALE.md`](RELEASES-RATIONALE.md) for the why behind every rule. The
short version: feature branch → PR to `dev` (squash) → cherry-pick to `release/v<version>` cut from `main` → PR to
`main` (squash) → annotated tag push triggers `release.yml`.

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
- [xurl-rs](https://github.com/brettdavies/xurl-rs) — upstream transport dependency.
