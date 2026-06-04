# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-06-04

### Added

- Add project cap display to `bird usage --sync` output showing usage/cap with percentage and reset day by @brettdavies in [#28](https://github.com/brettdavies/bird/pull/28)
- Add per-app daily breakdown showing tweet counts grouped by client app ID
- `scripts/generate-changelog.sh`, `scripts/generate-changelog.py`, and `scripts/generate-completions.sh` for PR-body-aware changelog generation and shell-completion publishing during release. by @brettdavies in [#32](https://github.com/brettdavies/bird/pull/32)
- `CHANGELOG.md` `[0.1.3] - 2026-03-25` section (backfill of the released v0.1.3 notes). by @brettdavies in [#36](https://github.com/brettdavies/bird/pull/36)
- Pre-push hook steps for shellcheck, MSRV verification, rustdoc-as-warnings, and Windows cross-clippy (`x86_64-pc-windows-gnu`). by @brettdavies in [#40](https://github.com/brettdavies/bird/pull/40)
- `bird skill install` subcommand with `--host`, `--dry-run`, and `--all` flags. Default host is `claude-code`. Idempotent: re-running overwrites the destination file in place. by @brettdavies in [#41](https://github.com/brettdavies/bird/pull/41)
- `bird login --no-browser` (alias `--headless`) prints the X authorization URL to stdout and reads the redirect URL back from stdin so agents and headless machines can authenticate without launching a browser. by @brettdavies in [#42](https://github.com/brettdavies/bird/pull/42)
- Under `--output json`, `bird login --no-browser` emits `{"data":{"auth_url":"...","state":"..."},"meta":{"awaiting": "callback_url_on_stdin"}}` before reading stdin and `{"data":{"status":"authenticated"},"meta":{}}` on success.
- Global CLI flags `--output {text,json,jsonl,ndjson}`, `--json`, `--jsonl`, `--color {auto,always,never}`, `--verbose` (`-v`, repeatable), `--timeout <secs>`, `--no-interactive`, `--raw`, `--examples`. Each is also bound to a `BIRD_*` env var. All flags are `global = true` and propagate to every subcommand. by @brettdavies in [#43](https://github.com/brettdavies/bird/pull/43)
- Structured JSON error envelope under `--output json`: `{"error", "kind", "message", "exit_code", "meta"}` (with optional `command`, `status` extras). Success envelope is `{"data", "meta"}`.
- `--timeout` binds to the xurl subprocess wait so slow upstream calls fail fast under agent control.
- `bird schema` subcommand: prints the universal success-envelope schema by default; `bird schema <name>` prints a specific output schema; `bird schema --list` enumerates available names (text or JSON via `--output json`). by @brettdavies in [#44](https://github.com/brettdavies/bird/pull/44)
- Ten JSON Schema 2020-12 documents at `schema/` (success-envelope, error-envelope, bookmarks, search, thread, profile, doctor, usage, watchlist, raw-get) with stable `https://bird.dev/schema/<name>-v1.json` `$id`s for external consumers to pin against.
- Per-subcommand `Examples:` blocks in `bird <sub> --help` for every subcommand, including nested `cache clear|stats` and `watchlist check|add|remove|list`. by @brettdavies in [#45](https://github.com/brettdavies/bird/pull/45)
- Top-level `bird --help` now includes a curated `Examples:` section with a paired text + `--output json` invocation.
- `bird --examples` global flag prints the curated examples block and exits zero; under `--output json` it emits a structured envelope listing every example.
- `--force` / `--yes` / `--dry-run` on every destructive or mutating subcommand. `--dry-run` validates inputs and prints the would-be HTTP request (JSON envelope under `--output json`, single-line text otherwise) without sending it. Without `--force`/`--yes` and no TTY, bird returns `requires-confirmation` (exit 2) instead of hanging on stdin. by @brettdavies in [#46](https://github.com/brettdavies/bird/pull/46)
- Global `--limit <N>` and `--cursor <TOKEN>` (alias: `--page`) on list-style commands (`bookmarks`, `search`, `watchlist list`, `watchlist check`, `get`). Limits clamp to per-command ceilings; responses surface `meta.next_cursor` when upstream has more results so agents can paginate without re-scanning.
- `bird skill update` (aliased `upgrade`) — refresh the installed agent-skill bundle to the embedded version by @brettdavies in [#48](https://github.com/brettdavies/bird/pull/48)
- `BIRD_JSON` and `BIRD_JSONL` env vars as shortcuts for `--json` / `--jsonl`
- Shell completions for `bird schema` (top-level subcommand) by @brettdavies in [#56](https://github.com/brettdavies/bird/pull/56)
- Shell completions for `bird skill install` and `bird skill update`

### Changed

- Change `bird usage` to sync from the X API by default instead of requiring `--sync` by @brettdavies in [#29](https://github.com/brettdavies/bird/pull/29)
- Add `--local` flag to skip the API and show only local cost estimates
- Expand `AGENTS.md` to the canonical xurl-rs / agentnative-cli structure with YAML frontmatter, surface map, cache modes, output envelope, and references. by @brettdavies in [#33](https://github.com/brettdavies/bird/pull/33)
- Raise minimum supported Rust version from 1.87 to 1.94. by @brettdavies in [#35](https://github.com/brettdavies/bird/pull/35)
- Expand `Cargo.toml` package `exclude` list to keep `.context/`, `.markdownlint-cli2.yaml`, `deny.toml`, and `scripts/` out of published artifacts.
- Bring `deny.toml` to structural parity with the brettdavies framework: add `[graph]`, `[output]`, workspace-default-feature bans, and the `[sources.allow-org]` block. `CDLA-Permissive-2.0` added to the license allowlist.
- Update `docs/DEVELOPER.md` branching-workflow diagram to reference the `dev` integration branch (post-Phase-0 rename). by @brettdavies in [#39](https://github.com/brettdavies/bird/pull/39)
- `scripts/hooks/pre-push` expanded to 9 steps for framework parity with xurl-rs. by @brettdavies in [#40](https://github.com/brettdavies/bird/pull/40)
- Bird now parses arguments with `Cli::try_parse()`. Clap parse failures under `--output json` (or `--json` / `--jsonl`) emit the JSON error envelope instead of clap's plain text. by @brettdavies in [#43](https://github.com/brettdavies/bird/pull/43)
- Error envelope key renamed from `code` to `exit_code` to match the anc canonical form. Consumers depending on the old `code` key must update.
- `bird watchlist check` is now `bird watchlist fetch`. The old `check` name continues to work as a hidden alias for backward compatibility — no breaking change. by @brettdavies in [#48](https://github.com/brettdavies/bird/pull/48)
- `--pretty` help text now reads `"Pretty-print human-readable output"` uniformly across every subcommand that supports it (previously inconsistent across variants). by @brettdavies in [#52](https://github.com/brettdavies/bird/pull/52)
- `bird` cache-hit and fresh-API GET paths no longer re-serialize the response body when the caller only reads the parsed JSON. Internal optimization; no observable behavior change. by @brettdavies in [#53](https://github.com/brettdavies/bird/pull/53)
- `bird watchlist check` completion renamed to `bird watchlist fetch` (matches the binary's subcommand rename) by @brettdavies in [#56](https://github.com/brettdavies/bird/pull/56)
- Collapse `scripts/generate-changelog.{sh,py}` into a single `scripts/generate-changelog.py` (`uv run --script`). Same CLI surface and pipeline, fewer moving parts. Doc references in `RELEASES.md`, `RELEASES-RATIONALE.md`, and `.github/pull_request_template.md` updated. by @brettdavies in [#60](https://github.com/brettdavies/bird/pull/60)

### Fixed

- Fix `bird usage --sync` failing to parse actual usage data from the X API. by @brettdavies in [#27](https://github.com/brettdavies/bird/pull/27)
- Resolve a rustdoc invalid-HTML-tag warning under Rust 1.94 in `src/db/mod.rs` by wrapping `Option<BirdDb>` in backticks. by @brettdavies in [#35](https://github.com/brettdavies/bird/pull/35)
- `README.md` `cargo install` example now passes `--locked` so first-time installs use the lockfile. by @brettdavies in [#36](https://github.com/brettdavies/bird/pull/36)
- Reworded `bird login --no-browser` example lines to avoid the "headless" → "less" substring that tripped anc's pager-detection heuristic. by @brettdavies in [#47](https://github.com/brettdavies/bird/pull/47)
- `bird login` with an invalid `BIRD_XURL_PATH` now reports the specific resolution error (`BIRD_XURL_PATH=/missing does not exist`, `... is not executable`, etc.) instead of the generic "xurl not found" message. by @brettdavies in [#54](https://github.com/brettdavies/bird/pull/54)

### Documentation

- Update MSRV claim from 1.87 to 1.94. by @brettdavies in [#33](https://github.com/brettdavies/bird/pull/33)
- Refresh known-debt line counts (`main.rs` 766; `db/db.rs` 1289; `db/client.rs` 1153) and add a Token & file permissions section.
- Split the prior monolithic `RELEASES.md` into a runbook (`RELEASES.md`), a rationale companion (`RELEASES-RATIONALE.md`), and a bird-specific pre-cut checklist (`RELEASES-PREFLIGHT.md`). by @brettdavies in [#34](https://github.com/brettdavies/bird/pull/34)
- Add `.github/pull_request_template.md` with the canonical Summary / Changelog / Type of Change / Related Issues / Files Modified / Testing structure.
- Fix the README "Documentation" table entry that pointed at `RELEASING.md` to point at `RELEASES.md`.
- Refresh `AGENTS.md` for the v0.2.0 surface: global flags table, `BIRD_*` env-var bindings, JSON envelope shape (with the canonical `exit_code` key), `bird schema`, `bird skill install`/`update`, `bird login --no-browser`, write-op guards, pagination, `watchlist fetch` rename, and `usage --local` flag. Drop the stale Known Debt section and the stale modernization-sprint references. Sharpen the skill frontmatter `description` so the embedded bundle auto-discovers on X / Twitter API prompts. by @brettdavies in [#61](https://github.com/brettdavies/bird/pull/61)

### Removed

- Remove `--sync` flag from `bird usage` (use default behavior instead) by @brettdavies in [#29](https://github.com/brettdavies/bird/pull/29)

### Deprecated

- `--plain` and `--no-color` survive as hidden aliases for `--color never`; prefer `--color never` going forward. by @brettdavies in [#43](https://github.com/brettdavies/bird/pull/43)

**Full Changelog**: [v0.1.3...v0.2.0](https://github.com/brettdavies/bird/compare/v0.1.3...v0.2.0)

## [0.1.3] - 2026-03-25

### Added

- Add changelog enforcement check for PRs to main (`ci / Changelog` required status check) by @brettdavies in [#25](https://github.com/brettdavies/bird/pull/25)
- Add commit provenance guard for PRs to main — verifies non-exempt commits have PR references, auto-skipped for `release/*` branches

### Changed

- Convert guard-main-docs from inline JavaScript to centralized reusable workflow caller by @brettdavies in [#25](https://github.com/brettdavies/bird/pull/25)

### Fixed

- Drain stdout/stderr in background threads to prevent pipe-buffer deadlock in xurl transport by @brettdavies in [#24](https://github.com/brettdavies/bird/pull/24)

**Full Changelog**: [v0.1.2...v0.1.3](https://github.com/brettdavies/bird/compare/v0.1.2...v0.1.3)

## [0.1.2] - 2026-03-19

### Fixed

- Isolate config via XDG_CONFIG_HOME in CLI smoke tests (#16)
- Filter auto-changelog commits from cliff.toml (#17)
- Pass CHANGELOG_TOKEN for ruleset bypass (#19)

## [0.1.1] - 2026-03-17

### Changed

- Remove legacy OAuth config fields and cleanup
- Remove remaining legacy auth references
- Remove unused OpenAPI spec, scripts, and references
- Reflow markdown to 120-char lines and fix MD060 table alignment
- Add project-level markdownlint-cli2 config (120-char line length)
- Update RELEASING.md with release branch pattern and Trusted Publishing status

### Fixed

- Fix Trusted Publishing token wiring for crates.io publish
- Fix rustfmt drift in watchlist tests

## [0.1.0] - 2026-03-17

### Added

- Add BirdDb SQLite cache + cost estimation modules
- Wire CachedClient into all handlers
- Add bird cache clear/stats, doctor integration, login auto-clear
- Add search command with filtering, sorting, and pagination
- Add bird profile command for user lookup
- Add bird thread command for conversation reconstruction
- Add watchlist and usage commands
- Add watchlist and usage commands (#5)

### Changed

- Add dedicated SEARCH_ACCEPTED auth constant
- GA release readiness v0.1.0 (#14)

### Documentation

- Document SQLite cache layer solution
- Document search command implementation pattern
- Add profile and thread commands to CLI design doc
- Document thread/profile command patterns
- Add all plan documents for research commands series

### Fixed

- Resolve 15 code review findings across security, performance, and quality (#1)
- Resolve pre-existing clippy warnings across auth, login, output
- Address P1/P2 code review findings
- Cap --cache-ttl at 24h to prevent stale-forever entries
- Address review findings in thread command
- Correct cargo pkgid version extraction in release workflow
