---
title: "chore: Consolidate CI tokens into a single CI_RELEASE_TOKEN"
type: chore
status: active
date: 2026-03-19
---

# Consolidate CI Tokens into a Single CI_RELEASE_TOKEN

## Enhancement Summary

**Deepened on:** 2026-03-19
**Key improvements:**

1. Added explicit step-by-step execution order across repos
2. Added coordination constraints between reusable workflows and
   callers
3. Added fine-grained PAT permission verification from GitHub API
   docs
4. Incorporated learning from `gh-cli-fine-grained-pat-missing-
   oauth-scopes` solution doc (same 403 pattern)

## Overview

Three GitHub fine-grained PATs currently serve overlapping CI
purposes across five repos. Two of the three are over-scoped (All
Repositories) yet under-permissioned (Contents read-only where write
is needed), and neither has an expiration. Consolidate into one
properly scoped token.

## Problem Statement

| Token | 1P Item | Repos it's a secret on | Actual repo scope | Permissions issue | Expiration |
|-------|---------|----------------------|-------------------|-------------------|------------|
| `HOMEBREW_TAP_TOKEN` | `HOMEBREW_TAP_TOKEN` | bird, xurl-rs, homebrew-tap | All repos (over-scoped) | Contents **read-only** on bird — 403 on Contents API write | None |
| `RELEASE_TOKEN` | `Dotfiles — Release Token` | dotfiles | All repos (over-scoped) | Unknown write scope | None |
| gh CLI PAT | `GitHub PAT — brettdavies (gh CLI)` | None (local only) | All repos | Full access | 2026-06-15 |

The `HOMEBREW_TAP_TOKEN` caused a CI failure (run 23307110716) when
the changelog job tried to commit via the Contents API on bird — the
token lacks Contents write permission on that repo. This is the same
403 pattern documented in `~/dev/homebrew-tap/docs/solutions/
workflow-issues/gh-cli-fine-grained-pat-missing-oauth-scopes-
20260318.md` — fine-grained PATs fail silently on reads but 403 on
writes when the specific permission is missing.

## Proposed Solution

### Rename and consolidate

Merge `HOMEBREW_TAP_TOKEN` and `RELEASE_TOKEN` into a single token
named **`CI_RELEASE_TOKEN`**. The name reflects its actual purpose:
CI release automation across all repos.

| Attribute | Value |
|-----------|-------|
| **Name** | `CI_RELEASE_TOKEN` |
| **Type** | Fine-grained PAT |
| **Repository access** | All repositories |
| **Permissions** | Contents: Read and write |
| **All other permissions** | No access |
| **Expiration** | 1 year from creation |

**Why "All repositories":** Automating per-repo scope changes is not
possible via the GitHub API for personal accounts. "All repositories"
with a single narrow permission (Contents R+W) avoids manual token
updates when onboarding new tools.

**Why "Contents: Read and write" only:** This is the minimum
permission that covers all current uses:

| Operation | API endpoint | Permission needed |
|-----------|-------------|-------------------|
| Changelog auto-commit | `PUT /repos/{owner}/{repo}/contents/{path}` | Contents: write |
| Homebrew formula dispatch | `POST /repos/{owner}/{repo}/dispatches` | Contents: write |
| Git push (formula updates) | HTTPS push | Contents: write |
| Checkout with token | HTTPS clone | Contents: read |
| Release create/upload | `POST /repos/{owner}/{repo}/releases` | Contents: write |

**Ruleset bypass:** The token inherits the owner's admin role for
ruleset bypass. The `pull_request` rule on bird's `protect-main`
ruleset blocks `GITHUB_TOKEN` (not admin) but allows this PAT
(owned by brettdavies = admin = bypass actor).

### Token kept separate

| Token | Reason |
|-------|--------|
| `GitHub PAT — brettdavies (gh CLI)` | Local-only, broader permissions needed for interactive `gh` use. Never in CI. Already has expiration. |

## Execution Order

### Coordination constraint

The reusable workflows in `dot-github` define the `workflow_call`
secrets interface. Caller workflows in `bird`/`xurl-rs` must pass
secrets matching those parameter names. If the reusable workflow
renames a secret parameter but a caller still passes the old name,
the next CI run fails with an empty secret.

**The reusable workflow change and all caller changes must land in
rapid succession (< 5 minutes apart).**

`homebrew-tap` and `dotfiles` are independent — they reference
secrets directly, not through reusable workflows.

### Step-by-step execution

```text
Step 1: Human creates PAT on GitHub (manual, ~2 min)
Step 2: Update 1Password (automated, ~1 min)
Step 3: Set CI_RELEASE_TOKEN secret on all 4 repos (automated, ~1 min)
        ├── bird
        ├── xurl-rs
        ├── homebrew-tap
        └── dotfiles
        (old secrets still exist — both coexist)

Step 4: Push workflow changes — COORDINATED BLOCK (~5 min)
        ├── 4a. dot-github (reusable workflows) → push to main
        ├── 4b. bird (caller workflows + docs) → branch + PR
        └── 4c. xurl-rs (caller workflows) → branch + PR
        These three must land in rapid succession.

Step 5: Push workflow changes — INDEPENDENT
        ├── 5a. homebrew-tap → branch + PR
        └── 5b. dotfiles → branch + PR
        No coordination needed. Can happen any time after Step 3.

Step 6: Verify CI passes on all repos

Step 7: Delete old secrets (automated, ~1 min)
        ├── bird: delete HOMEBREW_TAP_TOKEN
        ├── xurl-rs: delete HOMEBREW_TAP_TOKEN
        ├── homebrew-tap: delete HOMEBREW_TAP_TOKEN
        └── dotfiles: delete RELEASE_TOKEN

Step 8: Delete old 1Password items
        ├── HOMEBREW_TAP_TOKEN
        └── Dotfiles — Release Token

Step 9: Update documentation
        ├── SKILL.md
        └── dot-github README.md (already updated in Step 4a)
```

### Repo change details

**Step 4a — `dot-github` (push direct to main):**

| File | Change |
|------|--------|
| `rust-release.yml` | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` in `workflow_call.secrets` and `homebrew` job env |
| `rust-ci.yml` | `CHANGELOG_TOKEN` → `CI_RELEASE_TOKEN` in `workflow_call.secrets` and fallback expression |
| `README.md` | Update interface contracts and caller examples |

**Step 4b — `bird` (PR to main):**

| File | Change |
|------|--------|
| `.github/workflows/release.yml` | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` |
| `.github/workflows/ci.yml` | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` |
| `RELEASING.md` | Update secret name and scope description |
| `docs/SECRETS.md` | Update secret name and scope description |

**Step 4c — `xurl-rs` (PR to main):**

| File | Change |
|------|--------|
| `.github/workflows/release.yml` | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` |
| `.github/workflows/ci.yml` | Add `CI_RELEASE_TOKEN` secret pass-through for changelog (if not present) |

**Step 5a — `homebrew-tap` (PR to main):**

| File | Refs | Change |
|------|------|--------|
| `.github/workflows/publish.yml` | 6 | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` |
| `.github/workflows/update-formula.yml` | 4 | `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN` (including git remote URL) |

**Step 5b — `dotfiles` (PR to main or direct push):**

| File | Refs | Change |
|------|------|--------|
| `.github/workflows/release.yml` | 4 | `RELEASE_TOKEN` → `CI_RELEASE_TOKEN` |

### 1Password item creation

Create via `create_item.sh`:

```bash
create_item.sh \
  --title "CI_RELEASE_TOKEN" \
  --tags "ci,github" \
  --notes "Fine-grained PAT for all CI release workflows. All
repositories, Contents read+write only. Used as CI_RELEASE_TOKEN
secret on bird, xurl-rs, homebrew-tap, and dotfiles. Operations:
Homebrew formula dispatch, changelog auto-commit (Contents API),
release creation, git push past rulesets (admin bypass)." \
  --hostname "github.com" \
  --field "username=brettdavies" \
  --field "credential[concealed]=<token>" \
  --field "type=Fine-grained token" \
  --field "expires=<unix-timestamp>"
```

## Acceptance Criteria

- [ ] Single `CI_RELEASE_TOKEN` secret on bird, xurl-rs,
  homebrew-tap, and dotfiles
- [ ] No `HOMEBREW_TAP_TOKEN` or `RELEASE_TOKEN` secrets remain
- [ ] CI passes on all 4 repos after the rename
- [ ] Changelog auto-commit works on bird (the original 403 is
  resolved)
- [ ] Homebrew dispatch works on a release (or dry-run verification)
- [ ] 1Password has one `CI_RELEASE_TOKEN` item with correct
  metadata, and old items are deleted
- [ ] All documentation references updated
- [ ] `rg HOMEBREW_TAP_TOKEN` across all 5 repos returns zero
  workflow hits (docs/plans are OK)
- [ ] `rg RELEASE_TOKEN` in dotfiles returns zero workflow hits

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| CI breaks between dot-github push and caller pushes | Low | Medium | Push all three in < 5 min window; no releases during window |
| Token scope too broad | Low | Low | Only Contents R+W; no admin/issues/PRs/actions permissions |
| Forget to update a workflow file | Low | Medium | `rg` sweep after all changes; acceptance criteria check |
| Homebrew dispatch fails with new token | Low | High | Verify with `gh api repos/.../dispatches` dry-run before deleting old secrets |
| `repository_dispatch` needs permissions beyond Contents | Very Low | High | Verified: GitHub docs confirm Contents: write is sufficient |
