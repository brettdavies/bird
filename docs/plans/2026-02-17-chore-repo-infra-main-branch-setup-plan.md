---
title: "chore: Repo Infrastructure — Main Branch & CI Setup"
type: chore
status: completed
date: 2026-02-17
deepened: 2026-02-17
---

# chore: Repo Infrastructure — Main Branch & CI Setup

## Enhancement Summary

**Deepened on:** 2026-02-17
**Research agents used:** best-practices-researcher (x2), architecture-strategist,
code-simplicity-reviewer, security-sentinel, pattern-recognition-specialist,
spec-flow-analyzer, framework-docs-researcher

### Key Improvements

1. **Single CI job replaces 3 parallel jobs** — compiles once instead of three
   times; faster wall-clock and lower GitHub Actions minutes
2. **Branching workflow clarified** — feature branches → development (PRs) →
   main (release merges with docs excluded). Guard workflow enforces separation.
3. **Release merge scripted** — a repeatable process strips compound-engineering
   docs from development before merging to main
4. **Cargo.lock committed** — binary crate should track lock file for
   reproducible builds and supply-chain security
5. **Security hardening** — explicit `permissions: contents: read`, concurrency
   control, backup tag before force push

### Critical Bugs Found in Original Plan

| Bug | Impact | Fix |
|-----|--------|-----|
| Typo `dtolnoy` in clippy action | CI fails immediately | Corrected to `dtolnay` |
| `items_after_test_module` in `src/output.rs` | Clippy fails with `-Dwarnings` | Move functions above `mod tests` |
| `--all-features` on project with no features | No-op, misleading | Removed |
| Missing `chmod +x` on hook | Hook silently ignored | Added to phase 4 |

---

## Overview

Establish main as the default branch with proper CI and workflow conventions.
Currently, `development` is the GitHub default branch, main is 26 commits behind
with a **disjoint history** (no common ancestor), there is no CI for tests/linting,
and the only workflow enforcement is a docs guard on PRs to main.

## Problem Statement

1. **Disjoint branch histories.** `main` and `development` were initialized
   independently. `git merge` fails without `--allow-unrelated-histories`. Main has
   2 commits of no unique value; development has 27 commits with all real work.

2. **No CI.** No `cargo test`, `cargo clippy`, or `cargo fmt --check` runs on PRs
   or pushes. Quality is enforced manually.

3. **No branch protection.** The repo is private on GitHub's free plan, which does
   not support branch protection rules or rulesets. Direct pushes to main are
   unguarded.

4. **Formatting drift.** `cargo fmt --check` currently fails with diffs across
   multiple files. This should be fixed before establishing CI.

5. **Default branch is `development`.** Should be `main` for conventional git
   workflow.

6. **`Cargo.lock` not committed.** For a binary crate, the Rust project recommends
   committing `Cargo.lock` for reproducible builds. Without it, each CI run resolves
   dependencies independently, widening the supply-chain attack window.

7. **`items_after_test_module` clippy warning.** In `src/output.rs`, the
   `emoji_available` and `emoji_unavailable` functions are defined after the
   `#[cfg(test)] mod tests` block. With `RUSTFLAGS: "-Dwarnings"`, this will
   fail CI.

## Proposed Solution

### Phase 1: Code Hygiene (on development)

Fix all formatting, lint issues, and structural problems so CI passes from day one.

- [x] **1a.** Fix `items_after_test_module` in `src/output.rs`: move
  `emoji_available()` and `emoji_unavailable()` above the `#[cfg(test)] mod tests`
  block
- [x] **1b.** Run `cargo fmt --all` to fix all formatting diffs
- [x] **1c.** Commit `Cargo.lock`: remove from `.gitignore` and `git add Cargo.lock`
- [x] **1d.** Verify `cargo clippy` passes with `-Dwarnings`
- [x] **1e.** Verify `cargo test` passes (116 tests)
- [x] **1f.** Commit: `style: fix clippy warning, cargo fmt, and commit Cargo.lock`

### Research Insights (Phase 1)

**Why commit `Cargo.lock` (security sentinel):** Without a committed lock file,
each CI run resolves dependencies independently. A compromised crate published
between runs could be silently pulled in. With the lock file, dependency updates
are explicit and visible in diffs. The Rust project recommends committing
`Cargo.lock` for all binary crates.

**The `items_after_test_module` issue (architecture strategist):** Clippy fires
this warning because items after `#[cfg(test)] mod tests` are unreachable in test
builds due to Rust's module ordering semantics. With `RUSTFLAGS: "-Dwarnings"` in
CI, this becomes a hard failure. Must be fixed before CI is added.

---

### Phase 2: Add CI Workflow (on development)

Add a GitHub Actions CI workflow. Single job with sequential steps (fmt → clippy →
test) compiles once and reuses build artifacts, which is faster and cheaper than
three parallel jobs for an 11k-line project.

- [x] **2a.** Create `.github/workflows/ci.yml`:

**`ci.yml`:**
```yaml
name: CI

on:
  push:
    branches: [main, development]
  pull_request:

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings"

jobs:
  check:
    name: Fmt, clippy, test
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --all --check

      - name: Clippy
        run: cargo clippy --all-targets

      - name: Tests
        run: cargo test
```

- [x] **2b.** Commit: `ci: add test, fmt, and clippy checks`

### Research Insights (Phase 2)

**Single job vs. 3 parallel (simplicity reviewer, CI best practices):** Three
parallel jobs means 3 VM spin-ups, 3 checkouts, 3 toolchain installs, and 3 full
dependency compilations. For an 11k-line project, the dominant cost is dependency
compilation, not the checks. A single job compiles once, then fmt/clippy/test
all reuse the same build artifacts. Faster wall-clock and ~1/3 the Actions minutes.

**Trigger pattern (CI best practices):** `push: [main, development]` catches
merge commits when PRs land on either branch. `pull_request:` (no branch filter)
fires for PRs targeting any branch — covering PRs to both development and main.
No double-triggering: PR source pushes go to feature branches (not main or
development), so only `pull_request` fires during PR workflow.

**`permissions: contents: read` (security sentinel):** When specified, all
unspecified permissions default to `none`. This follows least-privilege. The
existing `release.yml` has `permissions: contents: write` (needed for creating
releases), so this is consistent with setting explicit permissions per workflow.

**`concurrency` (architecture strategist, framework docs):** Cancels superseded
CI runs on the same branch. This matters on GitHub's free tier (2,000 minutes/month
for private repos). The expression `cancel-in-progress: true` is safe here since
CI is idempotent.

**`Swatinem/rust-cache@v2` (CI best practices):** Actively maintained (v2.8.2,
Nov 2025). Caches `~/.cargo` registry and `./target` dependencies. Cuts
subsequent CI runs from ~3-5 minutes to ~30-60 seconds for projects this size.
Must be placed after `dtolnay/rust-toolchain` so the Rust version is part of the
cache key.

**`RUSTFLAGS: "-Dwarnings"` at workflow level (CI best practices):** The official
Clippy docs and a Clippy maintainer both recommend this globally, not just for
clippy. Catches unused imports, dead code, etc. in test compilation too.

**`ubuntu-22.04` pinned (pattern recognition):** Both `release.yml` and
`guard-main-docs.yml` pin to `ubuntu-22.04`. Using `ubuntu-latest` would break
this convention. For Rust projects with bundled SQLite, the runner version rarely
matters, but consistency is valuable.

**No `--all-features` (CI best practices):** The project has no custom features
in `Cargo.toml`. The flag only activates extra features on dependencies, which is
not the intent.

**Step names (pattern recognition):** `release.yml` uses descriptive step names
(`Install Rust`, `Build release`). Added names for CI readability.

---

### Phase 3: Resolve Disjoint History & Switch Default Branch

The cleanest approach: reset main to development's HEAD, then force push.
Main's current 2-commit history has no unique value.

- [x] **3a.** Create a backup tag before force push:
  ```bash
  git tag backup/main-pre-sync main
  ```
- [x] **3b.** Reset main to development, then strip compound-engineering docs:
  ```bash
  git checkout main
  git reset --hard development
  git rm -r docs/plans/ docs/solutions/ docs/brainstorms/ 2>/dev/null
  git commit -m "chore: sync main from development, exclude engineering docs"
  git push --force-with-lease --force-if-includes origin main
  ```
- [x] **3c.** Switch default branch on GitHub:
  ```bash
  gh api repos/brettdavies/bird -X PATCH -f default_branch=main
  ```
- [x] **3d.** Verify development is unchanged:
  ```bash
  git checkout development
  ```

### Research Insights (Phase 3)

**`--force-if-includes` (branching best practices):** Added alongside
`--force-with-lease` for defense against background `git fetch` defeating the
lease check. Git 2.30+ feature. If the developer's editor (VS Code, etc.) runs
background fetches, `--force-with-lease` alone may silently succeed even when the
remote changed. `--force-if-includes` verifies the remote changes were actually
integrated locally.

**Backup tag (security sentinel):** Creates `backup/main-pre-sync` pointing at
the old main HEAD. Zero cost, easy rollback if anything goes wrong. Can be deleted
later with `git tag -d backup/main-pre-sync && git push origin :refs/tags/backup/main-pre-sync`.

**Docs exclusion on initial sync:** The `git rm -r docs/plans/ docs/solutions/
docs/brainstorms/` step strips compound-engineering docs before pushing to main.
This establishes the clean separation from the start. Future merges from
development to main follow the same pattern (see Release Merge Workflow below).

**Default branch switch gotchas (branching research):** No open PRs exist, so
no retargeting needed. CI workflows use explicit branch names (`branches: [main]`),
not "default branch", so they continue working. `git clone` behavior changes to
check out main — which is the desired outcome.

**Branching model (post-migration):**

```
main (default branch, releases tagged here, no compound-engineering docs)
  │
development (integration branch, all feature PRs target here)
  ├── feat/next-feature     (short-lived, PR to development)
  ├── fix/some-bug          (short-lived, PR to development)
  └── chore/some-task       (short-lived, PR to development)
```

Feature branches → development (via PR). When ready to release, development →
main (via release merge that strips docs). Tags created on main.

**Release merge workflow (repeatable process):**

When ready to merge development into main for a release:

```bash
# 1. Create release branch from development
git checkout -b release/v0.x.0 development

# 2. Strip compound-engineering docs
git rm -r docs/plans/ docs/solutions/ docs/brainstorms/ 2>/dev/null
git commit -m "chore: exclude engineering docs for release"

# 3. PR to main — guard workflow passes because docs are removed
gh pr create --base main --title "chore: merge development for vX.Y.Z release"

# 4. After merge, tag on main
git checkout main && git pull
git tag v0.x.0 && git push origin v0.x.0

# 5. Clean up
git branch -d release/v0.x.0
```

The guard workflow validates that no compound-engineering docs slip through.

---

### Phase 4: Local Safety Hook

Since branch protection is unavailable on the free plan, add a local pre-push
hook to prevent accidental direct pushes to main.

- [x] **4a.** Create `.githooks/pre-push`:
  ```bash
  #!/usr/bin/env bash
  protected_branch='main'
  current_branch=$(git rev-parse --abbrev-ref HEAD)

  if [ "$current_branch" = "$protected_branch" ]; then
    echo "[Policy] Never push directly to main. Use a PR from a feature branch."
    exit 1
  fi
  ```
- [x] **4b.** Make the hook executable and verify git tracks the permission:
  ```bash
  chmod +x .githooks/pre-push
  git ls-files -s .githooks/pre-push  # Should show mode 100755
  ```
- [x] **4c.** Configure git to use the hooks directory:
  ```bash
  git config core.hooksPath .githooks
  ```
- [x] **4d.** Document in DEVELOPER.md that new clones should run the
  `git config` command
- [x] **4e.** Commit on development:
  ```bash
  git checkout development
  git add .githooks/ docs/DEVELOPER.md
  git commit -m "chore: add pre-push hook to prevent direct pushes to main"
  git push origin development
  ```

### Research Insights (Phase 4)

**Simplicity reviewer says cut it.** The argument: you're a solo developer, the
cost of accidentally pushing to main is near-zero (fix with a force push in 30
seconds), and the hook adds ongoing maintenance (`.githooks/` dir, `core.hooksPath`
config, documentation, `chmod +x` tracking).

**Architecture strategist says keep it.** The argument: the hook is a cheap
"speed bump" that catches absent-minded mistakes. With no branch protection
available, it's the only automated guard besides CI.

**Recommendation: Keep it, but keep it minimal.** The hook is ~6 lines of bash.
The setup ceremony is one `git config` command. The cost is low. The user
explicitly asked for workflow enforcement.

**`#!/usr/bin/env bash` (architecture strategist):** More portable than
`#!/bin/bash`, especially for macOS where bash may not be at `/bin/bash` (Apple
ships zsh as default now). The release matrix includes `macos-14`.

**`chmod +x` is critical (spec-flow analyzer):** Git hooks without the
executable bit are **silently ignored** — no error, no warning. Git tracks the
execute permission bit, but cloning on Windows may lose it. The `git ls-files -s`
verification ensures the bit is committed.

**Phase 4 on development (spec-flow analyzer):** Since development remains the
integration branch, the hook commit goes there. It will reach main via the next
release merge. The hook protects against accidental direct pushes to main in the
meantime.

---

## Decisions

**D1: Reset, not merge.** `git reset --hard` produces a clean, linear history.
`--allow-unrelated-histories` would create a confusing merge of two independent
DAGs with no benefit. Confirmed by all reviewers.

**D2: No release branch for now.** Tags are commit pointers, not branch pointers.
The existing `release.yml` workflow triggers on `v*` tag pushes regardless of which
branch the tagged commit lives on. A release branch adds ceremony without value for
a solo developer. **Gotcha:** `generate_release_notes: true` diffs between tags,
not branches, so this works cleanly.

**D3: Local hook, not branch protection.** Private repos on GitHub's free plan
cannot use branch protection rules or rulesets. The hook is a discipline aid, not
enforcement — it can be bypassed with `--no-verify`. Acceptable for solo developer.

**D4: Development branch stays.** Feature branches PR into `development`.
Releases merge from `development` → `main` via a release branch that strips
compound-engineering docs. This preserves the separation between active
development and release-ready code.

**D5: Keep docs guard.** The `guard-main-docs.yml` workflow stays and enforces
that `docs/plans/`, `docs/solutions/`, and `docs/brainstorms/` never land on
main. The release merge workflow strips these files before the PR.

**D6: Single CI job.** Three parallel jobs triple compilation cost for an 11k-line
project. A single job with sequential steps (fmt → clippy → test) compiles once and
reuses artifacts. Split into parallel jobs later if CI time becomes painful.

## Acceptance Criteria

- [x] `cargo fmt --check` passes (zero diffs)
- [x] `cargo clippy` with `-Dwarnings` passes (zero warnings)
- [x] `cargo test` passes (116+ tests)
- [x] `Cargo.lock` is committed and tracked
- [x] `.github/workflows/ci.yml` exists with single job (fmt, clippy, test)
- [x] `.github/workflows/guard-main-docs.yml` is kept (enforces docs exclusion)
- [x] `main` is the default branch on GitHub
- [x] `main` has development's code minus compound-engineering docs
- [x] `.githooks/pre-push` prevents direct pushes to main (executable bit set)
- [x] `docs/DEVELOPER.md` documents the hooks setup and branching workflow

## Implementation Order

```
Phase 1 (hygiene):  1a → 1b → 1c → 1d → 1e → 1f  (single commit on development)
Phase 2 (CI):       2a → 2b                        (single commit on development)
Phase 3 (branches): 3a → 3b → 3c → 3d             (operational + one commit on main)
Phase 4 (hook):     4a → 4b → 4c → 4d → 4e        (commit on development)
```

Phases 1-2 and 4 are commits on development. Phase 3 syncs main (with docs
stripped) and switches the default branch.

## What This Does NOT Cover

- Actual release (tagging `v0.1.0`) — separate concern
- Making the repo public — separate decision
- Branch protection rules — requires GitHub Pro ($4/mo)
- PR templates, CONTRIBUTING.md — nice-to-have, not blocking
- `cargo audit` workflow — recommended as follow-up (separate scheduled workflow)
- SHA-pinning actions in `release.yml` — recommended follow-up (has write perms + secrets)

## Follow-Up Recommendations (from review agents)

**Security (medium priority):**
- Add a `cargo audit` scheduled workflow (weekly + on `Cargo.lock` changes)
- SHA-pin action references in `release.yml` (supply-chain hardening for
  workflow with `contents: write` and secret access)

**Operational (low priority):**
- Create `script/setup` to automate `git config core.hooksPath .githooks`
- Consider `just init` target (common in Rust ecosystem)
