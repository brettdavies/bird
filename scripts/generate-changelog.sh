#!/usr/bin/env bash
# Generate or update CHANGELOG.md using git-cliff with PR body expansion.
#
# Usage:
#   generate-changelog.sh [--tag vX.Y.Z] [repo-path]
#   generate-changelog.sh --check [repo-path]
#
# Options:
#   --tag vX.Y.Z   Override version tag (default: extracted from branch name)
#   --check        Verify CHANGELOG.md has a versioned section (exit 1 if only [Unreleased])
#
# The version tag is extracted from the branch name by matching the pattern
# release/vN.N.N (with optional suffix like release/v1.0.5:ci-migration).
# Use --tag to override when not on a release branch.
#
# Generates entries for commits since the last tag, prepends to existing
# CHANGELOG.md, then expands squash commit entries by fetching categorized
# changelog sections (### Added, ### Changed, ### Fixed, ### Documentation)
# from each PR body's ## Changelog section.
#
# Falls back to ## Changes (flat list) for PRs using the old template.
#
# Run this on a release branch before opening a PR to main.

set -euo pipefail

CHECK_MODE=false
REPO_PATH="."
TAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      CHECK_MODE=true
      shift
      ;;
    --tag)
      TAG="$2"
      shift 2
      ;;
    *)
      REPO_PATH="$1"
      shift
      ;;
  esac
done

cd "$REPO_PATH"

# Verify prerequisites
if [[ ! -f cliff.toml ]]; then
  echo "error: cliff.toml not found in $(pwd)" >&2
  exit 1
fi

if ! command -v git-cliff &>/dev/null; then
  echo "error: git-cliff is not installed" >&2
  echo "  Install: cargo install git-cliff" >&2
  echo "  Or:      brew install git-cliff" >&2
  exit 1
fi

if $CHECK_MODE; then
  if [[ ! -f CHANGELOG.md ]]; then
    echo "FAIL: CHANGELOG.md does not exist" >&2
    exit 1
  fi

  # Check for a versioned section (not just [Unreleased])
  LATEST_SECTION=$(awk '/^## \[/{print; exit}' CHANGELOG.md)
  if echo "$LATEST_SECTION" | grep -q '\[Unreleased\]'; then
    echo "FAIL: CHANGELOG.md has [Unreleased] instead of a versioned section" >&2
    echo "Run: generate-changelog.sh (on a release/vX.Y.Z branch)" >&2
    exit 1
  fi

  echo "OK: CHANGELOG.md has versioned section"
  exit 0
fi

# Extract version from branch name if --tag not provided
if [[ -z "$TAG" ]]; then
  BRANCH=$(git branch --show-current 2>/dev/null || true)
  if [[ "$BRANCH" =~ ^release/v([0-9]+\.[0-9]+\.[0-9]+) ]]; then
    TAG="v${BASH_REMATCH[1]}"
    echo "Detected version $TAG from branch $BRANCH"
  else
    echo "error: could not detect version from branch '$BRANCH'" >&2
    echo "Either use a release/vX.Y.Z branch or pass --tag vX.Y.Z" >&2
    exit 1
  fi
fi

# Ensure GitHub token is available for remote integration (PR links, authors)
if [[ -z "${GITHUB_TOKEN:-}" ]]; then
  if command -v gh &>/dev/null && gh auth status &>/dev/null 2>&1; then
    export GITHUB_TOKEN
    GITHUB_TOKEN=$(gh auth token)
  fi
fi

# Step 1: Run git-cliff to prepend entries tagged with the release version
CLIFF_ARGS=(--unreleased --tag "$TAG")
if [[ -f CHANGELOG.md ]]; then
  CLIFF_ARGS+=(--prepend CHANGELOG.md)
else
  CLIFF_ARGS+=(-o CHANGELOG.md)
fi
git cliff "${CLIFF_ARGS[@]}"

# Step 2: Expand squash commit entries using PR body changelog sections
OWNER=$(awk -F'"' '/^\[remote\.github\]/{found=1} found && /^owner/{print $2; exit}' cliff.toml)
REPO=$(awk -F'"' '/^\[remote\.github\]/{found=1} found && /^repo/{print $2; exit}' cliff.toml)

VERSION="${TAG#v}"

if [[ -z "$OWNER" || -z "$REPO" ]] || ! command -v gh &>/dev/null; then
  echo "Updated CHANGELOG.md (skipping PR expansion — missing [remote.github] or gh CLI)"
  echo ""
  echo "Next steps:"
  echo "  git add CHANGELOG.md"
  echo "  git commit -m 'docs: update CHANGELOG.md'"
  exit 0
fi

VERSION_SECTION=$(awk -v ver="$VERSION" '
  /^## \[/{
    if (found) exit
    if (index($0, "[" ver "]")) found=1
  }
  found{print}
' CHANGELOG.md)
PR_NUMBERS=$(echo "$VERSION_SECTION" | grep -oP '\(#\K\d+' | sort -un)

if [[ -z "$PR_NUMBERS" ]]; then
  echo "Updated CHANGELOG.md"
  echo ""
  echo "Next steps:"
  echo "  git add CHANGELOG.md"
  echo "  git commit -m 'docs: update CHANGELOG.md'"
  exit 0
fi

# Pass PR numbers as comma-separated arg to python
PR_LIST=$(echo "$PR_NUMBERS" | tr '\n' ',' | sed 's/,$//')

"$(dirname "$0")/generate-changelog.py" "$OWNER" "$REPO" "$PR_LIST" "CHANGELOG.md" "$VERSION" "$TAG"

echo "Updated CHANGELOG.md"
echo ""
echo "Next steps:"
echo "  git add CHANGELOG.md"
echo "  git commit -m 'docs: update CHANGELOG.md'"
