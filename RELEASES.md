# Releasing `bird`

Every change reaches production via this pipeline. Direct commits to `development` or `main` are not permitted — every
change has a PR number in its squash commit message, which keeps the history scannable, attributable, and
changelog-ready.

```text
feature branch → PR to development (squash merge)
              → cherry-pick to release/* branch
              → PR to main (squash merge)
              → tag push triggers crates.io publish + GitHub Release + Homebrew dispatch
```

## Branches

| Branch        | Role                                       | Lifetime                                    | Protection                                |
| ------------- | ------------------------------------------ | ------------------------------------------- | ----------------------------------------- |
| `main`        | Production. Only release commits.          | Forever.                                    | `.github/rulesets/protect-main.json`      |
| `development` | Integration. All feature PRs land here.    | Forever. Never delete.                      | `.github/rulesets/protect-dev.json`       |
| `feat/*`, `fix/*`, `chore/*`, `docs/*` | Feature work. | One PR's worth. Auto-deleted on merge.      | None — squash into development freely.    |
| `release/*`   | Head of a development → main PR.           | One release's worth. Auto-deleted on merge. | None.                                     |

`development` is a **forever branch**. Never delete it locally or remotely, even after a `release/* → main` merge. The
next release cycle reuses the same `development`. The repo's `deleteBranchOnMerge: true` setting doesn't touch
`development` as long as `development` is never the head of a PR — using a short-lived `release/*` head is what keeps
the setting compatible with a forever integration branch.

> Note: other brettdavies CLI repos (xurl-rs, agentnative) use `dev` as the integration branch name. bird uses
> `development` for historical reasons; the workflow is otherwise identical.

## Daily development (feature → development)

```bash
git checkout development && git pull
git checkout -b feat/short-description
# ... work ...
git push -u origin feat/short-description
gh pr create --base development --title "feat(scope): what changed"
# CI passes → squash-merge (PR_BODY becomes the development commit message)
```

- **Commit style**: [Conventional Commits](https://www.conventionalcommits.org/).
- **PR body**: follow `.github/pull_request_template.md`. The `## Changelog` section is the source of truth for
  user-facing release notes — `git-cliff` extracts these bullets verbatim into `CHANGELOG.md` during release prep.

## Releasing development to main

Engineering docs (`docs/plans/`, `docs/solutions/`, `docs/brainstorms/`) live on `development` only.
`guard-main-docs.yml` blocks them from reaching `main`. Use the release-branch cherry-pick pattern:

**Branch naming**: `release/v<version>` or `release/v<version>-<slug>` (e.g. `release/v0.1.4`,
`release/v0.2.0-watchlist-api`). The `v<version>` prefix is required —
`~/.claude/skills/rust-tool-release/scripts/generate-changelog.sh` extracts the version from the branch name.

```bash
# 1. Branch from main, NOT development. Branching from development causes add/add
#    conflicts when development and main have divergent histories (the post-squash-merge norm).
git fetch origin
git checkout -b release/v0.2.0 origin/main

# 2. List the development commits not yet on main:
git log --oneline development --not origin/main

# 3. Cherry-pick the ones you want to ship. Docs commits stay on development.
git cherry-pick <sha1> <sha2> ...

# 4. Verify no guarded paths leaked through:
git diff origin/main --stat
# If anything under docs/plans/, docs/solutions/, or docs/brainstorms/
# shows up, you cherry-picked a docs commit by mistake — reset and redo.

# 5. Bump version in Cargo.toml and commit:
#    sed -i 's/^version = ".*"/version = "0.2.0"/' Cargo.toml
#    cargo update -p bird   # refresh Cargo.lock
#    git add Cargo.toml Cargo.lock && git commit -m "chore: bump version to 0.2.0"

# 6. Regenerate completions (catches any subcommand/flag changes missed during dev):
~/.claude/skills/rust-tool-release/scripts/generate-completions.sh
git add completions/ && git commit -m "chore: regenerate shell completions" || true

# 7. Generate CHANGELOG.md (auto-detects version from branch name; CI enforces this):
~/.claude/skills/rust-tool-release/scripts/generate-changelog.sh
git add CHANGELOG.md && git commit -m "docs: update CHANGELOG.md for v0.2.0"

# 8. Push and open the PR:
git push -u origin release/v0.2.0
gh pr create --base main --head release/v0.2.0 --title "release: v0.2.0"
```

When the PR merges, the deploy / publish workflow picks up the push to `main`. Auto-delete removes the release branch
from the remote on merge. `development` is untouched.

### Why branch from main, not development

Branching from `development` and then `gio trash`-ing the guarded paths seems simpler but produces `add/add` merge
conflicts whenever `development` and `main` have diverged (which they always do after the first squash merge). The file
appears as "added" on both sides with different content. Always branch from `origin/main` and cherry-pick onto it.

## Tagging and publishing

After the `release/v<version> → main` PR merges, tag and push:

```bash
git checkout main && git pull
git tag -a -m "Release v0.2.0" v0.2.0
git push origin main --tags
```

> Always use annotated tags (`-a -m`). Bare `git tag <name>` silently fails with
> `fatal: no tag message?` on machines where `tag.gpgsign=true` is set globally
> (a brettdavies dotfile default). See
> [solutions: git tag fails with tag.gpgsign — use annotated tags](https://github.com/brettdavies/solutions-docs/blob/main/best-practices/git-tag-fails-with-tag-gpgsign-use-annotated-tags-2026-04-13.md).

The tag push triggers `.github/workflows/release.yml`, which calls the reusable
`brettdavies/.github/.github/workflows/rust-release.yml@main` and runs:

| Step | What |
|------|------|
| `check-version` | Verify the tag matches `Cargo.toml` version (gate). |
| `audit` | `cargo deny check` (license + advisory + ban). |
| `build` | Cross-compile for 5 targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. Each archive includes the `bird` binary, completions, README, and licenses. |
| `publish-crate` | `cargo publish` to crates.io via Trusted Publishing (OIDC, no static token). |
| `release` | Create a **draft** GitHub Release with all 5 archives + `sha256sum.txt`. |
| `homebrew` | Dispatch `update-formula` to `brettdavies/homebrew-tap` (formula `bird`). |

After the homebrew-tap workflow uploads bottles to the draft release, it dispatches `finalize-release` back to this
repo, which publishes the draft. End result: crate on crates.io, GitHub Release published, Homebrew formula updated with
bottles, all atomically advertised.

`cargo publish` runs BEFORE GitHub Release creation. If publish fails, no release is advertised and no Homebrew update
is triggered.

## CHANGELOG.md handling

`CHANGELOG.md` is a committed artifact managed during release prep, not auto-generated in CI.
`~/.claude/skills/rust-tool-release/scripts/generate-changelog.sh` requires `--tag vX.Y.Z` (or extracts the version from
the release branch name) and prepends a versioned section while preserving existing content. It also pulls `##
Changelog` sections from squashed PR bodies via the GitHub API for rich categorized entries.

CI enforces that `CHANGELOG.md` is modified in every PR to main (`ci / Changelog` required status check) and that it
contains a versioned section, not `[Unreleased]`. The release workflow extracts the latest section for the GitHub
Release body.

## crates.io publishing

Publishing uses Trusted Publishing via `rust-lang/crates-io-auth-action` — no static API token. OIDC exchanges a
short-lived GitHub Actions token for a ~30-minute crates.io token.

Trusted Publishing was configured after the v0.1.0 manual publish. To reconfigure:

1. `https://crates.io/settings/tokens/trusted-publishing`
2. Add trusted publisher: owner=`brettdavies`, repo=`bird`, workflow=`release.yml`
3. Enable "Enforce Trusted Publishing" to disable token-based publishing

## Required secrets

| Secret | Purpose | Lifecycle |
|--------|---------|-----------|
| `CI_RELEASE_TOKEN` | Fine-grained PAT, Contents R+W, Pull requests R+W. Used by `release.yml` to dispatch the Homebrew formula update. | Rotated annually (max 1 year). 1Password vault: `secrets-dev`. |

`GITHUB_TOKEN` is automatic; CI (`ci.yml`) only needs `contents: read` and uses no extra secrets.

## Distribution channels

| Channel | How |
|---------|-----|
| Homebrew | `brew install brettdavies/tap/bird` |
| Pre-built binary | Download from [GitHub Releases](https://github.com/brettdavies/bird/releases) |
| Rust crate | `cargo install bird` |
| Fast binary | `cargo binstall bird` |
| From source | `git clone && cargo build --release` |

## Related docs

- [`.github/pull_request_template.md`](.github/pull_request_template.md) — PR body structure with changelog sections
- [`README.md`](README.md) — install channels, CLI reference, X API integration
