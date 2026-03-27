---
title: "feat: Make bird usage API-first by default"
type: feat
status: completed
date: 2026-03-26
---

# feat: Make bird usage API-first by default

## Overview

Invert `bird usage` so it hits the X API by default and falls back to local DB estimates only when explicitly requested
via `--local`. Currently the common case (real API data) requires `--sync` while the rare case (offline/local-only) is
the default.

## Problem Frame

Users almost always want actual usage numbers from the X API, not bird's local cost estimates. The API call is free (no
tweet cap cost), fast (~200ms), and already has graceful degradation on failure. Requiring `--sync` for the common path
is unnecessary friction, especially now that PR #28 adds cap and per-app display (only available from the API).

## Requirements Trace

- R1. `bird usage` hits the X API by default (no flag needed)
- R2. A `--local` flag exists to skip the API and show only local estimates
- R3. `--sync` is removed (breaking change, acceptable pre-1.0)
- R4. Graceful fallback to local data on API failure (already exists, preserved)
- R5. JSON output includes cap and per-app data by default when available

## Scope Boundaries

- No changes to the sync logic itself (`sync_actual_usage()` is unchanged)
- No changes to pretty formatting or JSON structure
- No new DB tables or persistence changes
- No deprecation period for `--sync` — clean removal (pre-1.0 project)

## Context & Research

### Relevant Code and Patterns

- `src/cli.rs:175-186` — clap `Usage` variant with `sync: bool` field
- `src/usage.rs:41-139` — `run_usage()` where `sync` bool gates API call
- `src/usage.rs:82` — decision point: `let mut sync_status = if sync { "failed" } else { "skipped" }`
- `src/requirements.rs:89-94` — `usage` (AuthType::None) vs `usage_sync` (AuthType::Bearer)
- `src/main.rs:369-376` — dispatch passes `sync` bool through
- Graceful degradation pattern: `Option<BirdDb>`, diag! macro for suppressible stderr

### Institutional Learnings

- `docs/solutions/architecture-patterns/quiet-flag-diagnostic-suppression-pattern.md` — diag! macro pattern for
  conditional stderr; `--local` diagnostic messages should use it
- Graceful degradation principle: cache/API failures never fatal, fall back silently

## Key Technical Decisions

- **`--local` over `--no-sync` or `--offline`**: `--local` is shorter, clearer, and doesn't reference the removed
  `--sync` concept. It describes what you get (local data), not what you skip.
- **Remove `--sync` entirely**: No deprecation needed pre-1.0. Clean break is simpler than maintaining a no-op flag.
- **Keep `usage_sync` auth entry**: The requirements.rs entry stays — it documents that Bearer auth is needed for the
  API path. The entry name is internal and doesn't need renaming.

## Open Questions

### Resolved During Planning

- **Should `--sync` be deprecated or removed?** Removed. Pre-1.0, no backwards compatibility obligation. A deprecated
  no-op flag adds complexity for zero benefit.
- **What auth requirement does the default path need?** Bearer (app-only). Already wired via `sync_actual_usage()` which
  hardcodes `AuthType::Bearer`. No change needed.

### Deferred to Implementation

- **Should `sync_status` field values change in JSON output?** Currently "skipped"/"failed"/"success". With API-first,
  "skipped" only appears with `--local`. Semantics still hold — defer to implementation to confirm no downstream
  consumers depend on specific values.

## Implementation Units

- [ ] **Unit 1: Invert CLI flag — replace `--sync` with `--local`**

  **Goal:** Change the clap definition so the boolean controls local-only mode instead of sync mode.

  **Requirements:** R1, R2, R3

  **Dependencies:** PR #28 merged to development (usage display enhancements)

  **Files:**
- Modify: `src/cli.rs` — Replace `sync: bool` with `local: bool` in the `Usage` variant, update help text
- Modify: `src/main.rs` — Update dispatch to pass `local` instead of `sync`
- Modify: `src/usage.rs` — Invert the boolean in `run_usage()`: parameter becomes `local: bool`, sync logic becomes `if
  !local` instead of `if sync`
- Test: `src/usage.rs` (existing tests) + `tests/cli_smoke.rs`

  **Approach:**
- In `cli.rs`: rename field, change `#[arg(long)]` help to "Show only local estimates (skip API)"
- In `main.rs`: pass `local` through dispatch
- In `usage.rs`: rename parameter, flip the conditional — `let sync = !local;` at the top of `run_usage()` keeps the
  rest of the function unchanged
- Update the "No usage data" hint: without `--local`, suggest checking Bearer auth instead of `--sync`

  **Patterns to follow:**
- Existing `--pretty`, `--quiet` flag patterns in `cli.rs`
- `diag!` macro for any new diagnostic messages

  **Test scenarios:**
- `bird usage --help` shows `--local` flag, no `--sync`
- `bird usage` (no flags) attempts API sync by default
- `bird usage --local` skips API, shows only local estimates
- `bird usage --local --pretty` works with pretty output

  **Verification:**
- `cargo fmt --all && cargo clippy && cargo test` all pass
- `bird usage --help` output shows `--local` and not `--sync`

- [ ] **Unit 2: Update requirements.rs and diagnostic messages**

  **Goal:** Clean up internal references to `--sync` and update user-facing messages.

  **Requirements:** R1, R3

  **Dependencies:** Unit 1

  **Files:**
- Modify: `src/requirements.rs` — Update comment on `usage_sync` entry (internal name, no rename needed, but comment
  should reflect new default)
- Modify: `src/usage.rs` — Update diag! messages that reference `--sync`

  **Approach:**
- Grep for string `"sync"` and `"--sync"` in usage.rs and requirements.rs
- Update diagnostic messages: "Run `bird usage --sync`" becomes irrelevant; "Showing local data only" messages should
  clarify this is fallback behavior
- No functional logic changes — just string updates

  **Test scenarios:**
- Diagnostic messages don't reference `--sync`
- JSON output `sync_status` field still works ("success"/"failed"/"skipped")

  **Verification:**
- `rg -- '--sync' src/` returns zero matches
- All tests pass

## System-Wide Impact

- **CLI surface:** `--sync` removed, `--local` added. Breaking change for any scripts using `--sync`. Acceptable
  pre-1.0.
- **JSON output:** `sync_status` field semantics unchanged. "skipped" now only appears with `--local`.
- **Auth:** Default path now requires Bearer auth availability. If Bearer isn't configured, graceful degradation shows
  local data with a diagnostic — same behavior as current `--sync` failure path.
- **bird skill:** The bird skill at `~/.claude/skills/bird/SKILL.md` references `bird usage --sync`. Should be updated
  to reflect new default.

## Risks & Dependencies

- **Dependency:** PR #28 (usage display enhancements) must merge first — this plan builds on the
  `SyncData`/`ProjectCap`/`AppDailyUsage` structs from that PR.
- **Risk:** Users without Bearer auth configured will see degraded output by default. Mitigated by existing graceful
  degradation and diagnostic messages guiding them to configure auth.

## Sources & References

- Todo: `.context/compound-engineering/todos/002-pending-p2-usage-default-to-api.md`
- Related PR: #28 (usage display enhancements, dependency)
- Related PR: #27 (usage JSON parsing fix)
- Solution: `docs/solutions/architecture-patterns/quiet-flag-diagnostic-suppression-pattern.md`
