#!/usr/bin/env bash
# Shared bash utilities for the release-(pre|post)flight orchestrators and any
# project-authored surface-smoke suite that wants to participate in the
# orchestrator's PASS/FAIL/SKIP accounting. Source via:
#
#   . "$(dirname "$0")/_lib.sh"
#
# Provides:
#   - Color helpers (C_RED, C_GRN, C_YLW, C_RST, C_BLD), empty when stdout is
#     not a TTY, so output is clean in CI logs.
#   - Gate counters (PASS_COUNT, FAIL_COUNT, SKIP_COUNT) and emitters
#     (gate_pass, gate_fail, gate_skip).
#   - Section header helper, and a path-list renderer for gate detail
#     (count_and_list).
#   - Dependency checks (require_bin, have_bin).
#   - 1Password helper (read_1p) routing through the brettdavies 1password skill.
#   - Final summary printer (print_summary).
#   - Sub-script delegation (delegate_to_subscript) for surface-smoke aggregation.
#
# Idempotent: safe to source multiple times. Re-sourcing is a no-op so the
# `readonly` declarations on color constants don't fail.

if [[ -n "${_RELEASE_LIB_SOURCED:-}" ]]; then
  return 0
fi
_RELEASE_LIB_SOURCED=1

# Require bash >= 4.4: associative arrays, mapfile, and safe empty-array
# expansion under `set -u`. Sourced, so return (not exit) to avoid killing an
# interactive shell; the sourcing script aborts on the non-zero return.
if ((BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4))); then
  printf 'error: bash >= 4.4 required, but this is bash %s.\n' "${BASH_VERSION:-unknown}" >&2
  printf 'Install a newer bash: brew install bash\n' >&2
  # shellcheck disable=SC2317  # `exit` runs only when executed rather than sourced
  return 1 2>/dev/null || exit 1
fi

# Color helpers --------------------------------------------------------------

if [[ -t 1 ]]; then
  C_RED=$'\033[31m'
  C_GRN=$'\033[32m'
  C_YLW=$'\033[33m'
  C_RST=$'\033[0m'
  C_BLD=$'\033[1m'
else
  C_RED='' C_GRN='' C_YLW='' C_RST='' C_BLD=''
fi
readonly C_RED C_GRN C_YLW C_RST C_BLD

# Gate counters and emitters -------------------------------------------------

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

gate_pass() {
  printf "  %s✓%s %s\n" "$C_GRN" "$C_RST" "$1"
  PASS_COUNT=$((PASS_COUNT + 1))
}
gate_fail() {
  printf "  %s✗%s %s\n    %s\n" "$C_RED" "$C_RST" "$1" "${2:-}"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}
gate_skip() {
  printf "  %s⊝%s %s: %s\n" "$C_YLW" "$C_RST" "$1" "${2:-not yet ready}"
  SKIP_COUNT=$((SKIP_COUNT + 1))
}
header() { printf "\n%s== %s ==%s\n" "$C_BLD" "$1" "$C_RST"; }

# Renders a newline-separated path list as gate detail: the count first, then
# the paths. A gate that printed a bare `head -N` told the operator neither how
# many there were nor that the list was cut, so a release deliberately
# diverging from dev read as three stray files instead of sixteen.
#
# Args: $1 newline-separated list; $2 optional cap (default 20).
count_and_list() {
  local list=$1 cap=${2:-20} n shown remainder
  n=$(printf '%s\n' "$list" | grep -c . || true)
  shown=$(printf '%s\n' "$list" | grep . | head -"$cap" | tr '\n' ' ')
  remainder=$((n - cap))
  if [[ "$remainder" -gt 0 ]]; then
    printf '%s file(s): %s(+%s more)' "$n" "$shown" "$remainder"
  else
    printf '%s file(s): %s' "$n" "$shown"
  fi
}

# Release package ------------------------------------------------------------

# Per-repo release configuration, sourced when present. preflight, postflight
# and sync-dev are three separate entry points, so a value declared in one of
# them is missing from the other two; this file is the one place all three read.
# A single-package repo ships none and needs none.
# shellcheck source=/dev/null
[[ -f "${BASH_SOURCE[0]%/*}/release.env" ]] && . "${BASH_SOURCE[0]%/*}/release.env"

# The manifest whose `[package]` version a `vX.Y.Z` tag names. A single-package
# repo carries it at the root. A workspace root is a virtual manifest with no
# version of its own, so the crate the tag releases is the carrier and
# release.env names it.
RELEASE_MANIFEST="${RELEASE_MANIFEST:-Cargo.toml}"

# Auto-detects rather than trusting RELEASE_MANIFEST blindly: a root manifest
# with a [package] table is the carrier whatever the variable says, so a
# single-package repo cannot be misconfigured into reading the wrong file.
release_manifest() {
  if grep -q '^\[package\]' Cargo.toml 2>/dev/null; then
    echo Cargo.toml
  else
    echo "$RELEASE_MANIFEST"
  fi
}

# The changelog the release notes are cut from. A workspace member keeps its
# own beside its manifest, so this is not always the repository root's.
release_changelog() {
  echo "${RELEASE_CHANGELOG:-CHANGELOG.md}"
}

# The `[package] version` the tag must match.
project_version() {
  grep -m1 '^version = ' "$(release_manifest)" | sed -E 's/^version = "(.*)"/\1/'
}

# The `[package] name` of the release package.
project_crate() {
  awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/          { in_pkg = 0 }
    in_pkg && /^name = / { sub(/^name = "/, ""); sub(/".*/, ""); print; exit }
  ' "$(release_manifest)"
}

# The newest tag on the binary's `vX.Y.Z` line. A workspace's library tags
# (`<crate>-vX.Y.Z`) sort into the same list and would name the wrong crate, so
# the pattern is anchored to a bare `v` followed by a digit.
last_release_tag() {
  git tag --list 'v[0-9]*' --sort=-version:refname | head -n 1
}

# Semver helpers -------------------------------------------------------------

# Which bump the working tree claims over a baseline tag, for the release type
# cargo-semver-checks validates against. Compares Cargo.toml's version to the
# tag rather than guessing from commit markers: a break reaches the branch
# whether or not its commit carried a `!` marker, so the version is the only
# honest statement of what this release claims to be.
#
# Rust-only, and callers gate on Cargo.toml themselves.
# Seconds since the epoch for a YYYY-MM-DD date, on GNU and BSD alike. GNU date
# parses a free-form date with -d; BSD date rejects -d outright and wants -j
# with an explicit input format. Try GNU first, since a Linux CI runner is the
# common case, and fall back rather than probing for a version string.
epoch_of_date() {
  date -d "$1" +%s 2>/dev/null || date -j -f "%Y-%m-%d" "$1" +%s 2>/dev/null
}

# A script's own header comment block, rendered as help text: every line after
# the shebang up to the first line that is not a comment, with the leading `# `
# stripped. Reading the block's extent means a header can grow without anyone
# remembering to widen a line range.
#
# awk, not `sed -n '/^[^#]/q;2,$p'`: BSD and GNU sed disagree about `q` inside a
# range, and this form needs no flag either dialect argues over.
print_usage_header() {
  awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "${1:-$0}"
}

semver_release_type() {
  local baseline="${1#v}" current
  current=$(project_version)
  # An unresolved version must not reach the comparison below. Empty, it
  # differs from every baseline major and returns `major`, which is the one
  # answer that lets cargo-semver-checks accept any break at all: the gate
  # would report green while validating nothing. A virtual workspace root with
  # RELEASE_MANIFEST left at its default lands exactly here.
  if [[ -z "$current" ]]; then
    echo "no version in $(release_manifest); set RELEASE_MANIFEST to the crate the tag releases" >&2
    return 1
  fi
  local b_major="${baseline%%.*}" c_major="${current%%.*}"
  local b_rest="${baseline#*.}" c_rest="${current#*.}"
  local b_minor="${b_rest%%.*}" c_minor="${c_rest%%.*}"
  if [[ "$c_major" != "$b_major" ]]; then
    echo major
  elif [[ "$c_minor" != "$b_minor" ]]; then
    echo minor
  else
    echo patch
  fi
}

# Dependency checks ----------------------------------------------------------

require_bin() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing dependency: $1" >&2
    exit 2
  }
}

have_bin() {
  command -v "$1" >/dev/null 2>&1
}

# 1Password helpers (read-only) ----------------------------------------------
#
# Reads from the secrets-dev vault via the 1password skill's read_field.sh. The
# skill enforces --vault secrets-dev and the brettdavies naming/tagging
# conventions; calling `op read` directly would bypass those. Set
# OP_SERVICE_ACCOUNT_TOKEN in the environment (or use a service-account-bound
# shell) before invoking. Returns empty string if the skill isn't installed or
# the field doesn't exist; callers gate on `[[ -n "$value" ]]` and SKIP cleanly.
#
# Example: dev_bearer=$(read_1p "<APP-NAME>" credential)

readonly OP_SKILL="${OP_SKILL:-$HOME/.claude/skills/1password/scripts}"

read_1p() {
  [[ -x "$OP_SKILL/read_field.sh" ]] || return 1
  "$OP_SKILL/read_field.sh" "$1" "$2" 2>/dev/null
}

# Final summary --------------------------------------------------------------
#
# Callers that suppress (sub-scripts invoked with --result-file by the
# delegate_to_subscript helper) write counters to the result file instead and
# skip the colored summary line.

print_summary() {
  printf "\n%sSummary:%s  %s%d passed%s  %s%d failed%s  %s%d skipped%s\n" \
    "$C_BLD" "$C_RST" "$C_GRN" "$PASS_COUNT" "$C_RST" \
    "$C_RED" "$FAIL_COUNT" "$C_RST" "$C_YLW" "$SKIP_COUNT" "$C_RST"
}

# Sub-script delegation ------------------------------------------------------
#
# Runs a sub-script with --result-file pointing at a tmp file and aggregates
# its PASS/FAIL/SKIP counters into the parent's. The sub-script must accept
# --result-file PATH and write three space-separated integers to PATH at exit.
# A sub-script using this contract sources _lib.sh, runs its gates, and ends
# with:
#
#   if [[ -n "$RESULT_FILE" ]]; then
#       printf "%d %d %d\n" "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT" > "$RESULT_FILE"
#   else
#       print_summary
#   fi
#
# Exit codes from the sub-script are not propagated; the parent decides
# pass/fail based on its own aggregated counters after every gate runs.
#
# Usage:
#   delegate_to_subscript <script> <args...>

delegate_to_subscript() {
  local script="$1"
  shift
  local result_file
  result_file=$(mktemp)
  "$script" "$@" --result-file "$result_file" || true
  if [[ -s "$result_file" ]]; then
    local p f s
    read -r p f s <"$result_file"
    PASS_COUNT=$((PASS_COUNT + p))
    FAIL_COUNT=$((FAIL_COUNT + f))
    SKIP_COUNT=$((SKIP_COUNT + s))
  fi
  rm -f "$result_file"
}

# SMOKE_HOME seeding ---------------------------------------------------------
#
# Smoke gates that exercise the project's CLI against a live external service
# need an isolated $HOME so the dev machine's real config / token store is
# never touched. `shred -u` overwrites bytes before unlinking the tempdir on
# exit (closes the exfil window for cred recovery off the FS, backups, or the
# trash bin). Refuses to operate outside /tmp or $HOME as a path-typo guardrail
# (mirrors the 1Password skill's stage_secret.sh contract).
#
# Callers source _lib.sh, set SMOKE_HOME via mktemp inside their seed function,
# and the EXIT trap handles cleanup. Set NO_CLEANUP=1 to skip the shred (useful
# for debugging the seeded state after a failed gate).

SMOKE_HOME="${SMOKE_HOME:-}"
NO_CLEANUP="${NO_CLEANUP:-0}"

shred_tmpdir() {
  local dir="$1"
  [[ -n "$dir" && -d "$dir" ]] || return 0
  case "$dir" in
    /tmp/* | "$HOME"/*) ;;
    *)
      echo "refusing to shred outside /tmp or \$HOME: $dir" >&2
      return 1
      ;;
  esac
  if command -v shred >/dev/null 2>&1; then
    find "$dir" -type f -exec shred -u {} + 2>/dev/null || true
  else
    # `wc -c`, not `stat -c%s`: the stat flag is GNU-only and BSD stat rejects
    # it, which left `count=` empty, made dd a no-op under 2>/dev/null, and
    # silently downgraded the overwrite to a plain delete on every BSD host.
    find "$dir" -type f -exec sh -c '
      for f; do
        n=$(wc -c <"$f" | tr -d "[:space:]")
        dd if=/dev/urandom of="$f" bs=1 count="$n" conv=notrunc 2>/dev/null
        rm -f "$f"
      done' _ {} +
  fi
  find "$dir" -depth -type d -exec rmdir {} + 2>/dev/null || true
}

cleanup_smoke() {
  [[ "$NO_CLEANUP" -eq 0 && -n "$SMOKE_HOME" && -d "$SMOKE_HOME" ]] || return 0
  shred_tmpdir "$SMOKE_HOME"
}
