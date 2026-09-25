# Releasing `bird`

Operational runbook. Rationale lives in [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md). Pre-cut go/no-go checklist
lives in [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md); post-tag verification lives in
[`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md).

```text
feature branch → PR to dev (squash merge)
              → dev's tree overlaid onto a release/* branch cut from main
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

- Engineering docs: `docs/architecture/`, `docs/brainstorms/`, `docs/ideation/`, `docs/plans/`, `docs/research/`,
  `docs/reviews/`, `docs/solutions/`, and anything under `.context/`.

`scripts/release/guarded-paths.sh` prints the authoritative set, resolved from the workflow. Run it rather than trusting
this list, which is a reading aid. A path that stays off `main` but is not in the reusable workflow's base list is
registered through the caller's `extra_paths` input in `.github/workflows/guard-main-docs.yml`; entries are globs
(`**/` any depth, `*` and `?` within a segment, a trailing `/` guards the subtree).

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

**Branch naming**: `release/v<version>` or `release/v<version>-<slug>` (e.g. `release/v0.3.0`,
`release/v0.3.0-watchlist-api`). The `v<version>` prefix is required: `generate-changelog.py` extracts the version from
the branch name.

`main` and `dev` share no merge-base at all: every release squash-merges into `main`, so the two branches diverge in
history even as their content converges. Reconciling that with a merge, or a branch cut from `dev`, produces a pile of
rename/delete and lockfile conflicts that are artifacts of the lineage, not of the content shipping. The release branch
is therefore built as a **clean descendant of `main`** with `dev`'s tree overlaid on top, asserting the desired
end-state directly:

```bash
# 0. Nothing on main that dev never received (security PRs, hotfixes, config). Exits 1 while drift exists.
scripts/release/drift.sh

# 1. Branch from main, NOT dev.
git fetch origin
git checkout -B release/v0.3.0 origin/main

# 2. Overlay dev's entire tracked tree onto the main base. `checkout -- .` writes dev's
#    paths but does not delete files that exist on main and are absent on dev, so remove
#    those next (the 'D' rows are main-only files dev deleted).
git checkout origin/dev -- .
git diff --name-status origin/main origin/dev | grep '^D'
trash <each main-only file listed above>

# 3. Strip the paths guard-main-docs forbids on main. The set resolves from the workflow;
#    never restate it inline, because every hand-kept copy drifted from what CI enforces.
GUARDED="$(scripts/release/guarded-paths.sh)"
git ls-files | grep -E "$GUARDED" | xargs -r trash
git add -A                                                      # stages adds, mods, AND deletions

# 4. Bump the version in Cargo.toml, refresh Cargo.lock, and regenerate the completions
#    (catches any subcommand or flag change missed during dev).
sed -i 's/^version = ".*"/version = "0.3.0"/' Cargo.toml
cargo update -p bird
./scripts/generate-completions.sh

# 5. Generate CHANGELOG.md from the PRs merged into dev since the previous release. The
#    overlay commit carries no per-PR history, so the section is built from dev's PRs,
#    not from this branch's commits. Scrub the result via Vale + LanguageTool + unslop
#    (see § Prose scrubbing); fix findings on the upstream PR bodies and regenerate,
#    never by hand-editing CHANGELOG.md.
scripts/generate-changelog.py --from-dev-prs
git add -A

# 6. Verify before committing.
#    A: staged tree equals dev's minus the version files, the completions, and the
#       stripped guarded paths. Anything else printed here is a mistake.
git diff --cached --name-only origin/dev | grep -Ev "$GUARDED" \
  | grep -Ev '^(Cargo\.toml|Cargo\.lock|CHANGELOG\.md|completions/.*)$' \
  && echo "unexpected delta above; investigate" || echo "(clean: only intended deltas)"
#    B: no guarded path in the release tree.
git diff --cached --name-only origin/main | grep -E "$GUARDED" \
  && echo "LEAKED a guarded path: reset and redo" || echo "(no guarded paths)"
#    D: what this release ADDS to main. The leak check screens against the registered
#       set, so it is blind to a category nobody registered yet. Every docs/ entry and
#       every added markdown file needs a reason to ship, or it needs registering in the
#       workflow's extra_paths and removing from the branch.
git diff --cached --diff-filter=A --name-only origin/main | grep -E '(^docs/|\.md$)' | grep -Ev "$GUARDED" || echo "(none unguarded)"

# 7. Commit the overlay as one commit sitting directly on top of main, then run the
#    preflight gates against it.
git commit
cargo build --release
scripts/release/preflight.sh all

# 8. Push and open the PR. Scrub body in /tmp/ first.
git push -u origin release/v0.3.0
gh pr create --base main --head release/v0.3.0 --title "release: v0.3.0" --body-file /tmp/body.md
```

The result is a single commit whose diff against `main` is the release, with `main` as an ancestor, so the PR merges
with zero conflicts. When it merges, the tag push flow below picks up. Auto-delete removes `release/v0.3.0` from the
remote on merge. `dev` is untouched.

→ Rationale (why overlay, not merge; why cut from `main`):
[`RELEASES-RATIONALE.md` § Branching model](./RELEASES-RATIONALE.md#branching-model). CHANGELOG mechanics:
[`RELEASES-RATIONALE.md` § CHANGELOG generation](./RELEASES-RATIONALE.md#changelog-generation).

### Exception: cherry-pick

The overlay is the release construction for this repo. Cherry-picking the dev squash-commits onto the `origin/main`
base is the exception, kept for a repo that has a stated reason it cannot overlay (record it under Project specifics);
the per-PR changelog is not such a reason, since `--from-dev-prs` builds it from `dev` either way. When cherry-picking,
run the triple-diff verification:

```bash
# 2. List the dev commits not yet on main.
git log --oneline dev --not origin/main

# 3. Cherry-pick the ones to ship. Docs commits stay on dev.
git cherry-pick <sha1> <sha2> ...

# 4. Triple-diff verification.
GUARDED="$(scripts/release/guarded-paths.sh)"

git diff origin/main..HEAD --stat                                              # A: ship surface
git diff HEAD..origin/dev --name-only | grep -Ev "$GUARDED" || echo "(none)"   # B: no missed picks
git diff origin/dev..origin/main --stat | tail -5                              # C: phantom-commits sanity

# Re-confirm no guarded paths leaked.
git diff origin/main..HEAD --name-only \
  | grep -E "$GUARDED" \
  && echo "LEAKED: reset and redo" || echo "(clean)"

# D: what this release ADDS to main (see step 6 above for why).
git diff origin/main..HEAD --diff-filter=A --name-only | grep -E '(^docs/|\.md$)' | grep -Ev "$GUARDED" || echo "(none unguarded)"

# Patch-id cherry check (noisy in squash-merge workflow; triage per-line).
git cherry HEAD origin/dev | grep '^+' || echo "(none)"
```

Cherry-picks of PRs that touched guarded paths hit modify/delete or rename/delete conflicts, since those paths live on
`dev` but are blocked from `main`; resolve them per the next section. Steps 4 to 8 of the overlay recipe then apply
unchanged.

→ Triple-diff false-positive triage:
[`RELEASES-RATIONALE.md` § Triple-diff verification](./RELEASES-RATIONALE.md#triple-diff-verification).

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
rename detection` before the leak check.

## Tagging and publishing

After the `release/v<version> → main` PR merges, tag and push:

```bash
git checkout main && git pull
git tag -a -m "Release v0.3.0" v0.3.0
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
this repo, which idempotently flips `make_latest: true`. Run
[`scripts/release/postflight.sh all`](./RELEASES-POSTFLIGHT.md) to verify the chain.

→ Rationale (`make_latest` flow, target matrix, annotated-tag gotcha):
[`RELEASES-RATIONALE.md` § Release pipeline](./RELEASES-RATIONALE.md#release-pipeline).

### After publish: sync `dev` with the release

Once `finalize-release.yml` has flipped the GitHub Release to `published`, bring the release bookkeeping (`Cargo.toml`,
`Cargo.lock`, `CHANGELOG.md`) and any edit made on the release branch back to `dev` so the integration branch starts
from the released baseline. Preview it first: `--dry-run` prints what the sync would carry, creates no branch, and
leaves the tree clean.

```bash
scripts/sync-dev-after-release.sh v0.3.0 --dry-run
scripts/sync-dev-after-release.sh v0.3.0
```

The script cuts a `chore/sync-dev-after-v0.3.0` branch, writes the released version into `Cargo.toml` in place, copies
`CHANGELOG.md` from `main`, refreshes the crate's `Cargo.lock` entry from the synced manifest with
`cargo update --workspace --offline`, and opens a PR against `dev` with the version in its title; merge it once CI is
green. `scripts/release/postflight.sh backport` gates on that merged PR.

Every other path `main` and `dev` disagree about is discovered, bounded by the previous release tag, the last point the
two branches agreed:

- **release-prep**: `dev`'s copy is unchanged since the previous tag, so the difference is `main`'s alone. Adopted
  automatically.
- **contested**: both sides moved since the previous tag. Listed and withheld. `--only PATH` (repeatable) adopts the
  paths it names; `--include-contested` takes `main`'s copy of every one, which also deletes each file `dev` added that
  `main` lacks.

Guarded paths (the engineering docs `scripts/release/guarded-paths.sh` resolves) never enter discovery, so the sync
cannot remove them. The lock refresh reads only the local registry cache; when the lock does not resolve against the
synced manifest, the script exits 70 and commits nothing (`cargo fetch` fills the cache). After the commit, when the
sync carried `CHANGELOG.md` and `git-cliff` is installed, it runs `scripts/generate-changelog.py --dry-run` and, on a
mismatch, prints the generator's reason (a PR body edited after generation, or line wrapping only); that warning does
not fail the sync.

Never merge `main` into `dev` or push to `dev` directly: the two branches share no history, so the merge conflicts on
every file both sides touched, and a direct push bypasses `dev`'s required checks.

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

## Rollback

A bad release is rolled back at the registry and distribution surfaces first, then repaired in git. Rollback re-points
what users get; it does not revert history. After rolling back, land a `fix/*` or `revert` through the normal `dev` to
`release/*` to `main` flow so `main` matches what is live. Knowing the last-good identifier before the release goes out
is a [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md) gate.

```bash
# crates.io: yank the bad version so `cargo install bird` and lockfile resolution skip it.
#            Yank is reversible (`--undo`) and leaves the published files in place.
cargo yank --version 0.3.0 bird

# GitHub Release: re-point /releases/latest (and cargo-binstall) at the previous tag.
gh release edit v0.2.0 --latest

# Homebrew: revert the bump and bottle commits in brettdavies/homebrew-tap (each release
#           lands as `chore(bird): bump to vX.Y.Z` then `bird: add X.Y.Z bottle.`) so
#           `brew install` resolves the previous bottle, whose assets are still attached to
#           the previous GitHub Release.
git -C ~/dev/homebrew-tap revert <bottle-sha> <bump-sha>
```

→ Rationale: [`RELEASES-RATIONALE.md` § Rollback](./RELEASES-RATIONALE.md#rollback).

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
# scripts/generate-changelog.py --from-dev-prs   # for CHANGELOG.md
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

# Subsequent updates (replace by ID; find via `gh api repos/brettdavies/bird/rulesets`):
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
- [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md) (post-tag verification of the publish chain)
- [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md) (release-flow rationale: branching, PR body, pipeline, prose-check)
- [`.github/pull_request_template.md`](.github/pull_request_template.md) (PR body structure with changelog sections)
- [`AGENTS.md`](AGENTS.md) (project structure, daily development)
- [`README.md`](README.md) (install channels, CLI reference, X API integration)
