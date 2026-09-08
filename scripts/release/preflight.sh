#!/usr/bin/env bash
# Run release preflight gates against the current checkout.
#
# Usage:
#   scripts/release/preflight.sh <subcommand>
#
# Subcommands:
#   drift         Branch drift: what main carries that dev never received (delegated to drift.sh)
#   surface       Establish surface: commits + diff vs last tag, breaking markers
#   smoke         Real-world live API / external dependency smoke (project-authored)
#   mechanics     Release mechanics sanity (version, lockfile, advisories, toolchain age, leak check,
#                 unguarded docs added to main, diff-B vs origin/dev)
#   all           Run drift, surface, smoke, mechanics (and surface-smoke if present)
#
# Post-tag verification (release.yml + homebrew dispatch + finalize-release) lives in
# scripts/release/postflight.sh — that runs AFTER the tag push, not before.
#
# Flags:
#   --smoke-home PATH   Reuse an existing seeded $SMOKE_HOME instead of creating + seeding
#   --no-cleanup        Keep $SMOKE_HOME after exit (default: shred on exit)
#   --tag TAG           Override LAST_TAG resolution (default: git tag --sort=-version:refname | head -n 1)
#
# Exit codes:
#   0 = all gates passed (or skipped with reason)
#   1 = one or more gates failed
#   2 = setup error (missing dep, unreachable secrets store, etc.)
#
# Dependencies:
#   - the built release binary (project decides how; Rust: cargo build --release)
#   - `gh`, `git` on PATH; `jaq`, `yq` if the project's gates need them
#   - 1Password CLI service-account env, if smoke gates seed from `secrets-dev`
#   - ~/.claude/skills/1password/scripts/ for vault reads (via _lib.sh's read_1p)
#
# This script is a starter skeleton vendored from ~/.claude/skills/github-repo-setup/.
# The shared scaffolding (gate helpers, 1Password reads, shred cleanup, dispatch, surface,
# mechanics) is generic; the smoke gate body is project-specific — replace the placeholder
# implementation with the project's actual API / auth / output-format checks.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
readonly REPO_ROOT

# Shared output helpers, gate counters, dependency checks, 1Password helper,
# SMOKE_HOME cleanup. Same _lib.sh as postflight.sh and surface-smoke.sh.
. "$(dirname "$0")/_lib.sh"

# Project-specific: path to the built release binary. Adjust per project.
# Rust default: target/release/<binary>. Override BIN_PATH before invocation if needed.
BIN_PATH="${BIN_PATH:-$REPO_ROOT/target/release/bird}"

require_built_binary() {
  [[ -x "$BIN_PATH" ]] || {
    echo "build the release binary first ($BIN_PATH not found)" >&2
    exit 2
  }
}

# SMOKE_HOME EXIT trap (uses cleanup_smoke from _lib.sh) ---------------------

trap cleanup_smoke EXIT

seed_smoke_store() {
  # PROJECT-SPECIFIC. Replace the body with the project's seed recipe.
  # Pattern:
  #   1. SMOKE_HOME=$(mktemp -d -t <project>-preflight-XXXXXX)
  #   2. Read credentials from 1Password (read_1p "<APP-NAME>" <field>)
  #   3. Seed the project's local config (config files, token store, db, etc.)
  #
  # Example skeleton (uncomment + adapt):
  #
  # SMOKE_HOME="$(mktemp -d -t <project>-preflight-XXXXXX)"
  # local app_secret token
  # app_secret=$(read_1p "<APP-NAME>" credential)
  # token=$(read_1p "<APP-NAME>" access_token)
  # HOME="$SMOKE_HOME" "$BIN_PATH" auth login --token "$token" >/dev/null
  # unset app_secret token

  SMOKE_HOME="$(mktemp -d -t preflight-XXXXXX)"
}

ensure_smoke_home() {
  [[ -n "$SMOKE_HOME" && -d "$SMOKE_HOME" ]] && return 0
  echo "  seeding isolated SMOKE_HOME from 1Password..."
  seed_smoke_store
}

# Gate: surface --------------------------------------------------------------
#
# Generic: confirms what's actually changing since the last tag. Counts feed
# the human's gut-check on release scope and the breaking-marker tally drives
# the major-version decision.

gate_surface() {
  header "Establish surface"
  local last_tag commits files breaking
  last_tag="${LAST_TAG:-$(git tag --sort=-version:refname | head -n 1)}"
  [[ -n "$last_tag" ]] || {
    gate_skip "LAST_TAG" "no tags in repo yet (first release); surface is everything on the branch"
    return
  }
  commits=$(git log "$last_tag..HEAD" --oneline | wc -l)
  files=$(git diff "$last_tag..HEAD" --name-only | wc -l)
  # Scoped markers count too: `feat(api)!:` is breaking as much as `feat!:`.
  breaking=$(git log "$last_tag..HEAD" --grep '^[a-z]\+\(([^)]*)\)\?!:' --oneline | wc -l)
  gate_pass "LAST_TAG = $last_tag  ($commits commits, $files files, $breaking breaking)"
}

# Gate: smoke ----------------------------------------------------------------
#
# PROJECT-SPECIFIC. Replace with the project's live-API / external-dependency
# checks. Use ensure_smoke_home above to drive against an isolated $HOME so
# the dev machine's real config is never touched. Use read_1p for credentials.
#
# Example shape (each line is one logical gate):
#
#   local out
#   out=$(HOME="$SMOKE_HOME" "$BIN_PATH" whoami --output json 2>&1 | jaq -r '.data.username // ""')
#   [[ -n "$out" ]] && gate_pass "whoami → $out" || gate_fail "whoami" "no username"

gate_smoke() {
  header "Real-world smoke (live API / external dependency)"
  gate_skip "smoke gates" "project-authored; fill in scripts/release/preflight.sh § gate_smoke"
}

# Gate: drift (delegated to drift.sh) ----------------------------------------
#
# Security PRs, hotfixes, and config edits land on main first. The release
# branch is cut from main and then takes dev's changes, so anything main holds
# that dev never received is reverted by the release or collides with it.
# drift.sh lists that set (commits since the last release whose changes dev
# lacks, .github/ parity, and lockfile packages main resolves newer) and
# fails while any exist. Run it before cutting the release branch. Repos
# without a dev branch skip it.

gate_drift() {
  local drift_script
  drift_script="$(dirname "$0")/drift.sh"
  [[ -x "$drift_script" ]] || return 0
  header "Branch drift (delegated to drift.sh)"
  if ! git rev-parse --verify --quiet origin/dev >/dev/null 2>&1; then
    gate_skip "drift" "no origin/dev branch (single-branch repo)"
    return
  fi
  delegate_to_subscript "$drift_script"
}

# Gate: surface-smoke (optional delegation) ----------------------------------
#
# If the project ships an HTTP / MCP / gRPC surface that needs a callable
# smoke suite (transport + tools + auth), put it in scripts/release/surface-
# smoke.sh and the `all` runner picks it up automatically. The sub-script
# must accept --result-file PATH per the contract in _lib.sh's
# delegate_to_subscript helper. Pre-flight runs it against a local dev
# instance; post-flight runs the SAME script against the deployed env.

gate_surface_smoke() {
  local surface_script
  surface_script="$(dirname "$0")/surface-smoke.sh"
  [[ -x "$surface_script" ]] || return 0
  header "Surface smoke (delegated to surface-smoke.sh)"
  # Project picks the local URL the surface runs against; common defaults:
  #   bunx wrangler dev   → http://localhost:8787
  #   uvicorn / fastapi   → http://localhost:8000
  #   cargo run --release → http://localhost:3000
  local local_url="${LOCAL_URL:-http://localhost:8787}"
  if ! curl -fsS --max-time 2 "$local_url/" >/dev/null 2>&1; then
    gate_skip "surface-smoke" "local server not running at $local_url (start it and re-run)"
    return
  fi
  delegate_to_subscript "$surface_script" "$local_url"
}

# Gate: mechanics ------------------------------------------------------------
#
# Mostly generic; the Rust-specific lines (Cargo.toml, rust-toolchain.toml,
# cargo deny) are annotated. Adapt for non-Rust projects: VERSION file or
# package.json/pyproject.toml/go.mod for the version source; project's own
# dep-advisory scanner; project's own pinned toolchain marker.

gate_mechanics() {
  header "Release mechanics sanity"
  local project_version changelog_version last_tag

  # Rust: read version from Cargo.toml. Non-Rust: swap for the project's source of truth.
  if [[ -f Cargo.toml ]]; then
    project_version=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/^version = "(.*)"/\1/')
    gate_pass "Cargo.toml version = $project_version"
    [[ -f Cargo.lock ]] && gate_pass "Cargo.lock present" || gate_fail "Cargo.lock" "missing"
  elif [[ -f package.json ]]; then
    project_version=$(jaq -r .version package.json)
    gate_pass "package.json version = $project_version"
  elif [[ -f pyproject.toml ]]; then
    project_version=$(grep -m1 '^version = ' pyproject.toml | sed -E 's/^version = "(.*)"/\1/')
    gate_pass "pyproject.toml version = $project_version"
  elif [[ -f VERSION ]]; then
    project_version=$(<VERSION)
    gate_pass "VERSION = $project_version"
  else
    gate_skip "project version" "no Cargo.toml / package.json / pyproject.toml / VERSION found"
    project_version=""
  fi

  if [[ -x "$BIN_PATH" && -n "$project_version" ]]; then
    local bin_version
    bin_version=$("$BIN_PATH" --version 2>/dev/null | awk '{print $NF}')
    [[ "$bin_version" == "$project_version" ]] \
      && gate_pass "$BIN_PATH --version = $bin_version (matches project version)" \
      || gate_fail "$BIN_PATH --version mismatch" "binary=$bin_version project=$project_version"
  else
    gate_skip "binary --version" "build the release binary first ($BIN_PATH)"
  fi

  if [[ -f CHANGELOG.md ]]; then
    changelog_version=$(grep -m1 -oE '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md | tr -d '[]## ')
    if [[ -n "$project_version" ]]; then
      [[ "$changelog_version" == "$project_version" ]] \
        && gate_pass "CHANGELOG top section = [$changelog_version] (matches project version)" \
        || gate_fail "CHANGELOG mismatch" "changelog=$changelog_version project=$project_version"
    fi
    grep -q '\[Unreleased\]' CHANGELOG.md \
      && gate_fail "CHANGELOG" "has [Unreleased] placeholder" \
      || gate_pass "CHANGELOG has no [Unreleased] placeholder"
  fi

  # Rust: toolchain quarantine. Skip for non-Rust.
  if [[ -f rust-toolchain.toml ]]; then
    local toolchain_channel release_date_match
    toolchain_channel=$(grep -m1 'channel = ' rust-toolchain.toml | sed -E 's/.*"([^"]+)".*/\1/')
    release_date_match=$(grep -m1 'released' rust-toolchain.toml | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' || true)
    if [[ -n "$release_date_match" ]]; then
      local age_days
      age_days=$((($(date +%s) - $(date -d "$release_date_match" +%s)) / 86400))
      [[ $age_days -ge 7 ]] \
        && gate_pass "rust-toolchain channel=$toolchain_channel (released $release_date_match, $age_days days ago — ≥7 day quarantine satisfied)" \
        || gate_fail "rust-toolchain quarantine" "channel $toolchain_channel released $release_date_match ($age_days days ago) is inside 7-day window"
    else
      gate_skip "rust-toolchain quarantine" "no 'released YYYY-MM-DD' comment found in rust-toolchain.toml"
    fi
  fi

  # Rust: cargo deny check advisories. Swap for the project's scanner.
  if command -v cargo >/dev/null 2>&1 && [[ -f deny.toml ]]; then
    if cargo deny check advisories >/dev/null 2>&1; then
      gate_pass "cargo deny check advisories"
    else
      gate_fail "cargo deny check advisories" "see cargo deny check advisories"
    fi
  fi

  # Generic: the three screens below match against the guarded set the
  # workflow enforces, resolved by guarded-paths.sh, so this copy cannot drift
  # from what guard-main-docs rejects. A copy that omits a guarded path
  # reports a real leak as clean.
  local guarded ship_base
  if ! guarded=$("$(dirname "$0")/guarded-paths.sh" 2>/dev/null); then
    gate_fail "guarded-path list" "scripts/release/guarded-paths.sh resolved no pattern"
    return
  fi
  ship_base="${LAST_TAG:-origin/main}"
  git rev-parse --verify --quiet origin/main >/dev/null 2>&1 && ship_base=origin/main

  # Leak check: no guarded path in what the release adds to main.
  local leaked
  leaked=$(git diff "$ship_base..HEAD" --name-only 2>/dev/null | grep -E "$guarded" || true)
  if [[ -z "$leaked" ]]; then
    gate_pass "leak check (guarded paths): clean"
  else
    gate_fail "leak check" "guarded paths in diff vs $ship_base: $(echo "$leaked" | tr '\n' ' ')"
  fi

  # The leak check screens against the registered set, so it is blind to a
  # category nobody registered yet. Enumerate what the release adds to main
  # (anything under docs/, plus markdown anywhere, so a root-level glossary
  # shows up) and put every unguarded doc in front of a human.
  local added_docs
  added_docs=$(git diff "$ship_base..HEAD" --diff-filter=A --name-only 2>/dev/null | grep -E '(^docs/|\.md$)' | grep -Ev "$guarded" || true)
  if [[ -z "$added_docs" ]]; then
    gate_pass "no unguarded docs newly added to main"
  else
    gate_skip "unguarded docs added to main (confirm each is meant to ship)" "$(echo "$added_docs" | tr '\n' ' ')"
  fi

  # diff-B: files on dev that this branch lacks. Excluding all of docs/ would
  # hide a missed pick under a directory that ships to main, so exclude only
  # the guarded set. Version files and the regenerated changelog are
  # release-only by design.
  if git rev-parse --verify --quiet origin/dev >/dev/null 2>&1; then
    local missed
    missed=$(git diff HEAD..origin/dev --name-only 2>/dev/null | grep -Ev "$guarded" | grep -Ev '^(Cargo\.toml|Cargo\.lock|package\.json|package-lock\.json|pyproject\.toml|uv\.lock|VERSION|CHANGELOG\.md)$' || true)
    if [[ -z "$missed" ]]; then
      gate_pass "diff-B: no missed picks vs origin/dev"
    else
      gate_skip "diff-B: files on dev but not on this branch (review)" "$(echo "$missed" | head -5 | tr '\n' ' ')"
    fi
  else
    gate_skip "diff-B" "no origin/dev branch"
  fi
}

# Main dispatcher ------------------------------------------------------------

usage() {
  sed -n '2,32p' "$0" | sed 's/^# \?//'
  exit 2
}

LAST_TAG=""
SUBCMD=""

while [[ $# -gt 0 ]]; do
  # NO_CLEANUP is read by _lib.sh's EXIT-trap cleanup, not within this file.
  # shellcheck disable=SC2034
  case "$1" in
    --smoke-home)
      SMOKE_HOME="$2"
      shift 2
      ;;
    --no-cleanup)
      NO_CLEANUP=1
      shift
      ;;
    --tag)
      LAST_TAG="$2"
      shift 2
      ;;
    -h | --help) usage ;;
    drift | surface | smoke | mechanics | surface-smoke | all)
      SUBCMD="$1"
      shift
      ;;
    post-tag)
      echo "post-tag moved to scripts/release/postflight.sh — run that after the tag push" >&2
      exit 2
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage
      ;;
  esac
done

[[ -n "$SUBCMD" ]] || usage

case "$SUBCMD" in
  drift) gate_drift ;;
  surface) gate_surface ;;
  smoke) gate_smoke ;;
  mechanics) gate_mechanics ;;
  surface-smoke) gate_surface_smoke ;;
  all)
    gate_drift
    gate_surface
    gate_smoke
    gate_surface_smoke
    gate_mechanics
    ;;
esac

print_summary

[[ $FAIL_COUNT -eq 0 ]] || exit 1
