# Releasing `bird`

Operational runbook. Rationale lives in [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md). Pre-cut go/no-go checklist
lives in [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md).

```text
feature branch → PR to dev (squash merge)
              → cherry-pick to release/* branch
              → PR to main (squash merge)
              → tag push triggers crates.io publish + GitHub Release + Homebrew dispatch
```

Direct commits to `dev` or `main` are not permitted: every change has a PR number in its squash commit message.

## Branches

| Branch                                 | Role                                    | Lifetime                                    | Protection                           |
| -------------------------------------- | --------------------------------------- | ------------------------------------------- | ------------------------------------ |
| `main`                                 | Production. Only release commits.       | Forever.                                    | `.github/rulesets/protect-main.json` |
| `dev`                                  | Integration. All feature PRs land here. | Forever. Never delete.                      | `.github/rulesets/protect-dev.json`  |
| `feat/*`, `fix/*`, `chore/*`, `docs/*` | Feature work.                           | One PR's worth. Auto-deleted on merge.      | None. Squash into dev freely.        |
| `release/*`                            | Head of a dev → main PR.                | One release's worth. Auto-deleted on merge. | None.                                |

→ Rationale: [`RELEASES-RATIONALE.md` § Branching model](./RELEASES-RATIONALE.md#branching-model).

## Daily development (feature → dev)

```bash
git checkout dev && git pull
git checkout -b feat/short-description
# ... work ...
git push -u origin feat/short-description
gh pr create --base dev --title "feat(scope): what changed"
# CI passes → squash-merge (PR_BODY becomes the dev commit message)
```

- **Commit style**: [Conventional Commits](https://www.conventionalcommits.org/).
- **PR body**: follow `.github/pull_request_template.md`. See [§ PR body](#pr-body).
- **PR body prose scrub**: see [§ Prose scrubbing](#prose-scrubbing).

### Dev-direct exception

Paths that live only on `dev` and never ship to `main` can be committed directly to `dev` without a feature branch or
PR. The `guard-main-docs` workflow blocks them from `main` PRs regardless. The exception applies to:

- Engineering docs: `docs/brainstorms/`, `docs/ideation/`, `docs/plans/`, `docs/research/`, `docs/reviews/`,
  `docs/solutions/`, and anything under `.context/`.

The standard feature → PR → squash-merge flow remains required for everything else, including consumer-facing markdown
(README, AGENTS, CONTRIBUTING, CHANGELOG, in-repo runbooks).

## PR body

Every PR (feature, fix, docs, release) uses `.github/pull_request_template.md` verbatim. Six sections, no inventions:
`## Summary`, `## Changelog`, `## Type of Change`, `## Related Issues/Stories`, `## Files Modified`, `## Testing`.

- **No explainer prose anywhere in the body.** User-facing substance only.
- **Summary describes the net diff only**: what merged `main` looks like vs the base branch. Not commit history,
  intermediate state, or cherry-pick mechanics.
- **Zero verification artifacts in the body.** No triple-diff stats, leak-check output ("`guard-main-docs` runs clean"),
  patch-id cherry-check counts, pre-push gate results, CI status, or prose-scrub findings. Anomalies get fixed before
  push, not audit-trailed.
- **Changelog** subsections (`### Added` / `### Changed` / `### Fixed` / `### Documentation`): 1-5 bullets each, delete
  empty subsections, each bullet starts with a verb.
- **Type of Change**: one checkbox. Prefer `feat`/`fix` over `chore` for any user-observable change.
- **Related Issues/Stories**: four labels (`Story:` / `Issue:` / `Architecture:` / `Related PRs:`). All four required
  even when empty (`- None.` / `n/a`).
- **Files Modified**: four sub-headers (`Modified` / `Created` / `Renamed` / `Deleted`). All four required even when
  empty.
- **No AI attribution** in commits or PR bodies.
- **No hard line wraps**: one logical line per paragraph or bullet.

→ Rationale: [`RELEASES-RATIONALE.md` § PR body conventions](./RELEASES-RATIONALE.md#pr-body-conventions).

## Releasing dev to main

Before cutting a release branch, walk [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md) end-to-end. Any unchecked item
holds the release.

Engineering docs (`docs/plans/`, `docs/solutions/`, `docs/brainstorms/`, `docs/reviews/`) live on `dev` only.
`guard-main-docs.yml` blocks them from reaching `main`, and `guard-release-branch.yml` rejects any PR to main whose head
isn't `release/*`.

**Branch naming**: `release/v<version>` or `release/v<version>-<slug>` (e.g. `release/v0.2.0`,
`release/v0.2.0-watchlist-api`). The `v<version>` prefix is required: `generate-changelog.sh` extracts the version from
the branch name.

```bash
# 1. Branch from main, NOT dev.
git fetch origin
git checkout -b release/v0.2.0 origin/main

# 2. List the dev commits not yet on main.
git log --oneline dev --not origin/main

# 3. Cherry-pick the ones to ship. Docs commits stay on dev.
git cherry-pick <sha1> <sha2> ...

# 4. Triple-diff verification.
git diff origin/main..HEAD --stat                                              # A: ship surface
git diff HEAD..origin/dev --name-only | grep -v '^docs/' || echo "(none)"      # B: no missed picks
git diff origin/dev..origin/main --stat | tail -5                              # C: phantom-commits sanity

# Re-confirm no guarded paths leaked.
git diff origin/main..HEAD --name-only \
  | grep -E '^(docs/plans|docs/brainstorms|docs/ideation|docs/reviews|docs/solutions|\.context)' \
  && echo "LEAKED — reset and redo" || echo "(clean)"

# Patch-id cherry check (noisy in squash-merge workflow; triage per-line).
git cherry HEAD origin/dev | grep '^+' || echo "(none)"

# 5. Bump version in Cargo.toml and commit.
sed -i 's/^version = ".*"/version = "0.2.0"/' Cargo.toml
cargo update -p bird   # refresh Cargo.lock
git add Cargo.toml Cargo.lock && git commit -m "chore: bump version to 0.2.0"

# 6. Regenerate completions (catches any subcommand/flag changes missed during dev).
./scripts/generate-completions.sh
git add completions/ && git commit -m "chore: regenerate shell completions" || true

# 7. Generate CHANGELOG.md (auto-detects version from branch name; CI enforces this).
./scripts/generate-changelog.sh

# 8. Scrub CHANGELOG.md via Vale + LanguageTool + unslop. See § Prose scrubbing.
#    Fix findings on upstream PR bodies, never by hand-editing CHANGELOG.md. When clean:
git add CHANGELOG.md && git commit -m "docs: update CHANGELOG.md for v0.2.0"

# 9. Push and open the PR. Scrub body in /tmp/ first.
git push -u origin release/v0.2.0
gh pr create --base main --head release/v0.2.0 --title "release: v0.2.0" --body-file /tmp/body.md
```

When the PR merges, the deploy / publish workflow picks up the push to `main`. Auto-delete removes `release/v0.2.0` from
the remote on merge. `dev` is untouched.

→ Rationale + triple-diff false-positive triage:
[`RELEASES-RATIONALE.md` § Triple-diff verification](./RELEASES-RATIONALE.md#triple-diff-verification). CHANGELOG
mechanics: [`RELEASES-RATIONALE.md` § CHANGELOG generation](./RELEASES-RATIONALE.md#changelog-generation).

### Cherry-pick conflicts on guarded paths

Cherry-picks of feature PRs that touched `docs/plans/` / `docs/brainstorms/` / `docs/ideation/` / `docs/reviews/` /
`docs/solutions/` / `.context/` files will hit modify/delete conflicts on the release branch. Those paths exist on `dev`
but are blocked from `main` by `guard-main-docs.yml`, so the cherry-pick sees them as "deleted in HEAD, modified in
`<commit>`". A PR that renames such a file also produces rename/delete conflicts on the same paths.

Resolution (the standard `git rm` is denied by repo policy; use the plumbing form):

```bash
# 1. Mark every unmerged guarded path as deleted in the index.
git update-index --remove $(git diff --name-only --diff-filter=U)

# 2. Trash the orphan worktree files left by the rename target side.
#    `trash` is a zsh alias to `gio trash`; xargs does not expand aliases,
#    so call `gio trash` directly when piping or batching.
gio trash docs/plans/<leftover-paths>.md

# 3. Continue the cherry-pick.
git cherry-pick --continue --no-edit
```

Repeat per conflicting commit. After all picks land, run `git ls-files docs/plans/ docs/brainstorms/`. If anything
remains, drop it with the same two-step pattern and commit as `chore(release): drop stray plan spikes from cherry-pick
rename detection` before step 4's leak check.

## Tagging and publishing

After the `release/v<version> → main` PR merges, tag and push:

```bash
git checkout main && git pull
git tag -a -m "Release v0.2.0" v0.2.0
git push origin main --tags
```

Always use annotated tags (`-a -m`). The tag push triggers `.github/workflows/release.yml`, which calls the reusable
`brettdavies/.github/.github/workflows/rust-release.yml` and runs:

| Step            | What                                                                                                                                                                                                                                        |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check-version` | Verify the tag matches `Cargo.toml` version (gate).                                                                                                                                                                                         |
| `audit`         | `cargo deny check` (license + advisory + ban).                                                                                                                                                                                              |
| `build`         | Cross-compile binaries for 5 targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. Each archive includes the `bird` binary, completions, and licenses. |
| `publish-crate` | `cargo publish` to crates.io via Trusted Publishing (OIDC, no static token after first publish).                                                                                                                                            |
| `release`       | Create a **non-draft** GitHub Release with `make_latest: false`. Includes all 5 archives + `sha256sum.txt`.                                                                                                                                 |
| `homebrew`      | Dispatch `update-formula` to `brettdavies/homebrew-tap` (formula name: `bird`).                                                                                                                                                             |

After the homebrew-tap workflow uploads bottles to this repo's release assets, it dispatches `finalize-release` back to
this repo, which idempotently flips `make_latest: true`.

→ Rationale (`make_latest` flow, target matrix, annotated-tag gotcha):
[`RELEASES-RATIONALE.md` § Release pipeline](./RELEASES-RATIONALE.md#release-pipeline).

### After publish: sync `dev` with the release

Once `finalize-release.yml` has flipped the GitHub Release to `published`, merge `main` back into `dev` so the release
bookkeeping (`Cargo.toml` version, `Cargo.lock`, `CHANGELOG.md`) lands on the integration branch:

```bash
git checkout dev && git pull
git merge --no-ff origin/main -m "Merge remote-tracking branch 'origin/main' into dev"
git push origin dev
```

→ Rationale: [`RELEASES-RATIONALE.md` § Release pipeline](./RELEASES-RATIONALE.md#release-pipeline).

### First-time publish (one-time)

The initial crate publish requires a regular crates.io API token (Trusted Publishing needs the crate to exist first).
bird completed this step with `v0.1.0`. To configure Trusted Publishing for subsequent releases:

1. Verify your email on crates.io (`https://crates.io/settings/profile`).
2. `cargo publish` locally with `CARGO_REGISTRY_TOKEN` set.
3. Configure Trusted Publishing on crates.io: `https://crates.io/settings/tokens/trusted-publishing` → add
   `brettdavies/bird`, workflow `release.yml`.
4. Enable "Enforce Trusted Publishing" to block token-based publishes.
5. Remove the `CARGO_REGISTRY_TOKEN` repository secret.

Subsequent releases use the OIDC flow built into `release.yml`: no static token in CI.

## Prose scrubbing

Three release-flow artifacts live outside any automated prose check and need a manual scrub before they ship:

- PR bodies (`gh pr create` / `gh pr edit` send body text directly to GitHub).
- `CHANGELOG.md` (a generated artifact built from upstream PR bodies).
- Release-PR bodies (composed after `CHANGELOG.md` has been generated).

The canonical Vale + LanguageTool rule packs live in the agentnative-spec repo at
[`~/dev/agentnative-spec/docs/architecture/voice-enforcement.md`](../agentnative-spec/docs/architecture/voice-enforcement.md).
This repo does not ship a local copy; point Vale at the spec checkout via `--config`.

```bash
# 1. Save the artifact to /tmp/.
gh pr view <num> --json body --jq .body > /tmp/body.md         # for PR body edits
# cp CHANGELOG.md /tmp/body.md                                 # for changelog scrub

# 2. Vale (against the spec's rule packs).
vale --no-global --config ~/dev/agentnative-spec/.vale.ini --output=line --minAlertLevel=error /tmp/body.md

# 3. LanguageTool grammar check via lt_check (~/dotfiles/config/shell/languagetool.sh).
#    Skips cleanly if LT is unreachable. Inspect: `lt_rules`, `lt_info`.
lt_check /tmp/body.md

# 4. unslop (em-dash density and AI-unique structural patterns).
~/.claude/skills/unslop/scripts/score.py /tmp/body.md

# 5. Apply fixes per finding. Re-run until 0 blocking and unslop score is 0.

# 6. Apply the cleaned version.
gh pr edit <num> --body-file /tmp/body.md     # for PR body edits
# ./scripts/generate-changelog.sh   # for CHANGELOG.md
```

For a `CHANGELOG.md` finding, fix the upstream PR body and regenerate. Hand-editing `CHANGELOG.md` directly produces
drift the next regeneration overwrites.

→ Rationale + which artifacts need this:
[`RELEASES-RATIONALE.md` § Prose scrubbing scope](./RELEASES-RATIONALE.md#prose-scrubbing-scope).

## Branch protection

Two rulesets are committed under `.github/rulesets/` and applied to the repo via the GitHub API:

- `protect-main.json` (required signatures, linear history, squash-only merges via PR, required status checks (`ci /
  Fmt, clippy, test`, `ci / Package check`, `ci / Security audit (bans licenses sources)`, `ci / Changelog`, `guard-docs
  / check-forbidden-docs`, `guard-provenance / check-provenance`, `guard-release / check-release-branch-name`),
  creation/deletion blocked, non-fast-forward blocked).
- `protect-dev.json` (required signatures, deletion blocked, non-fast-forward blocked). PR-only norm is convention +
  `guard-release-branch` on the main side.

### Applying changes

```bash
# First apply (creating a ruleset):
gh api -X POST repos/brettdavies/bird/rulesets --input .github/rulesets/protect-dev.json

# Subsequent updates (replace by ID — find via `gh api repos/brettdavies/bird/rulesets`):
gh api -X PUT repos/brettdavies/bird/rulesets/<id> --input .github/rulesets/protect-main.json
```

→ Status-check context strings (inline vs reusable):
[`RELEASES-RATIONALE.md` § Status-check context strings](./RELEASES-RATIONALE.md#status-check-context-strings).

## Required secrets

| Secret                 | Purpose                                                                                                           | Lifecycle                                         |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| `CI_RELEASE_TOKEN`     | Fine-grained PAT, Contents R+W, Pull requests R+W. Used by `release.yml` to dispatch the Homebrew formula update. | Rotated annually. 1Password vault: `secrets-dev`. |
| `CARGO_REGISTRY_TOKEN` | crates.io API token. Required only for the first publish.                                                         | Removed after Trusted Publishing was enforced.    |

`GITHUB_TOKEN` is automatic; CI (`ci.yml`) only needs `contents: read` and uses no extra secrets.

## Distribution channels

| Channel          | How                                                                           |
| ---------------- | ----------------------------------------------------------------------------- |
| Homebrew         | `brew install brettdavies/tap/bird`                                           |
| Pre-built binary | Download from [GitHub Releases](https://github.com/brettdavies/bird/releases) |
| Rust crate       | `cargo install bird`                                                          |
| Fast binary      | `cargo binstall bird`                                                         |
| From source      | `git clone && cargo build --release`                                          |

## Related docs

- [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md) (pre-cut go/no-go checklist gating release-branch creation)
- [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md) (release-flow rationale: branching, PR body, pipeline, prose-check)
- [`.github/pull_request_template.md`](.github/pull_request_template.md) (PR body structure with changelog sections)
- [`AGENTS.md`](AGENTS.md) (project structure, daily development)
- [`README.md`](README.md) (install channels, CLI reference, X API integration)
