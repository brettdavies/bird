---
title: "refactor: GA release readiness (infrastructure-first v0.1.0)"
type: refactor
status: active
date: 2026-03-16
origin: docs/brainstorms/2026-03-16-ga-release-readiness-brainstorm.md
deepened: 2026-03-16
---

# refactor: GA release readiness (infrastructure-first v0.1.0)

## Enhancement Summary

**Deepened on:** 2026-03-16
**Review agents used:** architecture-strategist, security-sentinel, performance-oracle, pattern-recognition-specialist, code-simplicity-reviewer, agent-native-reviewer

### Key Improvements

1. **PR 1 hardened**: Make version check mandatory (reject unknown binaries) to prevent binary impersonation via the generic `xr` name. Canonicalize `which` result. Drop `binary_name` field (path already reveals which binary).
2. **PR 3 changed to deletion**: Delete `types/` instead of feature-gating. 10,343 lines of dead code with zero runtime references. Regeneration script + OpenAPI spec remain for future use. Measured: 14% clean-build improvement, 43% incremental improvement.
3. **PR 5 redesigned**: Replace `--plain` overloading with dedicated `--output text|json` flag + `BIRD_OUTPUT` env var, matching xurl-rs's pattern. Add `OutputConfig` struct to consolidate boolean parameter threading. Enrich error schema with HTTP `status` field for Command errors.

### New Considerations Discovered

- **Security**: `xr` is a 2-character generic name with higher impersonation/collision risk than `xurl`. Mandatory version check + canonicalize mitigates this.
- **Performance (measured)**: Removing types saves 1.1s clean build, 0.3s incremental. Binary size is identical with or without types (confirmed dead code).
- **Pattern divergence**: The plan originally claimed `--plain` "aligns with xurl-rs." It does not — xurl-rs uses a separate `--output` flag. This has been corrected.
- **`--pretty` duplication**: 11 subcommand variants duplicate the `--pretty` flag. PR 2 should note this as post-GA debt (CommonFlags extraction).
- **Tier 2 error paths**: PR 5 must cover doctor, watchlist, and username validation error paths, not just the `run()` result.

---

## Overview

Infrastructure hardening and quality improvements before the first public release of bird as v0.1.0. The current 28 commands, 201 tests, and full CI/CD pipeline are feature-sufficient. This work addresses internal debt: a 1,021-line main.rs, 368KB of unused compiled types, a binary name mismatch with xurl-rs, no machine-readable error contract, and no AI context file.

Five code PRs plus one release tag, targeting the existing v0.1.0 version.

## Problem Statement / Motivation

Bird's feature surface and distribution infrastructure are complete (see brainstorm: `docs/brainstorms/2026-03-16-ga-release-readiness-brainstorm.md`). What remains is internal quality:

- **Binary name mismatch:** bird's transport expects `xurl` (Go binary) but xurl-rs ships as `xr`, breaking the default experience for xurl-rs users
- **main.rs at 1,021 lines:** violates the project's 200-line refactor trigger; clap definitions are interleaved with dispatch logic
- **368KB unused types:** `types/generated.rs` compiles on every build but no module references it
- **No machine-readable errors:** agents and scripts cannot parse bird's error output
- **No AGENTS.md:** AI assistants lack quick architectural context

## Proposed Solution

Six PRs in sequence, each independently testable and reviewable.

## Technical Considerations

### Architecture impacts

- PR 2 (cli extraction) creates a new module boundary but no new public API
- PR 5 (JSON errors) extends `BirdError::print()` with a conditional JSON path and introduces an `OutputConfig` struct to consolidate parameter threading

### Performance implications (measured)

- PR 3 (delete types) reduces default compile time by **14% clean build (1.1s)** and **43% incremental (0.3s)** — measured via A/B testing on the actual codebase. Binary size is identical with/without types (confirmed zero runtime references).
- PR 1 adds one `which::which()` call (worst case <1ms, cached by OnceLock — one-time cost)
- PR 5 adds one `is_terminal()` check per error path (negligible — cold path only)

### Security considerations

- **PR 1 (MEDIUM risk):** `xr` is a generic 2-character name with higher impersonation/collision risk than `xurl`. Mitigated by: (1) mandatory version check as integrity gate — reject binaries that don't respond with parseable `"xurl X.Y.Z"` or `"xr X.Y.Z"`, (2) `canonicalize()` on the `which` result to resolve symlink attacks, (3) `BIRD_XURL_PATH` override remains the escape hatch. See security review for details.
- **PR 5 (LOW risk):** JSON errors contain the same data already printed to stderr. Apply `sanitize_for_stderr()` before JSON serialization to prevent JSON structure injection from malicious API responses.

## System-Wide Impact

- **Interaction graph:** PR 1 affects `resolve_xurl_path()` -> `check_xurl_version()` -> `build_xurl_status()` chain. PR 5 affects all error printing sites in `main()` (4 sites: username validation, config load, xurl gate, `run()` result) plus the Tier 2 doctor and watchlist error paths.
- **Error propagation:** JSON errors are a presentation change; `BirdError` enum and exit codes (78/77/1) are unchanged.
- **State lifecycle risks:** None. All changes are stateless transformations.
- **API surface parity:** New `--output text|json` flag with `BIRD_OUTPUT` env var, matching xurl-rs's `--output` + `XURL_OUTPUT` pattern.
- **Integration test scenarios:** (1) `bird me` finds `xr` when only xurl-rs is installed; (2) `BIRD_OUTPUT=json bird me` emits JSON error on auth failure; (3) `cargo build` default excludes generated types.

## Implementation Phases

### PR 1: Binary name resolution (`xr` + `xurl` support)

**Branch:** `refactor/xurl-rs-binary-resolution`

**Files modified:**

- `src/transport.rs` — `resolve_xurl_path()` (line 43), `XURL_INSTALL_HINT` (line 36), `check_xurl_version()` (line 79)
- `src/doctor.rs` — `build_xurl_status()`, pretty formatter
- `tests/transport_integration.rs` — update/add tests for dual-binary resolution
- `README.md` — update xurl install instructions

**Changes:**

1. Update `resolve_xurl_path()` to try `which::which("xr").or_else(|_| which::which("xurl"))` (line 70)
2. **Canonicalize the `which` result** — apply `canonicalize()` to the resolved path, matching the rigor of the `BIRD_XURL_PATH` code path (security hardening)
3. **Make version check mandatory** — if `check_xurl_version()` cannot parse a valid version string (with either `"xurl "` or `"xr "` prefix), reject the binary and fall back to the next candidate. This turns the version check into an integrity gate against binary impersonation.
4. Update `XURL_INSTALL_HINT` to reference `brettdavies/tap/xurl-rs` (primary) and `xdevplatform/tap/xurl` (alternative)
5. Make `check_xurl_version()` version prefix parsing defensive: `.strip_prefix("xurl ").or_else(|| s.strip_prefix("xr "))` (line 94)
6. Minimum version remains `>= 1.0.3` for either binary (single `MIN_VERSION` constant)

**What was dropped from the original plan (simplification):**

- `binary_name: String` field on `XurlStatus` — the resolved `path` already reveals which binary was found. Adding a dedicated field is over-engineering for v0.1.0.

**Acceptance criteria:**

- [ ] `bird me` works when only `xr` is on PATH
- [ ] `bird me` works when only `xurl` is on PATH
- [ ] When both are present, `xr` is preferred
- [ ] `BIRD_XURL_PATH` overrides all auto-detection
- [ ] Binary that fails version check is rejected (falls through to next candidate)
- [ ] Resolved path is canonicalized (symlink attacks mitigated)
- [ ] Version parsing handles both `"xurl X.Y.Z"` and `"xr X.Y.Z"` prefixes
- [ ] Install hint references both xurl-rs and Go xurl
- [ ] All existing transport integration tests pass
- [ ] README xurl install instructions updated

#### Research Insights

**Security (security-sentinel):**

- `xr` is a generic 2-character name. An unrelated binary named `xr` on PATH (e.g., X11 utility, custom script) would be silently executed with the user's privileges. The mandatory version check converts a medium-severity impersonation risk into a low-severity one.
- The existing `BIRD_XURL_PATH` path applies `canonicalize()`, `is_file()`, and executable permission checks. The `which::which()` path had none of these. Adding `canonicalize()` closes this gap.

**Performance (performance-oracle):**

- `which::which()` for a nonexistent binary (full PATH scan) completes in <1ms. The `OnceLock` caching means this is a one-time cost. No performance concern.

---

### PR 2: Extract clap definitions to cli.rs

**Branch:** `refactor/extract-clap-cli-module`

**Files modified:**

- `src/main.rs` — remove clap definitions, add `mod cli; use cli::*;`
- `src/cli.rs` — new file with extracted types

**Changes:**

1. Move these types from main.rs to `src/cli.rs` (lines 95-405):
   - `struct Cli` (lines 95-134)
   - `enum Command` (lines 136-376)
   - `enum CacheAction` (lines 378-387)
   - `enum WatchlistCommand` (lines 389-405)
2. All extracted types get `pub(crate)` visibility
3. `cli.rs` imports `clap_complete::Shell` for the `Completions` variant
4. main.rs adds `mod cli;` and `use cli::{Cli, Command, CacheAction, WatchlistCommand};`
5. `BirdError`, `map_cmd_error()`, `run()`, `main()`, and helpers stay in main.rs
6. `default_auth_type()` stays in main.rs (it is used by `run()`, not by clap definitions)

**What does NOT move:**

- `BirdError` enum (not a clap type; stays in main.rs)
- `parse_param_vec()`, `use_color_from_cli()` (helpers used by `run()`)
- `xurl_write_call()`, `xurl_write()` (command dispatch helpers)
- Tests in main.rs (they test `BirdError` and `map_cmd_error`, not clap types)

**Acceptance criteria:**

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all 201 tests pass with zero changes to test code
- [ ] `cargo clippy --all-targets` clean
- [ ] main.rs drops from ~1,021 to ~710 lines
- [ ] cli.rs is ~310 lines of pure clap derive definitions
- [ ] No public visibility leakage (all types are `pub(crate)`)
- [ ] Three-tier gating comments remain in main.rs with dispatch logic

#### Research Insights

**Architecture (architecture-strategist):**

- Import chain is clean: cli.rs imports `clap_complete::Shell` and nothing from main.rs. No circular dependency risk.
- The `pub(crate)` visibility is correct — these types are used only by `main.rs`.
- main.rs will still be ~710 lines (the `run()` function alone is ~358 lines). Future PR could extract write-command dispatch (13 arms that all follow the identical `xurl_write()` pattern, lines 646-732) into a `write.rs` module.

**Pattern (pattern-recognition-specialist):**

- xurl-rs uses a directory module (`src/cli/` with `mod.rs` + `commands/mod.rs`). Bird's flat `cli.rs` is appropriate for its smaller size.
- **Post-GA debt noted:** `--pretty` is duplicated across 11 subcommand variants. A `CommonFlags` struct (matching xurl-rs's pattern) would consolidate these. This should be tracked as a separate refactoring PR, not bloating this extraction.

---

### PR 3: Delete unused generated types

**Branch:** `refactor/remove-unused-generated-types`

> **Changed from original plan:** Feature-gating was replaced with deletion based on YAGNI analysis. The types module has zero runtime references (confirmed by grep and binary size comparison). The `scripts/generate-types.sh` script and `openapi/x-api-openapi.json` remain in the repo — regeneration is a single command if typed API work ever starts.

**Files modified:**

- `src/main.rs` — remove `mod types;` declaration (line 17)

**Files deleted:**

- `src/types/mod.rs` (4 lines)
- `src/types/generated.rs` (10,343 lines / 368KB)

**Changes:**

1. Remove `mod types;` from main.rs (line 17)
2. Delete `src/types/mod.rs` and `src/types/generated.rs`
3. `scripts/generate-types.sh` and `openapi/x-api-openapi.json` remain unchanged
4. No changes to Cargo.toml (no `[features]` section needed)
5. No changes to CI workflows

**Acceptance criteria:**

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all tests pass (none reference types module)
- [ ] `cargo clippy --all-targets` clean
- [ ] `cargo publish --dry-run` passes
- [ ] `scripts/generate-types.sh` still exists for future regeneration
- [ ] `openapi/x-api-openapi.json` still exists as source material

#### Research Insights

**Simplicity (code-simplicity-reviewer):**

- Feature-gating dead code is more complex than deleting it. A feature flag adds: Cargo.toml section, conditional compilation, CI job, and cognitive overhead for every future contributor.
- The generation script + OpenAPI spec remain in the repo. Regeneration is one command. YAGNI principle applies.

**Performance (performance-oracle, measured):**

- **14% clean build improvement** (7.9s -> 6.8s, saving 1.1s)
- **43% incremental rebuild improvement** (0.69s -> 0.39s, saving 0.3s)
- **Binary size unchanged** in both debug and release (confirms zero runtime references)
- The cost comes from 132 structs with 119 `#[derive(Serialize, Deserialize)]` invocations, each triggering serde proc-macro expansion.

---

### PR 4: AGENTS.md

**Branch:** `docs/agents-md`

**Files created:**

- `AGENTS.md` — top-level AI context file

**Content sections** (following xurl-rs pattern at `~/dev/xurl-rs/AGENTS.md`):

1. **Project description** — bird is a Rust CLI for X/Twitter API, wrapping xurl for transport
2. **Binary & Package** — binary: `bird`, package: `bird`, crate: `bird`
3. **Architecture** — `bird (CLI + intelligence) -> xurl (subprocess: auth + HTTP) -> X API`
4. **Transport dependency** — requires `xr` (xurl-rs) or `xurl` (Go) on PATH; override with `BIRD_XURL_PATH`
5. **Quality bar** — clippy clean, rustfmt (edition 2024), MSRV 1.87, no `unwrap()` in production, `cargo-deny` for advisories/licenses
6. **Exit codes** — 0=success, 1=command error, 77=auth error, 78=config error (note: differs from xurl-rs's sequential 0-5 scheme)
7. **Key modules** — main.rs (dispatch), cli.rs (clap definitions), transport.rs (xurl subprocess), db/ (SQLite cache), doctor.rs (diagnostics), requirements.rs (auth source of truth), output.rs (formatting + `diag!` macro)
8. **Known debt** — main.rs ~710 lines post-extraction, db/db.rs (1,289 lines), db/client.rs (1,141 lines), `--pretty` duplication across 11 variants, TODO at db/client.rs:38
9. **Testing** — `MockTransport` for unit tests, `BIRD_XURL_PATH` for integration tests, `#[ignore]` live tests
10. **Cross-references** — DEVELOPER.md, CLI_DESIGN.md, docs/solutions/

**Acceptance criteria:**

- [ ] AGENTS.md exists at repo root
- [ ] Content is 50-80 lines
- [ ] Cross-references to DEVELOPER.md and CLI_DESIGN.md are valid relative paths
- [ ] Documents exit code divergence from xurl-rs
- [ ] Lists known debt items
- [ ] Passes markdownlint

#### Research Insights

**Pattern (pattern-recognition-specialist):**

- xurl-rs AGENTS.md is 31 lines. Bird's will be larger (50-80) due to the xurl transport dependency documentation, exit code contract, and known debt section.
- Should document the exit code divergence: bird uses sysexits-style (78/77/1) vs xurl-rs sequential (0-5). Both are valid but surprising for users of both tools.

---

### PR 5: Structured JSON error output

**Branch:** `feat/json-error-output`

> **Redesigned based on review feedback:** Uses dedicated `--output text|json` flag with `BIRD_OUTPUT` env var instead of overloading `--plain`. Introduces `OutputConfig` struct. Enriches error schema with HTTP `status` for Command errors.

**Files modified:**

- `src/main.rs` — `BirdError::print()`, `main()` for output format detection, error path threading
- `src/output.rs` — new `OutputConfig` struct, JSON error formatting helper
- `src/cli.rs` — add `--output` global flag with `BIRD_OUTPUT` env var
- `tests/cli_smoke.rs` — add JSON error contract tests
- `README.md` — document error contract

**Design decisions** (resolving review feedback):

| Decision | Resolution | Rationale |
|----------|------------|-----------|
| Trigger mechanism | `--output json` flag + `BIRD_OUTPUT` env var | Do NOT overload `--plain`. xurl-rs uses a separate `--output` flag. `--plain` remains color-only. |
| Non-TTY default | Auto-detect: default to `json` when stderr is non-TTY | Agents piping stderr get JSON automatically |
| `--plain` semantics | Unchanged (no color, no hyperlinks only) | Preserve existing behavior; no surprises for current users |
| `"error"` field content | Underlying message only (no prefix) | `kind` already encodes category |
| `Command` variant schema | Include `"command"` and optional `"status"` fields | Machine consumers need the command name and HTTP status |
| Clap errors (exit 2) | Out of scope | Clap errors use clap's native format |
| `diag!` behavior in JSON mode | Suppressed (implicit quiet) | JSON consumers cannot parse interleaved text diagnostics |

**JSON error schema:**

```json
{"error": "<message>", "kind": "config", "code": 78}
{"error": "<message>", "kind": "auth", "code": 77}
{"error": "<message>", "kind": "command", "command": "<name>", "code": 1}
{"error": "<message>", "kind": "command", "command": "<name>", "status": 429, "code": 1}
```

The `"status"` field is present only when the underlying error is an `XurlError::Api { status, .. }`.

**Changes:**

1. Add `--output text|json` global flag to `Cli` struct in cli.rs with `BIRD_OUTPUT` env var. Default: `text` when stderr is TTY, `json` when not.
2. Create `OutputConfig` struct in `output.rs` consolidating `use_color: bool`, `quiet: bool`, and `output_format: OutputFormat` (replacing separate boolean parameters)
3. Thread `OutputConfig` through `run()` and command handlers (replaces current `use_color` + `quiet` booleans)
4. In `BirdError::print()`, when `output_format == Json`, emit the JSON envelope to stderr via `eprintln!`. Apply `sanitize_for_stderr()` to the error message before JSON serialization.
5. When `output_format == Json`, implicitly suppress `diag!` output (same as `--quiet`)
6. Add contract tests pinning the exact JSON structure for all three `BirdError` variants
7. Document the JSON error contract in README under "Agent / non-interactive usage"

**Error paths that must handle JSON output** (all sites in `main()` that call `BirdError::print()`):

- Username validation error (line 854)
- Config load error (lines 884-886)
- Doctor error path (lines 912-916)
- Watchlist local command error path (lines 922-944)
- Xurl gate error (lines 949-951)
- `run()` result error (lines 963-965)

**Acceptance criteria:**

- [ ] `bird --output json me` (with no xurl) emits JSON error to stderr
- [ ] `BIRD_OUTPUT=json bird me` emits JSON error to stderr
- [ ] `bird me 2>/tmp/err.json` (stderr non-TTY) defaults to JSON errors
- [ ] Interactive TTY defaults to human-readable text errors
- [ ] `--plain` alone does NOT change error format (only suppresses color)
- [ ] `--output json` implicitly suppresses `diag!` output
- [ ] JSON schema matches spec for all three BirdError variants
- [ ] `"status"` field present only for API errors with HTTP status
- [ ] Error messages pass through `sanitize_for_stderr()` before serialization
- [ ] Contract tests pin exact JSON field names and types
- [ ] Exit codes unchanged (78, 77, 1)
- [ ] README documents the JSON error contract and `BIRD_OUTPUT` env var
- [ ] Clap errors (exit 2) remain in clap's native format

#### Research Insights

**Agent-native (agent-native-reviewer):**

- Bird's success path is already fully agent-native (JSON stdout, structured exit codes, quiet mode). The error path is the gap.
- `--output json` should implicitly suppress diagnostics. JSON consumers cannot parse interleaved `[cost]` and `[store]` text lines on stderr.
- The `OutputConfig` struct approach (from xurl-rs) prevents the boolean parameter threading from getting worse. Currently `run()` has 6 parameters including `use_color` and `quiet`. Adding another boolean would be unwieldy.
- Consider `retryable: bool` hint in a future iteration — saves every agent from reimplementing retry logic based on status codes.

**Security (security-sentinel):**

- JSON errors contain the same data already printed to stderr. No new information exposure.
- Apply `sanitize_for_stderr()` before JSON serialization to prevent a malicious API response from injecting JSON metacharacters that could break the outer JSON structure.

**Pattern (pattern-recognition-specialist):**

- The original plan claimed `--plain` "aligns with xurl-rs." It does not — xurl-rs uses `--output text|json|jsonl` as a separate flag with `XURL_OUTPUT` env var. The redesigned approach genuinely aligns.
- Bird's `--output` can start with just `text|json` (no `jsonl` needed yet).

**Simplicity (code-simplicity-reviewer counter-argument, noted):**

- The simplicity reviewer argued to defer PR 5 entirely (no known machine consumers, exit codes suffice). This is a valid YAGNI argument. If the implementer agrees, PR 5 can be deferred to post-GA without affecting the release. The remaining 4 PRs + release are independently complete.

---

### PR 6: Tag and release v0.1.0

**Branch:** N/A (release from main)

**Pre-release checklist:**

- [ ] PRs 1-5 (or 1-4 if PR 5 deferred) merged to `development` via squash merge
- [ ] `development` squash-merged to `main`
- [ ] `cargo publish --dry-run` passes on main
- [ ] `cargo package --list` excludes sensitive files (docs/SECRETS.md, etc.)
- [ ] `cargo deny check` clean
- [ ] Full test suite passes on main
- [ ] `CARGO_REGISTRY_TOKEN` repository secret configured
- [ ] `HOMEBREW_TAP_TOKEN` repository secret configured and not expired
- [ ] The single TODO at `db/client.rs:38` is documented as accepted debt (in AGENTS.md)

**Release steps:**

```bash
git checkout main && git pull
git tag v0.1.0
git push origin main --tags
```

CI pipeline: `check-version + audit -> build (5 targets) -> publish-crate -> release -> homebrew`

**Post-release validation:**

- [ ] crates.io shows `bird@0.1.0`
- [ ] GitHub Release has 5 binary archives + checksums
- [ ] `brew install brettdavies/tap/bird` installs successfully
- [ ] `cargo install bird` installs successfully
- [ ] Consider configuring crates.io Trusted Publishing (OIDC) after first publish

#### Research Insights

**Risk (architecture-strategist):**

- `cargo publish` is irreversible. If a bug is found post-publish, the only option is yank + publish 0.1.1.
- The `Cargo.toml` already has `version = "0.1.0"` — no version bump needed. The `check-version` CI job will verify tag matches.

**Pre-existing debt (security-sentinel):**

- `FileConfig` in `config.rs` has legacy `client_id`/`client_secret` fields (lines 23-25) that lack Debug redaction. These are unused but could appear in debug logs. Consider adding `#[serde(skip)]` or a custom Debug impl as a follow-up.

## Acceptance Criteria

- [ ] `xr` binary resolution works alongside `xurl` with security hardening (PR 1)
- [ ] main.rs clap definitions extracted to cli.rs (PR 2)
- [ ] `types/generated.rs` deleted; generation script retained (PR 3)
- [ ] AGENTS.md exists at repo root with known debt documented (PR 4)
- [ ] JSON errors emitted via `--output json` / `BIRD_OUTPUT` (PR 5, or deferred)
- [ ] v0.1.0 tagged and released to crates.io, GitHub, and Homebrew (PR 6)

## Dependencies & Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Binary impersonation via generic `xr` name | Medium | Mandatory version check as integrity gate; canonicalize resolved path |
| `cargo publish` is irreversible | High | `cargo publish --dry-run` on every PR; full test suite on tagged commit |
| JSON error schema becomes public contract | High | Contract tests pin exact structure; documented at release. OR defer PR 5 to post-GA. |
| PR 2 + PR 3 both modify main.rs | Low | Linear merge order (PR 2 first) avoids conflicts |
| `CARGO_REGISTRY_TOKEN` not configured | High | Verify secret exists before tagging |
| `xr` version prefix change in future xurl-rs | Low | Defensive parsing handles both prefixes; mandatory version check catches unknown formats |
| PR 5 `OutputConfig` refactor scope | Medium | `OutputConfig` replaces existing booleans — net complexity is low, but touches many signatures |

## PR Dependency Graph

```text
PR 1 (Binary resolution) ---- independent
PR 2 (CLI extract)       ---- independent
PR 3 (Delete types)      ---- depends on PR 2 (both modify main.rs line 17)
PR 4 (AGENTS.md)         ---- independent (but content references PR 2's cli.rs)

PR 5 (JSON errors)       ---- depends on PR 2 (OutputConfig replaces use_color/quiet threading)

PR 6 (Tag + Release)     ---- depends on ALL of PRs 1-5 (or 1-4 if PR 5 deferred)
```

PRs 1, 2, and 4 can be developed in parallel. PR 3 merges after PR 2. PR 5 merges after PR 2. PR 6 is the final gate.

## Post-GA Debt (tracked in AGENTS.md)

- main.rs still ~710 lines after PR 2 — extract write-command dispatch to `write.rs`
- db/db.rs (1,289 lines) and db/client.rs (1,141 lines) exceed 200-line trigger
- `--pretty` duplicated across 11 subcommand variants — extract `CommonFlags` struct
- TODO at db/client.rs:38 — body re-serialization from JSON
- FileConfig legacy `client_id`/`client_secret` fields lack Debug redaction
- `--dry-run` flag for write commands
- Man page generation (clap_mangen)
- crates.io Trusted Publishing migration
- Expose additional xurl-rs commands: timeline, mentions, followers/following, media upload

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-16-ga-release-readiness-brainstorm.md](docs/brainstorms/2026-03-16-ga-release-readiness-brainstorm.md) — Key decisions: infrastructure-first scope, `xr`+`xurl` dual resolution, clap-only extraction, ship as v0.1.0

### Internal References

- Transport layer pattern: `docs/solutions/architecture-patterns/xurl-subprocess-transport-layer.md`
- main.rs three-tier gating: `docs/solutions/architecture-patterns/shell-completions-main-dependency-gating.md`
- Exit code contracts: `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md`
- Release pipeline: `docs/solutions/architecture-patterns/release-pipeline-cross-platform-publish.md`
- crates.io readiness: `docs/solutions/architecture-patterns/crates-io-distribution-readiness.md`
- Quiet flag pattern: `docs/solutions/architecture-patterns/quiet-flag-diagnostic-suppression-pattern.md`

### External References

- xurl-rs AGENTS.md: `~/dev/xurl-rs/AGENTS.md` (reference pattern, 31 lines)
- xurl-rs JSON error format: `~/dev/xurl-rs/src/output.rs:85-105` (alignment target)
- xurl-rs OutputConfig: `~/dev/xurl-rs/src/output.rs:21` (struct pattern)
- xurl-rs --output flag: `~/dev/xurl-rs/src/cli/mod.rs:94-102` (flag definition with `XURL_OUTPUT` env var)

### Key Implementation Files

- `src/transport.rs:36-76` — `XURL_INSTALL_HINT`, `resolve_xurl_path()`, `check_xurl_version()`
- `src/main.rs:29-64` — `BirdError` enum, `print()`
- `src/main.rs:95-405` — clap definitions to extract
- `src/main.rs:17` — `mod types;` declaration to remove
- `src/main.rs:445-810` — `run()` dispatch, parameter threading to refactor
- `src/doctor.rs:50-66` — `XurlStatus`, `build_xurl_status()`
- `src/output.rs` — color helpers, `diag!` macro, `sanitize_for_stderr()`
- `src/types/mod.rs` — types module entry (to delete)
- `Cargo.toml` — version 0.1.0 already set
