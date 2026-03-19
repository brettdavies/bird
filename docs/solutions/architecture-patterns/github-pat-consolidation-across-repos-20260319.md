---
title: "Consolidate three GitHub fine-grained PATs into single CI_RELEASE_TOKEN for cross-repo CI workflows"
category: architecture-patterns
date: 2026-03-19
tags:
  - github-actions
  - fine-grained-pat
  - ci-cd
  - secrets-management
  - cross-repo
  - token-consolidation
  - changelog
  - release-pipeline
severity: high
affected_components:
  - dot-github/rust-ci.yml
  - dot-github/rust-release.yml
  - bird/ci.yml
  - bird/release.yml
  - homebrew-tap/publish.yml
  - homebrew-tap/update-formula.yml
  - dotfiles/release.yml
symptoms:
  - "Changelog job 403 error when committing via GitHub Contents API (run 23307110716)"
  - "Fine-grained PAT silent success on reads but 403 on writes when permission missing"
  - "Three overlapping tokens with inconsistent permission scopes across repos"
root_cause: "HOMEBREW_TAP_TOKEN fine-grained PAT had Contents read-only but CI needed Contents write for changelog auto-commits via the Contents API"
related_docs:
  - docs/solutions/architecture-patterns/release-pipeline-cross-platform-publish.md
  - "~/dev/homebrew-tap/docs/solutions/workflow-issues/gh-cli-fine-grained-pat-missing-oauth-scopes-20260318.md"
  - "~/dev/homebrew-tap/docs/solutions/workflow-issues/github-ruleset-merge-state-blocked-bypass-actors-20260318.md"
---

# Consolidate GitHub Fine-Grained PATs into CI_RELEASE_TOKEN

## Problem Description

Three GitHub fine-grained PATs served overlapping CI purposes across five
repositories:

| Token | Repos (as secret) | Scope | Issue |
|-------|-------------------|-------|-------|
| `HOMEBREW_TAP_TOKEN` | bird, xurl-rs, homebrew-tap | All repos, Contents **read-only** | 403 on Contents API write |
| `RELEASE_TOKEN` | dotfiles | All repos, unknown write scope | Redundant |
| `gh CLI PAT` | local only | All repos, full access | Kept separate |

The `HOMEBREW_TAP_TOKEN` caused GitHub Actions run 23307110716 to fail when
the Changelog job tried to commit via the Contents API on bird:

```text
Resource not accessible by personal access token (HTTP 403)
{"message":"Resource not accessible by personal access token",
 "documentation_url":"https://docs.github.com/rest/repos/contents#...",
 "status":"403"}
```

The failing API call was `PUT /repos/{owner}/{repo}/contents/{path}` — the
Contents API write endpoint used to commit the updated `CHANGELOG.md`.

This is the same failure pattern documented in
[gh-cli-fine-grained-pat-missing-oauth-scopes-20260318.md](~/dev/homebrew-tap/docs/solutions/workflow-issues/gh-cli-fine-grained-pat-missing-oauth-scopes-20260318.md):
fine-grained PATs fail silently on reads but 403 on writes when the specific
permission is missing.

## Root Cause Analysis

GitHub fine-grained PATs have granular permission levels per resource type.
The `HOMEBREW_TAP_TOKEN` was created with **Contents: Read** but the
changelog commit job requires **Contents: Read and write**. The token worked
for all read operations (checkout, API queries) but failed when the workflow
attempted to write back the generated changelog via the Contents API.

The problem was compounded by token proliferation: three tokens with
overlapping purposes made it unclear which token had which permissions, and
no token had an expiration date set.

## Investigation Steps

1. Identified the 403 error in bird CI run 23307110716, in the Changelog job
2. Extracted exact error: `Resource not accessible by personal access token
   (HTTP 403)` on `PUT /repos/{owner}/{repo}/contents/{path}`
3. Traced the token flow: bird `ci.yml` passed `secrets.HOMEBREW_TAP_TOKEN`
   as the `CHANGELOG_TOKEN` secret to the reusable `rust-ci.yml` workflow
4. Inspected the reusable workflow: the changelog job used
   `${{ secrets.CHANGELOG_TOKEN || secrets.GITHUB_TOKEN }}` as `GH_TOKEN`
5. Confirmed `HOMEBREW_TAP_TOKEN` had Contents read-only (not read+write)
6. Audited all three PATs across all repos to map the full scope overlap

## Solution

### New token specification

| Attribute | Value |
|-----------|-------|
| **Name** | `CI_RELEASE_TOKEN` |
| **Type** | Fine-grained PAT |
| **Repository access** | All repositories |
| **Permissions** | Contents: Read and write, Pull requests: Read and write |
| **All other permissions** | No access |
| **Expiration** | 1 year (2027-03-19) |

**Why "All repositories":** Automating per-repo scope changes is not possible
via the GitHub API for personal accounts. A single narrow permission
(Contents R+W) across all repos avoids manual token updates when onboarding
new tools.

**Why "Contents: Read and write" only:** This is the minimum permission that
covers all current CI operations:

| Operation | API endpoint | Permission needed |
|-----------|-------------|-------------------|
| Changelog auto-commit | `PUT /repos/.../contents/{path}` | Contents: write |
| Homebrew formula dispatch | `POST /repos/.../dispatches` | Contents: write |
| Git push (formula updates) | HTTPS push | Contents: write |
| Checkout with token | HTTPS clone | Contents: read |
| Release create/upload | `POST /repos/.../releases` | Contents: write |
| PR creation (formula update) | GraphQL `createPullRequest` | Pull requests: write |
| PR comment/close (bottle publish) | `POST /repos/.../pulls/comments` | Pull requests: write |

### Token verification before deployment

```bash
# Verify Contents read
curl -s -H "Authorization: Bearer $TOKEN" \
  "https://api.github.com/repos/brettdavies/bird/contents/README.md"
# HTTP 200

# Verify Contents write (dispatch test)
curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" \
  -X POST "https://api.github.com/repos/brettdavies/homebrew-tap/dispatches" \
  -d '{"event_type":"verify-scope-test"}'
# HTTP 204

# Verify expiration from response header
curl -s -I -H "Authorization: Bearer $TOKEN" "https://api.github.com/" \
  | grep github-authentication-token-expiration
# github-authentication-token-expiration: 2027-03-19 00:00:00 -0500
```

### Coordination constraint

The reusable workflows in `dot-github` define the `workflow_call.secrets`
interface. Renaming a secret in the reusable workflow is a **breaking change**
for all callers. The deployment sequence:

1. Push dot-github reusable workflow changes to `main` (admin bypass)
2. Immediately push caller workflow PRs (bird, homebrew-tap, dotfiles)
3. Caller CI runs pick up the updated reusable workflow from `@main`

### Workflow changes

**dot-github reusable workflows** (pushed directly to main):

```yaml
# rust-ci.yml — workflow_call.secrets
# Before:                           # After:
CHANGELOG_TOKEN:                     CI_RELEASE_TOKEN:
  required: false                      required: false

# rust-ci.yml — changelog job
# Before:                           # After:
GH_TOKEN: ${{ secrets.              GH_TOKEN: ${{ secrets.
  CHANGELOG_TOKEN ||                   CI_RELEASE_TOKEN ||
  secrets.GITHUB_TOKEN }}              secrets.GITHUB_TOKEN }}

# rust-release.yml — workflow_call.secrets
# Before:                           # After:
HOMEBREW_TAP_TOKEN:                  CI_RELEASE_TOKEN:
  required: true                       required: true
```

**bird caller workflows** (PR #20):

```yaml
# ci.yml — before                   # ci.yml — after
secrets:                             secrets:
  CHANGELOG_TOKEN: ${{ secrets.        CI_RELEASE_TOKEN: ${{ secrets.
    HOMEBREW_TAP_TOKEN }}                CI_RELEASE_TOKEN }}

# release.yml — before              # release.yml — after
secrets:                             secrets:
  HOMEBREW_TAP_TOKEN: ${{ secrets.     CI_RELEASE_TOKEN: ${{ secrets.
    HOMEBREW_TAP_TOKEN }}                CI_RELEASE_TOKEN }}
```

**homebrew-tap** (PR #19): All `HOMEBREW_TAP_TOKEN` → `CI_RELEASE_TOKEN`
in `publish.yml` (5 refs) and `update-formula.yml` (4 refs).

**dotfiles** (PR #26): All `RELEASE_TOKEN` → `CI_RELEASE_TOKEN` in
`release.yml` (4 refs).

## Verification

1. **bird CI** — All checks passed on merge to main, including the Changelog
   job that originally triggered the 403
2. **homebrew-tap CI** — lint, detect, guard-docs all passed
3. **dotfiles CI** — Release workflow succeeded
4. **Token API verification** — Confirmed read and write access via direct
   API calls before deploying to CI
5. **Sweep** — `rg HOMEBREW_TAP_TOKEN` and `rg CHANGELOG_TOKEN` return zero
   workflow hits across all repos

## Prevention Strategies

### Prevent token proliferation

- Maintain a token registry in 1Password with clear metadata (repos, permissions,
  expiration, purpose)
- Before creating a new token, audit existing tokens for reuse
- Run a quarterly consolidation audit: GitHub Settings > Fine-grained tokens
  cross-referenced against 1Password

### Catch permission mismatches early

- **Always test write operations locally** before adding a PAT to CI secrets.
  Fine-grained PATs succeed on reads but 403 on writes when the permission is
  missing — the failure is non-obvious
- Document required permissions per workflow in a comment block at the top of
  each workflow file
- Use the verification script above as a checklist for new tokens

### Token lifecycle management

- Never create a fine-grained PAT without an expiration (max 1 year)
- Create a calendar reminder 2 weeks before expiration
- Rotation procedure: create new → verify → update secrets → trigger CI →
  revoke old → update registry

### Branch base discipline

During this fix, all three caller PRs had conflicts because branches were
created from the wrong base. The rule:

| PR Target | Branch From | Command |
|-----------|-------------|---------|
| `main` | `origin/main` | `git checkout -b fix/thing origin/main` |
| `development` | `origin/development` | `git checkout -b feat/thing origin/development` |

Always branch from the merge target.

## Cross-References

- [release-pipeline-cross-platform-publish.md](release-pipeline-cross-platform-publish.md) —
  Documents the full release pipeline that uses `CI_RELEASE_TOKEN` for
  Homebrew dispatch
- [gh-cli-fine-grained-pat-missing-oauth-scopes-20260318.md][pat-403] —
  Documents the same 403 failure pattern with fine-grained PATs
- [github-ruleset-merge-state-blocked-bypass-actors-20260318.md][ruleset] —
  Explains how the admin-owned PAT bypasses branch protection rulesets

[pat-403]: ~/dev/homebrew-tap/docs/solutions/workflow-issues/gh-cli-fine-grained-pat-missing-oauth-scopes-20260318.md
[ruleset]: ~/dev/homebrew-tap/docs/solutions/workflow-issues/github-ruleset-merge-state-blocked-bypass-actors-20260318.md
