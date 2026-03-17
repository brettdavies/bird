# GA Release Readiness: Infrastructure-First v0.1.0

**Date:** 2026-03-16
**Status:** Completed
**Scope:** Infrastructure hardening, refactoring, and quality improvements before first public release (v0.1.0)
**Alignment reference:** `~/dev/xurl-rs/` patterns and conventions

## What We're Building

Infrastructure and quality improvements to make bird ready for its first public release as v0.1.0. The current 28 commands and 201 tests are feature-sufficient — this work focuses on internal quality, developer experience, and machine-consumability before stamping a stable release.

### Work Items

1. **Binary name resolution** — Support both `xr` (xurl-rs) and `xurl` (Go original) in bird's transport layer. Check for `xr` first, fall back to `xurl`. Update install hints to reference both.

2. **Extract clap definitions from main.rs** — Move ~400 lines of clap derive structs/enums to a dedicated `cli.rs` module. Leave command dispatch in main.rs. Reduces main.rs from 1,021 lines toward the 200-line target.

3. **Feature-gate generated types** — Move `types/generated.rs` (368KB) behind `#[cfg(feature = "typed-api")]` so it doesn't compile by default. Preserves the code for future typed API work without the compile-time cost.

4. **AGENTS.md** — Add an architectural summary file for AI-assisted development, following xurl-rs's pattern. Documents binary/library naming, quality bar, architecture overview, and key conventions.

5. **Structured JSON error output** — Adopt xurl-rs's pattern of emitting errors as `{"error": "...", "kind": "auth|config|command", "code": 77}` when `--plain` is active or stdout is not a TTY. Enables machine consumers and agent workflows.

6. **Version: ship as v0.1.0** — Keep current version number. Infrastructure work is internal refactoring that doesn't warrant a version bump. Tag and release what we have.

## Why This Approach

Bird's feature surface is complete for a first release — 28 commands spanning auth, read, write, watchlist, usage tracking, diagnostics, and cache management. The distribution infrastructure (5-target CI, crates.io, Homebrew, shell completions, quiet flag) was completed in the previous 4-phase implementation cycle.

What's missing is internal quality:

- **main.rs at 1,021 lines** violates the project's own 200-line refactor trigger
- **368KB of unused generated types** compile on every build
- **Binary name mismatch** between bird (expects `xurl`) and xurl-rs (ships `xr`) creates a broken default experience for users who install xurl-rs
- **No machine-readable error contract** limits agent/automation consumers
- **No AGENTS.md** means AI assistants lack quick architectural context

These are all infrastructure concerns, not feature gaps. Fixing them before the first public release prevents accumulating debt that becomes harder to address once users depend on the current behavior.

## Key Decisions

### Binary resolution: `xr` first, `xurl` fallback

- Update `resolve_xurl_path()` in transport.rs to check `xr` first, then `xurl`
- Update `XURL_INSTALL_HINT` to reference `brettdavies/tap/xurl-rs` (primary) and `xdevplatform/tap/xurl` (alternative)
- `BIRD_XURL_PATH` env var continues to override all auto-detection
- `bird doctor` reports which binary was found and its version
- Minimum version requirement remains >= 1.0.3 for either binary

### main.rs refactor: extract clap definitions only

- Move `Command` enum, all subcommand structs, `GlobalFlags`, and clap derive macros to `src/cli.rs`
- Keep command dispatch (`run_command()` / `main()`) in main.rs
- This is the highest-payoff, lowest-risk decomposition — clap definitions are pure data structures with no runtime behavior
- Target: main.rs drops from ~1,021 to ~600 lines; cli.rs is ~400 lines
- Further decomposition (dispatch.rs, app.rs) deferred to post-GA

### Generated types: feature-gated, not removed

- Wrap `types/generated.rs` and `types/mod.rs` behind `#[cfg(feature = "typed-api")]`
- Default features do not include `typed-api`
- Preserves the OpenAPI-derived types for future work without compile-time cost
- `scripts/generate-types.sh` remains in the repo

### AGENTS.md: follow xurl-rs pattern

- Top-level `AGENTS.md` file with: binary name, package name, architecture overview, quality bar, key file paths, and cross-references to DEVELOPER.md and CLI_DESIGN.md
- Concise — target 50-80 lines

### Structured JSON error output

- When `--plain` is active or stdout is not a TTY, emit errors to stderr as JSON: `{"error": "message", "kind": "auth|config|command", "code": 77}`
- Aligns with xurl-rs's `{"error": "...", "kind": "...", "code": N}` pattern
- Human-readable error format remains the default for interactive use
- Exit codes unchanged (78=config, 77=auth, 1=command)

### Version: v0.1.0

- Ship current version as first public release
- Signals "usable and tested but CLI surface may evolve"
- No semver stability commitment yet — flags, command names, output formats may change in 0.x releases
- Path to 1.0.0 after real-world usage validates the CLI contract

## Resolved Questions

- **GA scope?** — Infrastructure-first. Current features are sufficient; invest in quality before release.
- **Binary name strategy?** — Support both `xr` and `xurl`, prefer `xr`.
- **main.rs refactor depth?** — Extract clap definitions only. Moderate effort, biggest win.
- **Generated types?** — Feature-gate behind `typed-api`, don't remove.
- **Test coverage?** — Current 201 tests are sufficient. No additional test work for GA.
- **xurl-rs patterns to adopt?** — AGENTS.md and structured JSON error output.
- **Version number?** — Ship as v0.1.0. Infrastructure work is internal.

## Open Questions

None — all key decisions resolved.

## Implementation Order

Suggested PR sequence (each builds on the previous):

### PR 1: Binary name resolution (`xr` + `xurl` support)

- Update `resolve_xurl_path()` to check `xr` first, then `xurl`
- Update `XURL_INSTALL_HINT` and install guidance
- Update `bird doctor` output to show which binary was found
- Update tests that mock xurl path resolution

### PR 2: Extract clap definitions to cli.rs

- Move clap derive structs/enums from main.rs to src/cli.rs
- Re-export from cli.rs as needed
- No behavior changes — pure structural refactor

### PR 3: Feature-gate generated types

- Add `typed-api` feature to Cargo.toml
- Gate `types/` module behind `#[cfg(feature = "typed-api")]`
- Verify default build no longer compiles generated types

### PR 4: AGENTS.md

- Create top-level AGENTS.md following xurl-rs pattern
- Cross-reference DEVELOPER.md and CLI_DESIGN.md

### PR 5: Structured JSON error output

- Extend `BirdError::print()` to emit JSON when `--plain` or non-TTY
- Add tests for JSON error format
- Document error contract in README

### PR 6: Tag and release v0.1.0

- Verify all PRs merged to development, then to main
- Tag `v0.1.0`, push — CI handles the rest
- Validate: crates.io publish, GitHub Release (5 targets), Homebrew formula update

## Future Work (post-GA)

- Further main.rs decomposition (dispatch.rs, app.rs)
- Expose additional xurl-rs commands: timeline, mentions, followers/following, media upload
- Fix the TODO in db/client.rs (body re-serialization from JSON)
- `--dry-run` flag for write commands
- `--output-format json|text` flag
- Man page generation (clap_mangen)
- crates.io Trusted Publishing migration
- Submit to homebrew-core once bird has traction
- Path to v1.0.0 after real-world usage validates CLI contract
