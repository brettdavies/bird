#!/usr/bin/env bash
# Backport release artifacts from main to dev after a release tag publishes.
#
# Writes the released version into every version carrier the repo has and
# copies CHANGELOG.md from main, then lands them via a PR against dev (per
# this repo's PR-only convention: direct commits to dev are not permitted).
#   - Version carriers, each updated in place when present: Cargo.toml (and
#     the crate's own entry in Cargo.lock), package.json, pyproject.toml,
#     VERSION (plain text, no leading "v").
#   - CHANGELOG.md, copied verbatim from origin/main when main carries one.
#     Main is fully authoritative for CHANGELOG; dev never edits it directly.
#
# Run AFTER:
#   1. The release/v* -> main PR has merged.
#   2. `git tag -a vX.Y.Z` has been pushed to origin.
#   3. The GitHub Release has been created.
#
# Usage:
#   ./scripts/sync-dev-after-release.sh v0.2.0
#
# Idempotent: safe to re-run. If dev already matches main on every synced
# file, the script exits 0 without creating a branch or PR.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 vX.Y.Z" >&2
  exit 64
fi

VERSION="$1"
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version must match vMAJOR.MINOR.PATCH (got: $VERSION)" >&2
  exit 64
fi
VERSION_NO_V="${VERSION#v}"

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree not clean -- commit or stash first" >&2
  git status --short >&2
  exit 65
fi

git fetch origin --tags --quiet

# Verify the release tag exists locally.
if ! git rev-parse --verify --quiet "refs/tags/$VERSION" >/dev/null; then
  echo "error: tag $VERSION not found locally -- run 'git fetch origin --tags' or verify the release published" >&2
  exit 66
fi

# Verify main is at or past the tag (i.e. release/* actually merged).
TAG_SHA="$(git rev-parse "$VERSION")"
if ! git merge-base --is-ancestor "$TAG_SHA" origin/main; then
  echo "error: tag $VERSION is not reachable from origin/main -- wait for release/v* to merge" >&2
  exit 66
fi

# Verify the GitHub Release exists and is not still a draft. The tag can exist
# (above check) while the GitHub Release was never created (or stayed draft),
# in which case consumers won't see the new version via `gh release` and the
# backport is premature.
if command -v gh >/dev/null 2>&1; then
  is_draft="$(gh release view "$VERSION" --json isDraft --jq .isDraft 2>/dev/null || true)"
  case "$is_draft" in
    false)
      ;;
    true)
      echo "error: GitHub Release $VERSION is still draft -- publish it first" >&2
      exit 67
      ;;
    "")
      echo "error: no GitHub Release for $VERSION -- create it with 'gh release create $VERSION'" >&2
      exit 67
      ;;
    *)
      echo "warning: unexpected isDraft value '$is_draft' for $VERSION -- proceeding" >&2
      ;;
  esac
else
  echo "warning: gh not on PATH -- skipping GitHub Release published-state check" >&2
fi

git switch dev
git pull --ff-only origin dev

# Cut a branch -- the repo's RELEASES.md and AGENTS.md ban direct commits to dev.
SYNC_BRANCH="chore/sync-dev-after-${VERSION}"

if git rev-parse --verify --quiet "$SYNC_BRANCH" >/dev/null; then
  echo "error: branch $SYNC_BRANCH already exists locally -- delete it or finish the prior run" >&2
  exit 68
fi
if git ls-remote --exit-code --heads origin "$SYNC_BRANCH" >/dev/null 2>&1; then
  echo "error: branch $SYNC_BRANCH already exists on origin -- check for an open PR or delete the remote branch" >&2
  exit 68
fi

git checkout -b "$SYNC_BRANCH"

# Writes VERSION_NO_V into the first `version = "..."` line of a TOML file,
# or the first `"version": "..."` entry of a JSON file, in place and without
# reformatting anything else.
set_version_line() {
  local file="$1" tmp
  tmp="$(mktemp)"
  awk -v v="$VERSION_NO_V" '
    !done && /^version = "/ { sub(/^version = "[^"]*"/, "version = \"" v "\""); done = 1 }
    !done && /"version": *"/ { sub(/"version": *"[^"]*"/, "\"version\": \"" v "\""); done = 1 }
    { print }
  ' "$file" >"$tmp"
  mv "$tmp" "$file"
}

# Cargo.lock carries the crate's own version too; a stale entry fails
# `cargo build --locked`. Update it for the crate named in Cargo.toml.
set_cargo_lock_version() {
  local crate tmp
  crate="$(grep -m1 '^name = ' Cargo.toml | sed -E 's/^name = "(.*)"/\1/')"
  [[ -n "$crate" && -f Cargo.lock ]] || return 0
  tmp="$(mktemp)"
  awk -v crate="$crate" -v v="$VERSION_NO_V" '
    /^name = "/ { current = $0 }
    /^version = "/ && current == "name = \"" crate "\"" { sub(/^version = "[^"]*"/, "version = \"" v "\""); current = "" }
    { print }
  ' Cargo.lock >"$tmp"
  mv "$tmp" Cargo.lock
}

# Every version carrier present gets the released number; the release commit
# on main bumped each of them.
SYNC_PATHS=()
if [[ -f Cargo.toml ]]; then
  set_version_line Cargo.toml
  SYNC_PATHS+=(Cargo.toml)
  if [[ -f Cargo.lock ]]; then
    set_cargo_lock_version
    SYNC_PATHS+=(Cargo.lock)
  fi
fi
if [[ -f package.json ]]; then
  set_version_line package.json
  SYNC_PATHS+=(package.json)
fi
if [[ -f pyproject.toml ]]; then
  set_version_line pyproject.toml
  SYNC_PATHS+=(pyproject.toml)
fi
if [[ -f VERSION || ${#SYNC_PATHS[@]} -eq 0 ]]; then
  printf '%s\n' "$VERSION_NO_V" >VERSION
  SYNC_PATHS+=(VERSION)
fi

# CHANGELOG.md from main (authoritative), once the changelog machinery has
# produced one there; until then the version carriers are the only synced
# artifacts.
if git cat-file -e origin/main:CHANGELOG.md 2>/dev/null; then
  git checkout origin/main -- CHANGELOG.md
  SYNC_PATHS+=(CHANGELOG.md)
fi

# `git checkout origin/main -- FILE` stages the file, so `git diff --quiet`
# (worktree against index) never sees that change and would report "no
# changes" with a differing CHANGELOG. `status --porcelain` sees staged,
# unstaged, and untracked alike, including a VERSION created on the first
# sync.
if [[ -z "$(git status --porcelain -- "${SYNC_PATHS[@]}")" ]]; then
  echo "no changes -- dev already in sync with $VERSION"
  git switch dev
  git branch -D "$SYNC_BRANCH"
  exit 0
fi

git add -- "${SYNC_PATHS[@]}"

COMMIT_MSG_FILE="$(mktemp -t "sync-dev-after-${VERSION}-commit.XXXXXX")"
cat >"$COMMIT_MSG_FILE" <<EOF
chore(release): backport $VERSION artifacts to dev

Brings dev's release bookkeeping current with the $VERSION release on
main: version carriers set to ${VERSION_NO_V}, and CHANGELOG.md copied
verbatim from origin/main when main carries one.

Synced: ${SYNC_PATHS[*]}
EOF
git commit --file "$COMMIT_MSG_FILE"
rm -f "$COMMIT_MSG_FILE"

# Post-sync sanity check: re-running generate-changelog.py against the current
# PR bodies should produce an identical CHANGELOG.md. Drift here means upstream
# PR bodies were edited after main's CHANGELOG.md was generated -- the
# backport brought the stale CHANGELOG over, and a future release-branch
# regen will surface unexpected diffs. Warn, do not fail; the backport is
# still correct against what main currently has.
if [[ -x scripts/generate-changelog.py ]] && command -v git-cliff >/dev/null 2>&1; then
  if scripts/generate-changelog.py --dry-run --tag "$VERSION" >/dev/null 2>&1; then
    echo "regen check: CHANGELOG.md matches what PR bodies would produce"
  else
    echo "warning: PR bodies have drifted from main's CHANGELOG.md for $VERSION" >&2
    echo "  re-run 'scripts/generate-changelog.py --dry-run --tag $VERSION' to see the diff" >&2
    echo "  fix by regenerating CHANGELOG.md on a follow-up release branch" >&2
  fi
fi

# Push the sync branch and open a PR. Direct merge to dev is not permitted.
if ! command -v gh >/dev/null 2>&1; then
  echo "error: gh not on PATH -- branch is committed locally as $SYNC_BRANCH; push and PR by hand" >&2
  exit 69
fi

git push -u origin "$SYNC_BRANCH"

# PR body composed at runtime; written to mktemp so gh pr create reads it
# via --body-file rather than an inline heredoc.
PR_BODY_FILE="$(mktemp -t "sync-dev-after-${VERSION}-pr-body.XXXXXX")"
trap 'rm -f "$PR_BODY_FILE"' EXIT

TAG_SHORT="$(git rev-parse --short "$TAG_SHA")"
SYNC_LIST="$(printf '\`%s\`, ' "${SYNC_PATHS[@]}")"
SYNC_LIST="${SYNC_LIST%, }"
SYNC_BULLETS="$(for f in "${SYNC_PATHS[@]}"; do
  if [[ "$f" == CHANGELOG.md ]]; then
    printf -- '- `%s` (verbatim copy from `origin/main` at `%s`)\n' "$f" "$TAG_SHORT"
  else
    printf -- '- `%s` (version set to `%s`)\n' "$f" "$VERSION_NO_V"
  fi
done)"

cat >"$PR_BODY_FILE" <<EOF
## Summary

Backports the v${VERSION_NO_V} release-prep state from \`main\` so dev's version carriers match the released
number and the v${VERSION_NO_V} CHANGELOG section sits at the top of dev's \`CHANGELOG.md\` going forward.

Source: tag \`${VERSION}\` at \`${TAG_SHORT}\` on \`main\`. Files synced: ${SYNC_LIST}.

Generated by \`scripts/sync-dev-after-release.sh\`. Run idempotently per release: if dev already matches main on
these files, the script exits 0 without creating this PR.

## Changelog

This PR is producer-side scaffolding and does not change anything users see; no \`## Changelog\` bullets to
extract.

## Type of Change

- [x] \`chore\`: Maintenance tasks (release backport)

## Related Issues/Stories

- Story: n/a
- Issue: n/a
- Architecture: n/a
- Related PRs: the release/${VERSION} PR into main

## Testing

- [x] Manual testing completed

The script's preflight verified: the release tag exists, \`origin/main\` is at or past it, and the GitHub Release
is not still a draft. \`generate-changelog.py --dry-run\` was also invoked post-sync to check for PR-body drift
against the backported CHANGELOG; see this PR's stderr for any drift warnings.

## Files Modified

**Modified:**

${SYNC_BULLETS}

**Created:**

- None.

**Renamed:**

- None.

**Deleted:**

- None.

## Breaking Changes

- [x] No breaking changes

## Deployment Notes

- [x] No special deployment steps required
EOF

gh pr create \
  --base dev \
  --head "$SYNC_BRANCH" \
  --title "chore(release): sync dev after ${VERSION}" \
  --body-file "$PR_BODY_FILE"

echo "PR opened against dev; review and merge once CI is green."
