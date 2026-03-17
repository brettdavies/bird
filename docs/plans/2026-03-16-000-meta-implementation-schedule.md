---
title: "meta: Implementation schedule for 2026-03-16 plans"
type: meta
status: completed
date: 2026-03-16
plans:
  - docs/plans/2026-03-16-001-feat-shell-completions-plan.md
  - docs/plans/2026-03-16-002-feat-quiet-flag-agentic-polish-plan.md
  - docs/plans/2026-03-16-003-feat-distribution-homebrew-crates-plan.md
---

# Implementation Schedule — 2026-03-16

## Dependency Graph

```text
Plan 001 (Completions) ──────────────────────────────────────────┐
    |                                                            |
    v                                                            v
Plan 003A (crates.io readiness)                Plan 003B (Homebrew tap + release)
    |
    v
Plan 002 (Quiet flag)
```

## Execution Order

### Phase 1: Plan 001 — Shell Completions

**Plan:** [2026-03-16-001-feat-shell-completions-plan.md](2026-03-16-001-feat-shell-completions-plan.md)
**Branch:** `feat/shell-completions`
**PR target:** `development`

Why first:

- Zero dependencies on other plans
- Unblocks Plan 003B (Homebrew formula needs `bird completions <shell>`)
- Restructures `main()` to exempt `completions` and `doctor` from xurl fail-fast — this change is also needed by Plan 003A but Plan 001 has the comprehensive solution
- Smallest code scope, cleanest starting point

Key deliverables:

- `clap_complete` dependency + `Completions` subcommand
- SIGPIPE fix (global `libc::signal`)
- `main()` restructuring: completions early-return, doctor early-return, xurl gate for API commands only
- Release workflow updates (archives, completions bundling, SHA256)
- Tests for completions, updated transport integration tests

### Phase 2: Plan 003 Stage A — crates.io Readiness

**Plan:** [2026-03-16-003-feat-distribution-homebrew-crates-plan.md](2026-03-16-003-feat-distribution-homebrew-crates-plan.md) (Stage A only)
**Branch:** `feat/distribution-crates-io`
**PR target:** `development`

Why second:

- No external dependencies (Stage A is self-contained)
- Benefits from Plan 001's `main()` restructuring being in place (skip the doctor/completions xurl exemption — already done)
- Touches Cargo.toml metadata, CI, docs, and release infrastructure
- Should ship before Plan 002 because Cargo.toml changes (license, features, exclude) are easier to merge before the quiet flag adds `"env"` feature to clap

Key deliverables:

- License change to `MIT OR Apache-2.0` + license files
- Cargo.toml metadata updates (documentation, exclude, binstall, release profile)
- Remove dead `BIRD_DEFAULT_CLIENT_ID` code
- Centralize xurl install instructions
- Rewrite stale docs (SECRETS.md, DEVELOPER.md)
- Create RELEASING.md, CHANGELOG.md, cliff.toml, deny.toml
- Pin GitHub Actions by SHA
- Add cargo-deny and cargo publish --dry-run to CI
- README install methods update

### Phase 3: Plan 002 — Quiet Flag

**Plan:** [2026-03-16-002-feat-quiet-flag-agentic-polish-plan.md](2026-03-16-002-feat-quiet-flag-agentic-polish-plan.md)
**Branch:** `feat/quiet-flag`
**PR target:** `development`

Why third:

- Largest file-touch surface (43 eprintln sites across 10 files)
- Adds `"env"` feature to clap in Cargo.toml — cleanest after Plan 003A's Cargo.toml changes
- Touches every command handler signature — fewer merge conflicts if done after structural changes
- Independent, but benefits from all prior changes being stable

Key deliverables:

- `--quiet` / `-q` flag with `FalseyValueParser` and `BIRD_QUIET` env var
- `diag!` macro in `src/output.rs`
- `quiet: bool` field on `BirdClient` struct
- Convert 40 suppressible `eprintln!` calls to `diag!`
- Thread `quiet` through all command handlers
- Tests for quiet mode and env var behavior

### Phase 4: Plan 003 Stage B — Homebrew Tap + Release Automation

**Plan:** [2026-03-16-003-feat-distribution-homebrew-crates-plan.md](2026-03-16-003-feat-distribution-homebrew-crates-plan.md) (Stage B only)
**Branch:** `feat/distribution-homebrew`
**PR target:** `development`

Why last:

- Depends on Plan 001 (completions subcommand for `generate_completions_from_executable`)
- Depends on Plan 003A (crates.io metadata, release profile, license files)
- Release workflow expansion builds on all prior changes

Key deliverables:

- 5-target build matrix with cross-compilation
- Release archives with binary + LICENSE + README
- macOS ad-hoc codesigning
- Version tag / Cargo.toml gating check
- cargo publish job (Trusted Publishing after manual v0.1.0)
- repository_dispatch trigger for homebrew-tap
- Updated Formula/bird.rb with sha256, completions, caveats

## Overlap Resolution

| Overlap | Resolution |
|---------|-----------|
| xurl fail-fast exemption for doctor | Plan 001 implements the full `main()` restructuring. Plan 003A skips this step (already done). |
| Cargo.toml changes | Plan 001 adds `clap_complete`. Plan 003A changes license, docs, exclude, binstall, release profile. Plan 002 adds `"env"` to clap features. Ordered to minimize conflicts. |
| Release workflow changes | Plan 001 switches to archives + completions bundling. Plan 003B expands to 5-target matrix + publish + dispatch. Sequential, not conflicting. |

## Branch / PR Strategy

Four separate PRs into `development`, one per phase. Each PR is a logical, reviewable unit:

1. `feat/shell-completions` -> `development`
2. `feat/distribution-crates-io` -> `development`
3. `feat/quiet-flag` -> `development`
4. `feat/distribution-homebrew` -> `development`

Commits within each PR follow SRP — logical, granular commits.

## Decisions Required Before Starting

From Plan 003:

1. **License:** Confirm `MIT OR Apache-2.0` is the intended direction
2. **Trusted Publishing vs CARGO_REGISTRY_TOKEN:** Recommendation is Trusted Publishing
3. **macOS Intel target inclusion:** Recommendation is include (Tier 1 until Sept 2026)
