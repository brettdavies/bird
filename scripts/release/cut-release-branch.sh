#!/usr/bin/env bash
# Build the release branch: a clean descendant of main carrying dev's tree.
#
# Usage:
#   scripts/release/cut-release-branch.sh [options] <version>
#
# `main` and `dev` share only an ancient merge-base, because every release
# squash-merges into main. Merging them, or cutting the release branch from
# dev, produces rename/delete and lockfile conflicts that are artifacts of the
# lineage rather than of the content shipping. So the release branch is built as
# a descendant of main with dev's tree asserted on top, and this script does the
# mechanical part of that: branch, overlay, strip the guarded paths, stage, and
# verify.
#
# It stops before committing. The commit, the preflight run, the push and the PR
# stay with the operator, because those are the steps that publish.
#
# Options:
#   --dry-run        Print the plan and touch nothing. Implies --no-drift.
#   --base BRANCH    Branch the release descends from (default: main)
#   --head BRANCH    Branch whose tree is overlaid (default: dev)
#   --branch NAME    Release branch name (default: release/v<version>)
#   --no-drift       Skip the drift gate. Only when drift.sh already passed.
#   --no-changelog   Skip generate-changelog.py even when it is present.
#   -h, --help       This text.
#
# Exit codes:
#   0 = the branch is staged and every verification passed
#   1 = the drift gate, the changelog step, or a verification failed. Past the
#       drift gate the branch is left staged to inspect, and the script prints
#       the way back instead of the commit steps.
#   2 = setup error (dirty worktree, unknown ref, missing dependency, no
#       guarded-path pattern)
#
# The worktree must be clean. The overlay resets the index and the working tree
# to the head branch's tree, which would silently discard uncommitted work.
#
# What it deliberately does NOT do: bump the version carriers. That is
# project-specific (each repo records its own in RELEASES.md § Project
# specifics), and guessing at it is how a release ships a half-bumped manifest.
# The script prints the reminder and leaves the edit to the operator.

set -euo pipefail

# shellcheck disable=SC1091  # sibling _lib.sh, always vendored alongside
. "$(dirname "$0")/_lib.sh"

usage() {
  print_usage_header
  exit 2
}

DRY_RUN=""
BASE="main"
HEAD_BRANCH="dev"
BRANCH=""
RUN_DRIFT=1
RUN_CHANGELOG=1
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      RUN_DRIFT=0
      shift
      ;;
    --base)
      BASE="$2"
      shift 2
      ;;
    --head)
      HEAD_BRANCH="$2"
      shift 2
      ;;
    --branch)
      BRANCH="$2"
      shift 2
      ;;
    --no-drift)
      RUN_DRIFT=0
      shift
      ;;
    --no-changelog)
      RUN_CHANGELOG=0
      shift
      ;;
    -h | --help) usage ;;
    -*)
      echo "unknown flag: $1" >&2
      usage
      ;;
    *)
      [[ -n "$VERSION" ]] && {
        echo "unexpected argument: $1" >&2
        usage
      }
      VERSION="$1"
      shift
      ;;
  esac
done

[[ -n "$VERSION" ]] || usage
VERSION="${VERSION#v}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || {
  echo "version must be X.Y.Z (optionally with a -pre or +build suffix), got: $VERSION" >&2
  exit 2
}
BRANCH="${BRANCH:-release/v${VERSION}}"

require_bin git

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Echo under --dry-run, execute otherwise. Mirrors the skill's own run helper.
act() {
  if [[ -n "$DRY_RUN" ]]; then
    printf '  + %s\n' "$*"
  else
    "$@"
  fi
}

# The way back from a cut that stopped after branching. The worktree was clean
# when the cut started, so everything staged is this script's own output and
# discarding it loses nothing. A re-run refuses the dirty worktree a failed cut
# leaves behind, so the operator needs these two commands before anything else.
print_recovery() {
  RECOVERY_SHOWN=1
  cat <<RECOVER

Not committed: $1. Discard the cut, fix the cause, and re-run:

  git checkout -f $HEAD_BRANCH
  git branch -D $BRANCH
RECOVER
}

# Preconditions -------------------------------------------------------------

header "Preconditions"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
  echo "not inside a git work tree" >&2
  exit 2
}

if [[ -z "$DRY_RUN" ]] && [[ -n "$(git status --porcelain)" ]]; then
  gate_fail "clean worktree" "the overlay resets the index and working tree; commit or stash first"
  print_summary
  exit 2
fi
gate_pass "worktree clean"

act git fetch origin --quiet
for ref in "$BASE" "$HEAD_BRANCH"; do
  git rev-parse --verify --quiet "origin/$ref" >/dev/null || {
    echo "unknown ref: origin/$ref" >&2
    exit 2
  }
done
gate_pass "origin/$BASE and origin/$HEAD_BRANCH resolve"

# Gate: drift ---------------------------------------------------------------
#
# Nothing may sit on the base branch that the head branch never received, or
# the overlay reverts it. drift.sh owns that comparison.

if [[ "$RUN_DRIFT" -eq 1 ]]; then
  header "Drift"
  if [[ -x "$SCRIPT_DIR/drift.sh" ]]; then
    if "$SCRIPT_DIR/drift.sh" >/dev/null 2>&1; then
      gate_pass "no drift: origin/$BASE carries nothing origin/$HEAD_BRANCH lacks"
    else
      gate_fail "drift" "run scripts/release/drift.sh and backport what it names before cutting"
      print_summary
      exit 1
    fi
  else
    gate_skip "drift" "scripts/release/drift.sh not vendored"
  fi
fi

# Build the branch ----------------------------------------------------------

header "Branch"

act git checkout -B "$BRANCH" "origin/$BASE" --quiet
gate_pass "$BRANCH branched from origin/$BASE"

# From here a failing command leaves the half-built branch checked out and the
# worktree dirty, which a re-run refuses. Under set -e the script would end on
# git's own error alone, so any failing exit that has not already printed the
# way back prints it now.
RECOVERY_SHOWN=""
recover_on_failure() {
  local rc=$?
  [[ $rc -eq 0 || -n "$RECOVERY_SHOWN" ]] || print_recovery "a step failed (exit $rc)"
}
[[ -n "$DRY_RUN" ]] || trap recover_on_failure EXIT

# One command asserts the whole tree, including the deletions. The procedure
# this replaces used `git checkout origin/dev -- .` and then hand-removed the
# paths the base carries and the head deleted, read off a `--name-status` diff.
# That diff needed --no-renames to be correct: with rename detection on, a
# base-only file git pairs with any similar head-side addition reports as R
# rather than D, drops off the list, and ships to main as a file the head
# branch already deleted. read-tree asserts the tree directly, so there is no
# diff to mis-read and the hazard cannot occur.
act git read-tree -u --reset "origin/${HEAD_BRANCH}^{tree}"
gate_pass "overlaid origin/$HEAD_BRANCH tree onto the origin/$BASE base"

# Strip the guarded paths ---------------------------------------------------
#
# The set resolves from the guard-main-docs caller, never from a copy kept
# here: every hand-maintained restatement drifted from what CI enforces.

header "Guarded paths"

GUARDED=""
if [[ -x "$SCRIPT_DIR/guarded-paths.sh" ]]; then
  GUARDED=$("$SCRIPT_DIR/guarded-paths.sh" 2>/dev/null || true)
fi
if [[ -z "$GUARDED" ]]; then
  gate_fail "guarded-path list" "scripts/release/guarded-paths.sh resolved no pattern"
  print_summary
  [[ -n "$DRY_RUN" ]] || print_recovery "the guarded-path list did not resolve"
  exit 2
fi

if [[ -n "$DRY_RUN" ]]; then
  # Read the head branch's tree rather than the index, which dry-run never touched.
  leaked=$(git ls-tree -r --name-only "origin/${HEAD_BRANCH}" | grep -E "$GUARDED" || true)
else
  leaked=$(git ls-files | grep -E "$GUARDED" || true)
fi

if [[ -n "$leaked" ]]; then
  count=$(printf '%s\n' "$leaked" | wc -l | tr -d '[:space:]')
  if [[ -n "$DRY_RUN" ]]; then
    printf '  + git rm -qf -- %s guarded path(s)\n' "$count"
  else
    # -f because the overlay staged every one of these as an addition against
    # the base, and plain `git rm` refuses a path with staged changes. Nothing
    # is lost: the content is on the head branch and its remote, which is the
    # only place a guarded doc is supposed to live.
    printf '%s\n' "$leaked" | tr '\n' '\0' | xargs -0 git rm -qf --
  fi
  gate_pass "stripped $count guarded path(s) from the release tree"
else
  gate_pass "no guarded paths in the overlaid tree"
fi

act git add -A

# Changelog -----------------------------------------------------------------
#
# Built from the head branch's merged PRs, not from this branch's commits: the
# overlay is a single commit and carries no per-PR history.

if [[ "$RUN_CHANGELOG" -eq 1 ]]; then
  header "Changelog"
  if [[ -x scripts/generate-changelog.py ]]; then
    # --tag explicitly rather than read from the branch name, which --branch
    # lets the caller choose freely. --crate when the release package is a
    # workspace member, so the generator writes that member's changelog.
    read -ra crate_args <<<"$(changelog_crate_args)"
    changelog_args=(--from-dev-prs --tag "v${VERSION}" "${crate_args[@]}")
    if act scripts/generate-changelog.py "${changelog_args[@]}"; then
      act git add -A
      gate_pass "changelog regenerated from origin/$HEAD_BRANCH PRs"
    else
      gate_fail "generate-changelog.py ${changelog_args[*]}" "see its output above"
    fi
  else
    gate_skip "changelog" "scripts/generate-changelog.py not vendored"
  fi
fi

# Verify --------------------------------------------------------------------

header "Verify"

if [[ -n "$DRY_RUN" ]]; then
  gate_skip "verification" "dry run staged nothing to compare"
  print_summary
  printf '\nDry run. Re-run without --dry-run to build %s.\n' "$BRANCH"
  exit 0
fi

# A: the staged tree equals the head branch's, minus the version carriers this
# release edits and the guarded paths just stripped. Anything else is a mistake.
# A manifest or changelog inside a workspace member counts the same as the
# root's, so a member's version bump and changelog are not read as mistakes.
# This is the set a release branch legitimately edits, which is wider than
# release.env's VERSION_CARRIERS (that one answers what drift.sh may ignore).
VERSION_CARRIERS='^((.*/)?(Cargo\.toml|Cargo\.lock|package\.json|package-lock\.json|bun\.lock|pyproject\.toml|uv\.lock|VERSION|CHANGELOG\.md))$'
unexpected=$(git diff --cached --name-only "origin/$HEAD_BRANCH" \
  | grep -Ev "$GUARDED" | grep -Ev "$VERSION_CARRIERS" || true)
if [[ -n "$unexpected" ]]; then
  gate_fail "staged tree matches origin/$HEAD_BRANCH" \
    "unexpected delta:
$(printf '%s\n' "$unexpected" | sed 's/^/      /')"
else
  gate_pass "staged tree equals origin/$HEAD_BRANCH minus version carriers and guarded paths"
fi

# B: no guarded path is added or modified. --diff-filter=ACMR is load-bearing:
# a release that removes guarded docs the base still carries from before the
# guard existed lists every one of them as a plain diff entry, and an
# unfiltered grep reads that cleanup as a leak and aborts a correct release.
# guard-main-docs itself only flags added and modified files.
leak=$(git diff --cached --diff-filter=ACMR --name-only "origin/$BASE" | grep -E "$GUARDED" || true)
if [[ -n "$leak" ]]; then
  gate_fail "no guarded path in the release tree" \
    "leaked:
$(printf '%s\n' "$leak" | sed 's/^/      /')"
else
  gate_pass "no guarded path added or modified against origin/$BASE"
fi

# D: what this release ADDS to the base. The leak check screens the registered
# set, so it is blind to a category nobody registered yet. Reported, never
# failed: each entry needs a human reason to ship or a place in extra_paths.
# --no-renames, because rename detection reports a doc the head moved or
# rewrote from one the base carries as R, and the A filter then drops it.
added_docs=$(git diff --cached --no-renames --diff-filter=A --name-only "origin/$BASE" \
  | grep -E '(^docs/|\.md$)' | grep -Ev "$GUARDED" || true)
if [[ -n "$added_docs" ]]; then
  gate_skip "unguarded docs added to origin/$BASE" \
    "each needs a reason to ship, or registering in the workflow's extra_paths:
$(printf '%s\n' "$added_docs" | sed 's/^/      /')"
else
  gate_pass "this release adds no unguarded docs to origin/$BASE"
fi

print_summary

if [[ "$FAIL_COUNT" -ne 0 ]]; then
  print_recovery "a check above failed on $BRANCH"
  exit 1
fi

cat <<NEXT

Staged, not committed, on $BRANCH. Next:

  1. Bump the version carriers to $VERSION (see RELEASES.md § Project specifics).
  2. git add -A && git commit
  3. scripts/release/preflight.sh all
  4. git push -u origin $BRANCH
NEXT
