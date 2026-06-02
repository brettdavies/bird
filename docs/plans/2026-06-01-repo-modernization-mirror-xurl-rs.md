---
title: Repo modernization — mirror xurl-rs sprint
date: 2026-06-01
status: in-progress
owner: brettdavies
---

# Repo modernization — mirror xurl-rs sprint

## Goal

Bring `bird` to parity with the `xurl-rs` repo standards landed 2026-05-31/06-01 (PRs #25–#28 there): split the release
docs into the canonical trio, adopt Rust 1.94 + MSRV bump + tightened tooling, vendor the supporting scripts, add
missing repo hygiene (SECURITY/CONTRIBUTING/CODEOWNERS/dependabot/PR template/issue routing), rename the integration
branch `development` → `dev`, and SHA-pin shared workflows. Net effect: bird ships releases the same way `xurl-rs` and
`agentnative-cli` do, and `/ce-work` subagents can drive each PR autonomously off this plan.

## Scope summary

**In scope**

- `development` → `dev` rename (GitHub branch, local tracking, ruleset retarget, CI trigger lists, doc references).
- Release-doc trio: enrich `RELEASES.md`, add `RELEASES-RATIONALE.md` and `RELEASES-PREFLIGHT.md` (bird-specific
  surface).
- `.github/pull_request_template.md` (canonical 165-LOC template from xurl-rs).
- `.markdownlint-cli2.yaml` refresh to 2026.04.15 calver header.
- `Cargo.toml` MSRV 1.87 → 1.94, exclude list expansion.
- `deny.toml` structural completeness (`[graph]`, `[output]`, workspace defaults, sources `allow-org`).
- `rust-toolchain.toml` comment refresh.
- `rustfmt.toml` already has `style_edition = "2024"` — verify only.
- `scripts/hooks/pre-push` expand 5 → 9 steps (toolchain banner, shellcheck, MSRV check, RUSTDOCFLAGS doc-warnings,
  Windows cross-clippy).
- `scripts/generate-changelog.sh`, `scripts/generate-changelog.py`, and `scripts/generate-completions.sh` vendored from
  `~/.claude/skills/rust-tool-release/scripts/` (sidecar `.py` confirmed present).
- `AGENTS.md` expand 66 → ~180 lines with YAML frontmatter, mirror sections from `agentnative-cli`.
- New repo-hygiene files: `CODEOWNERS`, `.github/dependabot.yml`, `.github/ISSUE_TEMPLATE/config.yml`.
- New workflow: `.github/workflows/guard-release-branch.yml` (wrapper around shared workflow confirmed present
  upstream).
- SHA-pin every `brettdavies/.github/.github/workflows/...@main` reference to the resolved HEAD SHA.
- Ruleset: `protect-dev.json` retarget `refs/heads/development` → `refs/heads/dev`; `protect-main.json` add
  `guard-release / check-release-branch-name` status check.
- README + AGENTS + CHANGELOG corrections: `RELEASING.md` → `RELEASES.md`, MSRV claim `1.87` → `1.94`, backfill missing
  `v0.1.3` section.
- Source-code fixes for Rust 1.94 lints + rustdoc warnings (collapsible-if, intra-doc links).

**Out of scope**

- xurl-rs PR #29 (library/CLI entrypoint split) and PR #30 (OAuth2 callback hardening) — those are xurl-internal.
- `release/v0.1.4` stale-branch resolution — tracked separately, placeholder PR-X below.
- The pending `docs/brainstorms/2026-04-03-xurl-crate-import-migration-requirements.md` refinement — committed
  direct-to-`dev` in Phase 0 as a planning artifact, separate from the sprint commits.
- `SECURITY.md` and `CONTRIBUTING.md` net-new authorship — `xurl-rs` does not ship either yet, so no source-of-truth to
  copy from. Defer until a canonical brettdavies template is authored (likely in `brettdavies/.github`).
- Any functional behaviour change to bird's auth, bookmarks, raw, or doctor surface — the sprint is repo-hygiene only.

## Phase 0 — branch rename and pre-flight cleanup

Phase 0 runs **before** any feature PRs. All steps below are interactive and performed by the operator (not a subagent),
in this order.

### Phase 0 steps

1. **Land planning artifacts on `development` first.**

- This plan file lands first (current commit, on `development` per the dev-direct exception for planning docs).
- Separately commit the existing modified `docs/brainstorms/2026-04-03-xurl-crate-import-migration-requirements.md`
     direct to `development` (planning artifact, dev-direct allowed): `git add
     docs/brainstorms/2026-04-03-xurl-crate-import-migration-requirements.md` → commit message `docs(brainstorms): bump
     xurl-rs target to v1.2.0 and refine migration plan` → push.

1. **Stash the two PR-foundation diffs so they survive the rename.**

- `git stash push -m "phase0-foundations" -- .markdownlint-cli2.yaml RELEASES.md` (these get unstashed inside their
     respective PR branches in PR1 and PR2).

1. **Push `dev` from `development`.**

- `git push origin development:dev`

1. **Confirm default integration branch in GitHub.**

- `gh api repos/brettdavies/bird --jq .default_branch` should already return `main`; do not change.
- We are **not** changing the default branch — only adding `dev` and retiring `development`. Releases still cut from
     `main`.

1. **Retarget the `protect-dev` ruleset.**

- Edit `.github/rulesets/protect-dev.json`: change `"include": ["refs/heads/development"]` → `["refs/heads/dev"]`.
- Re-apply via: `gh api -X PUT repos/brettdavies/bird/rulesets/<id> --input .github/rulesets/protect-dev.json` (look up
     `<id>` via `gh api repos/brettdavies/bird/rulesets --jq '.[] | select(.name=="protect-dev") | .id'`).
- This must happen **before** deleting `development` on the remote, or the unprotected branch is briefly exposed.

1. **Delete the old `development` branch on GitHub.**

- `gh api -X DELETE repos/brettdavies/bird/git/refs/heads/development`

1. **Retarget local clone.**

- `git checkout dev` (creates local tracking branch)
- `git branch -D development` (delete old local)
- `git remote set-head origin -a` (refresh HEAD reference)

1. **Verify CI trigger lists already reference `dev`.**

- Per current `.github/workflows/ci.yml`, triggers already are `[main, dev]`; confirm. If any workflow still says
     `development`, fix in that PR.

1. **Unstash foundation diffs once PR1 and PR2 branches exist** (handled inside those PRs — do NOT unstash on `dev`).

Exit criteria for Phase 0: `gh api repos/brettdavies/bird --jq .default_branch` returns `main`, `gh api
repos/brettdavies/bird/branches --jq '[.[].name]'` contains `main` and `dev` and no longer contains `development`, and
`gh api repos/brettdavies/bird/rulesets` shows `protect-dev` targeting `refs/heads/dev`.

## PRs (in merge order)

All PRs branch from `dev` after Phase 0. Each PR section has enough detail that a `/ce-work` subagent can execute it
from this plan alone. Commit message templates assume the subagent will write the body to a `/tmp/` file and submit via
`git commit --file` (never inline `-m`).

### PR1 — Release-doc trio + PR template

- **Branch:** `feat/release-doc-trio`
- **Complexity:** M
- **Parallelisable with:** none (foundation for PR2; many PRs reference `dev` paths that the trio defines).
- **Dependencies:** Phase 0 complete.
- **Files touched:**

- `RELEASES.md` — unstash the pending +202/-41 diff and absorb it as the new canonical runbook (trio split version, not
    the monolithic original).
- `RELEASES-RATIONALE.md` — new file; copy structure from `~/dev/xurl-rs/RELEASES-RATIONALE.md` and adapt examples to
    bird's surface (subprocess transport contract, cache, watchlist, instead of xurl-rs's media-upload and OAuth-store).
- `RELEASES-PREFLIGHT.md` — new file; bird-specific surface (see "Bird-specific preflight surface" below).
- `.github/pull_request_template.md` — new file; identical to `~/dev/xurl-rs/.github/pull_request_template.md` (165 LOC
    canonical).
- `README.md` — fix the one reference to `RELEASING.md` → `RELEASES.md` in the Documentation table (line ~263).

- **Concrete actions:**

1. `git stash pop` the `phase0-foundations` stash inside this branch and `git restore --staged .markdownlint-cli2.yaml`
     (we want only `RELEASES.md` in this PR; the markdownlint refresh moves to PR2).
2. Split `RELEASES.md` into three docs following the xurl-rs PR #27 split:

- `RELEASES.md` — runbook only (steps to cut a release).
- `RELEASES-RATIONALE.md` — the WHY (provenance rules, generated-changelog rule, `make_latest:false` decision, branch
       protection rationale).
- `RELEASES-PREFLIGHT.md` — pre-cut checklist (bird-specific; see surface below).

1. Drop `.github/pull_request_template.md` in place verbatim from xurl-rs.
2. README fix: `s/RELEASING\.md/RELEASES.md/` (one location near "Documentation" section).

- **Bird-specific preflight surface** (mirror xurl-rs section structure, adapt content):

- Build & test: `cargo build --release`, `cargo test --all`. Bird currently ships 17 source files (~6.3 KLOC) and
    transport-integration tests in `tests/`. The exact unit + integration count should be captured at PR1 time.
- Static analysis: `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- Doc warnings: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`.
- Supply-chain: `cargo deny check`.
- Bird-specific surface area to manually exercise per release (no real xurl-style auth panel here — bird delegates all
    auth to xurl):

- `bird --version` returns the expected `vX.Y.Z`.
- `bird doctor` and `bird doctor <cmd>` exit 0 on a healthy machine, 78 on missing xurl, 78 on bad config.
- `bird me --pretty`, `bird me --output json`, `bird me -q` round-trip via the entity store on a logged-in account.
- `bird bookmarks` paginates and streams (does not collect into memory before printing).
- `bird search "rust" --sort likes --min-likes 100 --pages 2 --output json` returns sorted JSON.
- `bird thread <tweet_id>` reconstructs a multi-page thread.
- `bird profile <handle>` resolves via `schema::validate_username` (strips `@`, charset check).
- `bird watchlist add @x`, `bird watchlist list`, `bird watchlist check`, `bird watchlist remove @x` round-trip via the
      local config (`~/.config/bird/config.toml`).
- `bird usage --local`, `bird usage --sync` exercise the usage subsystem; both should respect `--pretty`.
- `bird cache stats --pretty` shows store path, size, tweet/user/raw counts; `bird cache clear` drops counts to zero.
- `bird tweet "..."`, `bird reply <id> "..."`, `bird like <id>`, `bird unlike <id>`, `bird repost <id>`, `bird unrepost
      <id>`, `bird follow <user>`, `bird unfollow <user>`, `bird dm <user> "..."`, `bird block`, `bird unblock`, `bird
      mute`, `bird unmute` each round-trip via xurl passthrough; each must reject `--cache-only` with a Command error
      (code 1).
- `bird get /2/users/me -p id=123 -q expansions=author_id --pretty`, plus `bird post`, `bird put`, `bird delete` against
      a safe endpoint (e.g. a test workspace).
- `bird completions bash`, `zsh`, `fish`, `powershell`, `elvish` each produce a parseable script (`bash -n`, `zsh -n`,
      `fish --no-execute`).

- Subprocess transport contract:

- Override resolution: `BIRD_XURL_PATH=/path/to/xr bird raw /2/users/me` must use the overridden binary.
- Default discovery: with `BIRD_XURL_PATH` unset, bird must find `xr` (xurl-rs) first, then `xurl` (Go fallback).
- Missing-xurl exit: with `BIRD_XURL_PATH` pointing at a non-existent path, `bird me` exits 78 (config) with the install
      hint surfaced.
- SIGPIPE: `bird bookmarks --output jsonl | head -1` exits cleanly (no broken-pipe panic). The Unix-only
      `libc::signal(SIGPIPE, SIG_DFL)` is `#[cfg(unix)]`-gated; the pre-push hook's libc-grep is the always-on backstop.

- Output and color contract:

- `bird me --pretty` shows ANSI color and hyperlinks on a TTY.
- `bird me --plain` strips color and hyperlinks.
- `bird me --no-color` or `NO_COLOR=1 bird me` strips color only.
- `bird me 2>/tmp/err 1>/tmp/out` (stderr non-TTY) auto-selects JSON for the stderr error envelope.
- `bird me --output json` and `BIRD_OUTPUT=json bird me` both force JSON errors.
- `bird me -q` suppresses informational stderr diagnostics.

- Cache modes:

- `bird search "rust" --refresh` bypasses store, refreshes store.
- `bird search "rust" --no-cache` neither reads nor writes the store.
- `bird search "rust" --cache-only` serves from store only and never calls xurl. Write commands must reject
      `--cache-only`.

- Token & file permissions: bird does not store tokens itself (xurl does); confirm that `~/.config/bird/config.toml` is
    created at `0644` (config) and that `~/.config/bird/bird.db` (SQLite) is created at `0600`.

- Three exit codes: 0 success, 77 auth (XurlError::Auth detected), 78 config (missing xurl, invalid config), 1
    everything else. Stderr JSON envelope on `--output json`: `{"error":..., "kind":"config"|"auth"|"command",
    "code":78|77|1, "command":..., "status":...}`.

- Triple-diff verification before tag: `git diff origin/main..HEAD`, `git diff HEAD..origin/dev` (no non-doc paths),
    `git diff origin/dev..origin/main` (sanity) — all three must agree on intended scope. The leak-grep guard from
    xurl-rs's preflight applies verbatim to bird's `docs/plans/`, `docs/brainstorms/`, `docs/solutions/`,
    `docs/reviews/`, `.context/` paths.

- **Acceptance criteria:**

- The three files exist with correct names and YAML frontmatter where appropriate.
- Markdown lints cleanly (`markdownlint-cli2 "**/*.md"`).
- PR body uses `.github/pull_request_template.md` and fills every required section.
- No content lost from the prior monolithic `RELEASES.md` — moved, not deleted.

- **Commit-message template** (write to `/tmp/ce-commit-pr1-$$.md`):

  ```text
  docs(release): adopt three-doc release pattern and align repo with standards

  Split the prior monolithic RELEASES.md into the canonical trio:
  - RELEASES.md — runbook (cut-a-release steps only)
  - RELEASES-RATIONALE.md — the WHY (provenance, generated changelog, branch
    protection, make_latest:false)
  - RELEASES-PREFLIGHT.md — bird-specific pre-cut checklist

  Also add the canonical .github/pull_request_template.md and fix the one
  README reference to RELEASING.md.
  ```

### PR2 — Markdownlint + repo-hygiene files

- **Branch:** `chore/repo-hygiene`
- **Complexity:** S
- **Parallelisable with:** PR3 (different files; no overlap).
- **Dependencies:** PR1 merged (templates referenced by the issue-template body and dependabot CODEOWNERS resolution).
- **Files touched:**

- `.markdownlint-cli2.yaml` — unstash the remaining `phase0-foundations` half (the +11/-4 calver-header refresh from the
    pending diff).
- `CODEOWNERS` — new; single line `* @brettdavies`. Place at repo root (`xurl-rs` does not have one; this becomes the
    canonical placement). Acceptable to move to `.github/CODEOWNERS` if a reviewer prefers.
- `.github/dependabot.yml` — new; author from scratch (neither xurl-rs nor agentnative-cli ship one yet). Cover the two
    ecosystems bird actually uses: `cargo` (weekly, groups for security-only updates) and `github-actions` (weekly).
    Reviewer at `@brettdavies`.
- `.github/ISSUE_TEMPLATE/config.yml` — new; `contact_links` block routing off-topic issues to GitHub Discussions and
    pointing at the xurl-rs / xurl repos for upstream auth/transport issues.

- **Acceptance criteria:**

- `markdownlint-cli2 "**/*.md"` clean.
- `gh repo view --json codeOfConduct,securityPolicyUrl` enumerates without error (we add no SECURITY.md here; this just
    confirms the API still responds 200).
- Dependabot config validates: `gh api repos/brettdavies/bird/dependabot/alerts` enumerates (200 not 404).

- **Commit-message template:**

  ```text
  chore(repo): add hygiene files (CODEOWNERS, dependabot, issue routing) and refresh markdownlint

  Brings bird closer to parity with xurl-rs and agentnative-cli repo standards.
  SECURITY.md and CONTRIBUTING.md are deferred until a canonical template lands
  upstream in brettdavies/.github.
  ```

### PR3 — AGENTS.md expansion + YAML frontmatter

- **Branch:** `docs/agents-expansion`
- **Complexity:** M
- **Parallelisable with:** PR2 (different files).
- **Dependencies:** Phase 0 complete (references `dev` throughout).
- **Files touched:**

- `AGENTS.md` — expand 66 → ~180 lines.

- **Concrete actions:**

1. Add YAML frontmatter (format from agentnative-cli/xurl-rs):

     ```yaml
     ---
     name: bird
     binary: bird
     description: Rust CLI for the X (Twitter) v2 API. Adds entity caching, watchlist, search, thread reconstruction, and structured agent output on top of xurl.
     homepage: https://github.com/brettdavies/bird
     repository: https://github.com/brettdavies/bird
     ---
     ```

2. Adopt section structure from `~/dev/xurl-rs/AGENTS.md` (closest match by surface): Running bird, Architecture (CLI +
     xurl subprocess + entity store), Transport dependency (`BIRD_XURL_PATH`, `xr` vs `xurl` discovery), Output formats
     (text/JSON/JSONL via `OutputConfig`), Cache modes (`--refresh`, `--no-cache`, `--cache-only`), Quality bar,
     Testing, Releasing (point at trio), Known debt, Documented solutions (the 6-line section xurl-rs PR #26 added), and
     the references block.
3. Fix MSRV claim: `1.87` → `1.94`.
4. Update the known-debt section to match current code state (main.rs is 766 lines per the inspection, not 710). Keep
     db/db.rs and db/client.rs 200-line-trigger callouts.

- **Acceptance criteria:**

- File has valid YAML frontmatter (parseable by `yq`).
- Section headings track xurl-rs/agentnative-cli conventions.
- MSRV claim is `1.94`.
- Markdownlint clean.

- **Commit-message template:**

  ```text
  docs(agents): expand AGENTS.md to canonical format with YAML frontmatter

  Mirrors xurl-rs/agentnative-cli structure: frontmatter, surface map, exit
  codes, release process pointer. MSRV claim aligned to 1.94 and known-debt
  section refreshed against current line counts.
  ```

### PR4 — Vendor release scripts

- **Branch:** `feat/vendor-release-scripts`
- **Complexity:** S
- **Parallelisable with:** PR5, PR6, PR7 (separate files).
- **Dependencies:** PR1 merged (RELEASES.md references these scripts).
- **Files touched:**

- `scripts/generate-changelog.sh` — new; copy verbatim from
    `~/.claude/skills/rust-tool-release/scripts/generate-changelog.sh` (identical to xurl-rs's vendored copy).
- `scripts/generate-changelog.py` — new; copy the sidecar from
    `~/.claude/skills/rust-tool-release/scripts/generate-changelog.py` (confirmed present, 6.0 KB).
- `scripts/generate-completions.sh` — new; copy verbatim from
    `~/.claude/skills/rust-tool-release/scripts/generate-completions.sh` (identical to xurl-rs's vendored copy).

- **Acceptance criteria:**

- Scripts are `+x` (record in commit; verify `git show :scripts/generate-changelog.sh | head -1` is shebang).
- `bash -n scripts/generate-changelog.sh` and `bash -n scripts/generate-completions.sh` parse clean.
- `python3 -m py_compile scripts/generate-changelog.py` exits 0.
- `shellcheck scripts/*.sh` exits 0 with `--severity=warning`.

- **Commit-message template:**

  ```text
  feat(scripts): vendor generate-changelog.{sh,py} and generate-completions.sh

  Brings bird onto the canonical release tooling: PR-body-aware changelog
  generation and shell-completion publishing. Mirrors xurl-rs PR #28.
  ```

### PR5 — MSRV 1.94 + Cargo/deny/toolchain/rustfmt

- **Branch:** `fix/msrv-1-94-and-toolchain`
- **Complexity:** L (touches source files for clippy/rustdoc fixes).
- **Parallelisable with:** PR6, PR7 (different files).
- **Dependencies:** none directly, but easier after PR4 (so the new scripts can verify locally).
- **Files touched:**

- `Cargo.toml` — bump `rust-version = "1.87"` → `"1.94"`; expand `exclude` list to mirror xurl-rs (add `.context/`,
    `.markdownlint-cli2.yaml`, `benches/` if applicable, `deny.toml`, `scripts/`, on top of the existing `.claude/`,
    `.github/`, `.githooks/`, `cliff.toml`, `docs/`, `rustfmt.toml`, `tests/`, `todos/`).
- `deny.toml` — add `[graph]` and `[output]` blocks; expand `[bans]` with `workspace-default-features` and
    `external-default-features`; add `[sources.allow-org]` block (initially empty arrays). Mirror structure exactly from
    `~/dev/xurl-rs/deny.toml`.
- `rust-toolchain.toml` — comment is currently dated `2026-03-25`; xurl-rs has `2026-03-26`. Refresh comment to the
    actual release date of the pinned rustc; channel itself stays at `1.94.1`.
- `rustfmt.toml` — already contains `style_edition = "2024"` and `edition = "2024"`; verify only, no edit needed.
- `src/**/*.rs` — fix Rust 1.94 lints surfaced by the toolchain bump:

- `clippy::collapsible_if` — collapse nested `if` where applicable.
- rustdoc broken intra-doc links and missing-docs warnings on public APIs.
- Any new `let_else` / `manual_let_else` suggestions.

- **Concrete actions:**

1. Bump MSRV in `Cargo.toml`; `cargo update -p bird` to refresh `Cargo.lock`.
2. Update `deny.toml` structurally.
3. Refresh `rust-toolchain.toml` comment.
4. Run `cargo +1.94.1 clippy --all-targets -- -D warnings` and fix in place.
5. Run `RUSTDOCFLAGS="-D warnings" cargo +1.94.1 doc --no-deps` and fix.
6. Run `cargo +1.94.1 deny check` and resolve any new advisories.
7. Confirm `cargo +1.94.1 test --all` is fully green.

- **Acceptance criteria:**

- CI passes on Rust 1.94.1 across the matrix.
- `cargo fmt --all --check` clean.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo deny check` clean.
- `cargo test --all` all green.
- `cargo doc --no-deps` clean with `-D warnings`.

- **Commit-message template:**

  ```text
  fix(quality): bump MSRV to 1.94 and align Cargo/deny/toolchain/rustfmt with framework parity

  Mirrors xurl-rs PR #28: MSRV 1.87 → 1.94, deny.toml structural completeness,
  rust-toolchain.toml comment refresh, source fixes for Rust 1.94 clippy and
  rustdoc warnings (collapsible-if, intra-doc links).
  ```

### PR6 — Pre-push hook expansion 5 → 9 steps

- **Branch:** `feat/pre-push-hook-parity`
- **Complexity:** S
- **Parallelisable with:** PR5, PR7.
- **Dependencies:** PR4 merged (uses the new scripts) and PR5 merged (so the MSRV check matches the bumped value).
- **Files touched:**

- `scripts/hooks/pre-push` — expand current 5-step hook to mirror the 9-step xurl-rs version (
    `~/dev/xurl-rs/scripts/hooks/pre-push`, 127 lines). Adds: toolchain banner; MSRV verification block (`awk
    Cargo.toml` + `cargo +<msrv> check --quiet`); rustdoc-as-warnings step (`RUSTDOCFLAGS="-D warnings" cargo doc
    --no-deps --quiet`); shellcheck step on `git ls-files '*.sh' 'scripts/hooks/*'`; Windows cross-clippy step (`cargo
    clippy --target x86_64-pc-windows-gnu --all-targets -Dwarnings`) with skip-and-hint fallback when mingw or the
    windows target is missing.

- **Acceptance criteria:**

- `bash -n scripts/hooks/pre-push` parses.
- `shellcheck --severity=warning scripts/hooks/pre-push` clean.
- Running the hook locally executes all 9 steps and exits 0 on a clean tree.

- **Commit-message template:**

  ```text
  feat(hooks): expand pre-push from 5 to 9 steps for framework parity

  Adds toolchain banner, shellcheck, MSRV verification, rustdoc-as-warnings, and
  Windows cross-clippy. Mirrors xurl-rs PR #28.
  ```

### PR7 — SHA-pin reusable workflows + add guard-release-branch

- **Branch:** `ci/sha-pin-reusable-workflows`
- **Complexity:** M
- **Parallelisable with:** PR5, PR6 (different files).
- **Dependencies:** none.
- **Files touched:**

- `.github/workflows/ci.yml` (currently pins `brettdavies/.github/.github/workflows/rust-ci.yml@main`)
- `.github/workflows/release.yml` (`rust-release.yml@main`)
- `.github/workflows/finalize-release.yml` (`rust-finalize-release.yml@main`)
- `.github/workflows/guard-main-docs.yml` (`guard-main-docs.yml@main`)
- `.github/workflows/guard-main-provenance.yml` (`guard-main-provenance.yml@main`)
- `.github/workflows/guard-release-branch.yml` — NEW; copy verbatim from
    `~/dev/xurl-rs/.github/workflows/guard-release-branch.yml` (28 lines; job key `guard-release` is load-bearing for
    the status-check context name).

- **Concrete actions:**

1. Resolve the current HEAD SHA of `brettdavies/.github`. At plan-write time this was
     `469eb204a6d9e88af0f1e2d27c40b45136810344`. Re-resolve at PR-prep time with `gh api
     repos/brettdavies/.github/commits/main --jq .sha` and use that SHA.
2. Find every `brettdavies/.github/.github/workflows/*.yml@main` reference in all bird workflows and replace `@main`
     with `@<SHA> # main as of YYYY-MM-DD` (5 existing + 1 new = 6 lines total).
3. Add the new `guard-release-branch.yml` wrapper. Confirmed present upstream at
     `brettdavies/.github/.github/workflows/guard-release-branch.yml` (SHA `58f40efb…` at plan-write time, 2441 bytes).

- **Acceptance criteria:**

- `grep -rn "@main\b" .github/workflows/` returns no `brettdavies/.github/...` matches.
- Every reusable-workflow line has a trailing `# main as of YYYY-MM-DD` comment.
- CI runs green on PR head (proves the SHA pins resolve and the workflows still function).
- GitHub Actions UI shows a `guard-release / check-release-branch-name` check on the PR (this is the context PR8 will
    require).

- **Commit-message template:**

  ```text
  ci(supply-chain): SHA-pin brettdavies/.github reusable workflows and add guard-release-branch wrapper

  Per global supply-chain policy: pin to immutable commit SHAs instead of @main.
  Trailing comment names the dated tip. Adds guard-release-branch wrapper to
  enforce release/* branch naming on PRs to main.
  ```

### PR8 — Ruleset updates (protect-main status check + protect-dev retarget commit)

- **Branch:** `ci/ruleset-updates`
- **Complexity:** S
- **Parallelisable with:** none (must follow PR7 because the new status-check context only exists once
  `guard-release-branch.yml` runs once on `dev`).
- **Dependencies:** PR7 merged AND at least one CI run on `dev` that produces the new status-check name.
- **Files touched:**

- `.github/rulesets/protect-main.json` — add status check context `guard-release / check-release-branch-name` to the
    `required_status_checks` array. Verified verbatim from `~/dev/xurl-rs/.github/rulesets/protect-main.json` — the
    existing 6 contexts (`ci / Fmt, clippy, test`, `ci / Package check`, `ci / Security audit (bans licenses sources)`,
    `ci / Changelog`, `guard-docs / check-forbidden-docs`, `guard-provenance / check-provenance`) plus the new 7th
    context.
- `.github/rulesets/protect-dev.json` — commit the Phase 0 retarget (`refs/heads/development` → `refs/heads/dev`) so the
    on-disk file matches the deployed ruleset.

- **Concrete actions:**

1. Edit `.github/rulesets/protect-main.json`; append `{ "context": "guard-release / check-release-branch-name" }` to
     `rules[].parameters.required_status_checks` (the array currently has 6 entries; new total is 7).
2. Edit `.github/rulesets/protect-dev.json`; change `"include": ["refs/heads/development"]` → `["refs/heads/dev"]`.
3. Re-apply both rulesets:

     ```bash
     gh api -X PUT repos/brettdavies/bird/rulesets/<main-id> --input .github/rulesets/protect-main.json
     gh api -X PUT repos/brettdavies/bird/rulesets/<dev-id> --input .github/rulesets/protect-dev.json
     ```

4. Confirm with `gh api repos/brettdavies/bird/rulesets/<main-id> --jq
     '.rules[]|select(.type=="required_status_checks")'`.

- **Acceptance criteria:**

- On-disk JSON matches deployed ruleset (diff is empty).
- Test PR against `main` fails until `guard-release` succeeds.

- **Commit-message template:**

  ```text
  ci(rulesets): add guard-release-branch status check to main and retarget dev ruleset

  Codifies the Phase 0 development→dev rename and locks main behind the new
  release-branch-naming guard.
  ```

### PR9 — CHANGELOG v0.1.3 backfill + README MSRV correction

- **Branch:** `docs/changelog-and-msrv-claims`
- **Complexity:** S
- **Parallelisable with:** PR8 (different files).
- **Dependencies:** PR4 merged (so the changelog generator is in place and can be used to validate the backfilled
  section).
- **Files touched:**

- `CHANGELOG.md` — add the missing `[0.1.3]` section between `[0.1.2]` and a new top-of-file slot. v0.1.3 was tagged on
    `main` (commit `93f63bf fix(transport): release v0.1.3 — pipe deadlock fix and CI hardening (#26)`) but never landed
    in CHANGELOG.md (current file ends at `[0.1.2]`). Use `scripts/generate-changelog.sh` with `--tag v0.1.3` to draft,
    then verify against the actual PR body content for #24 (pipe deadlock fix) and the v0.1.3 release notes on GitHub.
- `README.md` — `1.87` → `1.94`. There is no MSRV claim in the current README install section, but verify; this is
    primarily an AGENTS-side fix done in PR3. README's `cargo install` example may need a `--locked` flag mention.

- **Acceptance criteria:**

- `git diff CHANGELOG.md` shows only the new `[0.1.3]` section, not edits to existing sections (CHANGELOG is generated
    and historical sections must not be retouched).
- `grep -nE "\bMSRV\b.*1\.87|\b1\.87\b.*MSRV" README.md` returns no matches.

- **Commit-message template:**

  ```text
  fix(docs): backfill CHANGELOG v0.1.3 and correct README MSRV claims to 1.94

  v0.1.3 shipped to main but never landed in CHANGELOG.md. Generated the
  missing section from PR bodies via scripts/generate-changelog.sh. README
  MSRV claim aligned with the new toolchain pin.
  ```

### PR10 — `dev` references in all consumer-facing markdown

- **Branch:** `docs/dev-branch-references`
- **Complexity:** S
- **Parallelisable with:** PR9.
- **Dependencies:** Phase 0 complete (the rename must already be live).
- **Files touched:**

- `README.md`, `AGENTS.md`, `RELEASES.md`, `RELEASES-RATIONALE.md`, `RELEASES-PREFLIGHT.md`, `docs/**/*.md` (excluding
    `docs/plans/`, `docs/brainstorms/`, `docs/solutions/`, `docs/reviews/` — those are historical artifacts and
    `guard-main-docs` keeps them off main anyway).

- **Concrete actions:**

1. `git grep -nE "\bdevelopment\b" -- '*.md' ':(exclude)docs/plans' ':(exclude)docs/brainstorms'
     ':(exclude)docs/solutions' ':(exclude)docs/reviews'`
2. Replace branch-name uses of `development` with `dev`. Leave the English word "development" (e.g., "during
     development", "developer experience") untouched.
3. Replace any `git checkout development` examples with `git checkout dev`; replace `origin/development` with
     `origin/dev`; replace `protect-dev ... refs/heads/development` in markdown prose with `refs/heads/dev`.

- **Acceptance criteria:**

- `git grep -nE "\borigin/development\b|\bgit checkout development\b|refs/heads/development" -- '*.md'` returns nothing
    outside excluded paths.
- Markdownlint clean.

- **Commit-message template:**

  ```text
  docs: update consumer-facing markdown to reference dev (renamed from development)

  Phase 0 of the modernization sprint renamed the integration branch
  development→dev. This PR catches up the README, AGENTS, RELEASES, and
  release-doc trio references.
  ```

### PR-X — Resolve stale `release/v0.1.4` branch

- **Branch:** TBD (depends on investigation outcome)
- **Complexity:** TBD
- **Status:** placeholder — running investigation in parallel. The branch exists locally and on the remote (last
  modification predates the dormant window) and v0.1.4 was never tagged.
- **Action:** Once the investigation concludes, fill this section with the resolution (likely either rebase +
  tag-and-merge, or `gh api -X DELETE repos/brettdavies/bird/git/refs/heads/release/v0.1.4` and start v0.1.4 fresh from
  `main` post-sprint).

## Verification gates

Per-PR gates the `/ce-work` subagent must clear before opening the PR.

### All PRs

- `cargo fmt --all --check` clean (Rust-touching PRs only).
- `markdownlint-cli2 "**/*.md"` clean (docs-touching PRs).
- PR body authored in `/tmp/ce-pr-body-<branch>-$$.md`, scrubbed with `/unslop`, submitted via `gh pr create
  --body-file`. No inline `--body` or heredoc.
- Commit message authored in `/tmp/ce-commit-<branch>-$$.md`, submitted via `git commit --file`. No inline `-m`.
- No AI attribution trailers (`Co-Authored-By: Claude`, robot-emoji "Generated with" trailer).
- No `/home/brett/` paths in any committed file or PR body.

### Rust code PRs (PR5, PR6)

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test --all` green (full unit + integration + transport suites).
- `cargo deny check` clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean.

### Workflow PRs (PR7, PR8)

- First CI run on PR head must complete green to prove SHA pins resolve and the new status-check context emits
  correctly.
- Re-applied rulesets verified by `gh api repos/brettdavies/bird/rulesets/<id>` diff vs on-disk JSON.

### Docs PRs (PR1, PR2, PR3, PR9, PR10)

- `markdownlint-cli2 "**/*.md"` clean.
- Manual visual diff review of every changed file before PR open.
- PR body filled per `.github/pull_request_template.md` (every section non-empty or explicitly marked N/A).

### CI-watch follow-through (global rule, every PR)

- After every push and every `gh pr create`/`merge`, watch the resulting CI runs to completion AND verify `gh pr view
  --json statusCheckRollup` shows every conclusion as `SUCCESS`. A completed watcher is not a green watcher. For PRs to
  `main` post-PR7, also watch the `homebrew-tap` cross-repo dispatch chain on tag pushes.

## Known risks and mitigations

- **`@main` → `@<sha>` pin side effects (PR7).** If `brettdavies/.github` HEAD changes between SHA resolution and PR
  merge, the pin captures one specific point in time. **Mitigation:** resolve the SHA inside the PR branch, run CI once
  before requesting review, and document the SHA's date in the trailing comment so reviewers can verify nothing material
  drifted.
- **Status-check context name mismatch (PR8).** The deployed ruleset references status-check contexts by exact string
  match. The job key in `guard-release-branch.yml` MUST stay `guard-release` so the context is `guard-release /
  check-release-branch-name`. **Mitigation:** the wrapper from xurl-rs already names the job correctly; copy verbatim.
  Run the workflow once on `dev` before re-applying the ruleset to confirm the literal context string surfaced in `gh
  api repos/.../check-runs`.
- **Dependabot churn after PR2.** Once `.github/dependabot.yml` lands, dependabot will open backlog PRs immediately.
  **Mitigation:** schedule PR2 to merge on a day with bandwidth to triage the first batch; group cargo PRs via `groups:`
  in the config if churn is high; cap to weekly cadence.
- **MSRV bump (PR5) breaks downstream consumers.** Any tool installing bird via `cargo install` will need Rust 1.94
  locally. **Mitigation:** call out the MSRV bump in the PR5 PR description, in the v0.2.0 (or whichever version cuts
  after this sprint) CHANGELOG, and in the README install section.
- **Branch rename leaves stale local clones broken.** Anyone with a local `development` checkout will hit upstream-gone
  errors after Phase 0. **Mitigation:** Phase 0 is operator-only; nobody else has clones. Document the rename in the
  v0.2.0 CHANGELOG.
- **`release/v0.1.4` stale branch (PR-X).** Currently under investigation. If the branch holds an in-flight version
  bump, deleting it loses work. **Mitigation:** investigation runs in parallel; do not touch the branch until it
  concludes.
- **Pre-push hook expansion (PR6) adds shellcheck + cross-clippy as required.** Contributors without `shellcheck` or the
  Windows MinGW toolchain installed will see skipped steps. **Mitigation:** the xurl-rs version skips gracefully with a
  one-line hint; preserve that behaviour. Document the optional dependencies in `AGENTS.md` (PR3).
- **Ruleset re-apply order (Phase 0 step 5 vs step 6).** Retarget `protect-dev` to `refs/heads/dev` BEFORE deleting
  `development` on the remote, otherwise the new `dev` branch is briefly unprotected. **Mitigation:** Phase 0 spells out
  the order explicitly; do not re-order.

## Open questions / TBD

1. **`release/v0.1.4` resolution (PR-X).** Open investigation; fills in once complete.
2. **CODEOWNERS placement.** xurl-rs has no CODEOWNERS yet; agentnative-cli likewise. PR2 places it at repo root by
   convention (matches the global supply-chain skill's expectation); if a reviewer prefers `.github/CODEOWNERS`, move it
   before merge.
3. **SECURITY.md / CONTRIBUTING.md template source.** Neither xurl-rs nor agentnative-cli ships these yet, so PR2 defers
   them. Track a follow-up to author a canonical template in `brettdavies/.github` and then backport to all three repos
   in one sweep.
