#!/usr/bin/env bash
# Verify the vX.Y.Z tag's downstream pipeline landed cleanly. Also reusable
# for verifying a deployed env (staging or prod) when the project ships
# multi-env deploys; --env staging|prod selects the target.
#
# Usage:
#   scripts/release/postflight.sh [--env staging|prod] <subcommand>
#
# Runs AFTER the release/v<X.Y.Z> -> main PR merges and the tag is pushed,
# triggering release.yml. Companion to scripts/release/preflight.sh which
# runs BEFORE the release branch cut.
#
# Single-env repos (Rust CLIs releasing to crates.io + homebrew with no
# staging deploy) ignore --env entirely — postflight runs identically.
# Multi-env site/service repos use --env staging after a dev push and
# --env prod after the release/* → main merge.
#
# Subcommands:
#   release        release.yml on the tag push (conclusion=success)
#   tap            homebrew-tap update-formula + Publish bottles SUCCESS
#   finalize       finalize-release.yml callback ran (cross-repo dispatch loop closed)
#   make-latest    GitHub Release v<X.Y.Z> is non-draft, non-prerelease, releases/latest matches
#   crates         crates.io index shows <crate> v<X.Y.Z> published (Rust only; auto-skips otherwise)
#   backport       dev has a merged PR carrying the released version in its title (prod only; SKIPs on staging)
#   surface-smoke  Delegates to scripts/release/surface-smoke.sh against the env's deployed URL (optional)
#   all            run every above sequentially
#
# Flags:
#   --env staging|prod      Target environment (default: prod). Single-env repos can ignore this.
#   --repo OWNER/REPO       Override the auto-detected nameWithOwner
#   --tap-repo OWNER/REPO   Override the homebrew-tap repo (default: brettdavies/homebrew-tap)
#   --tag vX.Y.Z            Override the tag (default: derived from Cargo.toml; falls back to latest git tag)
#   --crate NAME            Override the crate name for the `crates` gate (default: Cargo.toml [package].name)
#   --staging-url URL       Override the staging URL for surface-smoke
#   --prod-url URL          Override the prod URL for surface-smoke
#
# Exit codes:
#   0 = all gates passed (or skipped with reason)
#   1 = one or more gates failed
#   2 = setup error (missing dep, unauthenticated gh, etc.)
#
# Install-on-fresh-machine smokes (cargo install, brew install, cargo binstall) are NOT
# driven from here. Running them on the local dev machine pollutes the toolchain and
# doesn't actually exercise "fresh machine" semantics. See RELEASES-POSTFLIGHT.md
# § Checklist for the recipes; drive on a throwaway container or sibling machine.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
readonly REPO_ROOT
readonly DEFAULT_TAP_REPO="brettdavies/homebrew-tap"

# Shared output helpers, gate counters, dependency checks, 1Password helper.
# Same _lib.sh as preflight.sh and surface-smoke.sh.
# shellcheck disable=SC1091  # sibling _lib.sh, always vendored alongside
. "$(dirname "$0")/_lib.sh"

# Argument parsing -----------------------------------------------------------

ENV=""
REPO=""
TAP_REPO="$DEFAULT_TAP_REPO"
TAG=""
CRATE=""
STAGING_URL=""
PROD_URL=""
SUBCMD=""

usage() {
  sed -n '2,45p' "$0" | sed 's/^# \?//'
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --env)
      ENV="$2"
      shift 2
      ;;
    --repo)
      REPO="$2"
      shift 2
      ;;
    --tap-repo)
      TAP_REPO="$2"
      shift 2
      ;;
    --tag)
      TAG="$2"
      shift 2
      ;;
    --crate)
      CRATE="$2"
      shift 2
      ;;
    --staging-url)
      STAGING_URL="$2"
      shift 2
      ;;
    --prod-url)
      PROD_URL="$2"
      shift 2
      ;;
    -h | --help) usage ;;
    release | tap | finalize | make-latest | crates | backport | surface-smoke | all)
      SUBCMD="$1"
      shift
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage
      ;;
  esac
done

[[ -n "$SUBCMD" ]] || usage

# Default --env to prod when omitted. Multi-env repos override per invocation;
# single-env repos never see the difference because the env-specific gates
# (surface-smoke, backport-SKIP-on-staging) only fire when explicitly opted in.
ENV="${ENV:-prod}"
case "$ENV" in
  staging | prod) ;;
  *)
    echo "--env must be 'staging' or 'prod', got: $ENV" >&2
    exit 2
    ;;
esac

resolve_repo() {
  [[ -n "$REPO" ]] && {
    echo "$REPO"
    return
  }
  gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null \
    || {
      echo "could not resolve repo (pass --repo OWNER/REPO)" >&2
      exit 2
    }
}

resolve_tag() {
  if [[ -n "$TAG" ]]; then
    echo "$TAG"
    return
  fi
  # The project version, in the same order the preflight mechanics gate
  # detects it: Cargo.toml, package.json, pyproject.toml, VERSION.
  local version=""
  if [[ -f "$REPO_ROOT/Cargo.toml" ]]; then
    version=$(grep -m1 '^version = ' "$REPO_ROOT/Cargo.toml" | sed -E 's/^version = "(.*)"/\1/')
  elif [[ -f "$REPO_ROOT/package.json" ]] && have_bin jaq; then
    version=$(jaq -r '.version // empty' "$REPO_ROOT/package.json")
  elif [[ -f "$REPO_ROOT/pyproject.toml" ]]; then
    version=$(grep -m1 '^version = ' "$REPO_ROOT/pyproject.toml" | sed -E 's/^version = "(.*)"/\1/')
  elif [[ -f "$REPO_ROOT/VERSION" ]]; then
    version=$(tr -d '[:space:]' <"$REPO_ROOT/VERSION")
  fi
  if [[ -n "$version" ]]; then
    echo "v${version#v}"
    return
  fi
  # Fallback: latest git tag.
  local git_tag
  git_tag=$(git -C "$REPO_ROOT" tag --sort=-version:refname | head -n 1)
  if [[ -n "$git_tag" ]]; then
    echo "$git_tag"
    return
  fi
  echo "could not resolve tag (pass --tag vX.Y.Z)" >&2
  exit 2
}

resolve_crate() {
  [[ -n "$CRATE" ]] && {
    echo "$CRATE"
    return
  }
  [[ -f "$REPO_ROOT/Cargo.toml" ]] || return 1
  # [package].name = "..."
  awk '
        /^\[package\]/ { in_pkg = 1; next }
        /^\[/          { in_pkg = 0 }
        in_pkg && /^name = / {
            sub(/^name = "/, ""); sub(/".*/, ""); print; exit
        }
    ' "$REPO_ROOT/Cargo.toml"
}

resolve_env_url() {
  if [[ "$ENV" == "staging" ]]; then
    echo "${STAGING_URL:-${STAGING_URL_DEFAULT:-}}"
  else
    echo "${PROD_URL:-${PROD_URL_DEFAULT:-}}"
  fi
}

# Gate: release.yml ----------------------------------------------------------

# Every downstream run (tap update-formula, Publish bottles, finalize-release)
# is matched by name and start time against the release.yml run for this tag.
# Matching by name alone returns the previous release's runs when this
# release's are still queued, which reads as a false pass; the tap repo is
# shared across every CLI, so another repo's release can satisfy a name-only
# match too. Prints nothing when release.yml has not started for the tag.
release_started_at() {
  local repo tag
  repo=$(resolve_repo)
  tag=$(resolve_tag)
  gh run list --repo "$repo" --workflow release.yml --branch "$tag" --limit 1 \
    --json createdAt --jq '.[0].createdAt // empty' 2>/dev/null || true
}

gate_release() {
  header "release.yml on tag push"
  require_bin gh
  require_bin jaq
  local repo tag run
  repo=$(resolve_repo)
  tag=$(resolve_tag)

  run=$(gh run list --repo "$repo" --branch "$tag" --workflow release.yml --limit 1 \
    --json databaseId,status,conclusion --jq '.[0]' 2>/dev/null || true)
  if [[ -z "$run" || "$run" == "null" ]]; then
    gate_skip "release.yml run for $tag" "no run found on tag $tag yet (push the tag?)"
    return
  fi

  local status conclusion run_id
  status=$(printf '%s' "$run" | jaq -r .status)
  conclusion=$(printf '%s' "$run" | jaq -r .conclusion)
  run_id=$(printf '%s' "$run" | jaq -r .databaseId)

  if [[ "$status" != "completed" ]]; then
    gate_skip "release.yml run $run_id" "status=$status (still running; re-run after watcher exits)"
    return
  fi
  if [[ "$conclusion" == "success" ]]; then
    gate_pass "release.yml run $run_id conclusion=success"
  else
    gate_fail "release.yml run $run_id" "conclusion=$conclusion (see gh run view $run_id --log-failed)"
  fi
}

# Gate: homebrew-tap ---------------------------------------------------------

gate_tap() {
  header "homebrew-tap dispatch + bottles publish"
  require_bin gh
  require_bin jaq
  local tap=$TAP_REPO tag since
  tag=$(resolve_tag)
  since=$(release_started_at)
  if [[ -z "$since" ]]; then
    gate_skip "tap chain" "no release.yml run for $tag yet"
    return
  fi

  # update-formula = repository_dispatch from release.yml
  local uf
  uf=$(gh run list --repo "$tap" --event repository_dispatch --limit 10 \
    --json databaseId,status,conclusion,displayTitle,createdAt \
    --jq "[.[] | select(.displayTitle == \"update-formula\" and .createdAt >= \"$since\")] | .[0]" 2>/dev/null || true)
  if [[ -z "$uf" || "$uf" == "null" ]]; then
    gate_skip "tap update-formula dispatch" "no run on $tap since release.yml started ($since)"
  else
    local uf_status uf_conclusion uf_id
    uf_status=$(printf '%s' "$uf" | jaq -r .status)
    uf_conclusion=$(printf '%s' "$uf" | jaq -r .conclusion)
    uf_id=$(printf '%s' "$uf" | jaq -r .databaseId)
    if [[ "$uf_status" == "completed" && "$uf_conclusion" == "success" ]]; then
      gate_pass "tap update-formula run $uf_id (dispatch from release.yml) success"
    elif [[ "$uf_status" == "completed" ]]; then
      gate_fail "tap update-formula run $uf_id" "conclusion=$uf_conclusion (see gh run view $uf_id -R $tap --log-failed)"
    else
      gate_skip "tap update-formula run $uf_id" "status=$uf_status"
    fi
  fi

  # Publish bottles = workflow_run triggered by the CI completion on the formula-bump PR
  local pb
  pb=$(gh run list --repo "$tap" --event workflow_run --limit 10 \
    --json databaseId,status,conclusion,displayTitle,createdAt \
    --jq "[.[] | select(.displayTitle == \"Publish bottles\" and .createdAt >= \"$since\")] | .[0]" 2>/dev/null || true)
  if [[ -z "$pb" || "$pb" == "null" ]]; then
    gate_skip "tap Publish bottles" "no run since release.yml started ($since); CI on the formula PR may still be running"
    return
  fi
  local pb_status pb_conclusion pb_id
  pb_status=$(printf '%s' "$pb" | jaq -r .status)
  pb_conclusion=$(printf '%s' "$pb" | jaq -r .conclusion)
  pb_id=$(printf '%s' "$pb" | jaq -r .databaseId)
  if [[ "$pb_status" == "completed" && "$pb_conclusion" == "success" ]]; then
    gate_pass "tap Publish bottles run $pb_id success (bottle commit pushed to $tap main)"
  elif [[ "$pb_status" == "completed" ]]; then
    gate_fail "tap Publish bottles run $pb_id" "conclusion=$pb_conclusion"
  else
    gate_skip "tap Publish bottles run $pb_id" "status=$pb_status"
  fi
}

# Gate: finalize-release.yml -------------------------------------------------

gate_finalize() {
  header "finalize-release.yml callback"
  require_bin gh
  require_bin jaq
  local repo since
  repo=$(resolve_repo)
  since=$(release_started_at)
  if [[ -z "$since" ]]; then
    gate_skip "finalize-release.yml run" "no release.yml run for $(resolve_tag) yet"
    return
  fi

  local fr
  fr=$(gh run list --repo "$repo" --event repository_dispatch --workflow finalize-release.yml --limit 10 \
    --json databaseId,status,conclusion,createdAt \
    --jq "[.[] | select(.createdAt >= \"$since\")] | .[0]" 2>/dev/null || true)
  if [[ -z "$fr" || "$fr" == "null" ]]; then
    gate_skip "finalize-release.yml run" "no callback since release.yml started ($since); Publish bottles may still be running on $TAP_REPO"
    return
  fi
  local fr_status fr_conclusion fr_id
  fr_status=$(printf '%s' "$fr" | jaq -r .status)
  fr_conclusion=$(printf '%s' "$fr" | jaq -r .conclusion)
  fr_id=$(printf '%s' "$fr" | jaq -r .databaseId)
  if [[ "$fr_status" == "completed" && "$fr_conclusion" == "success" ]]; then
    gate_pass "finalize-release.yml run $fr_id success (cross-repo dispatch loop closed)"
  elif [[ "$fr_status" == "completed" ]]; then
    gate_fail "finalize-release.yml run $fr_id" "conclusion=$fr_conclusion"
  else
    gate_skip "finalize-release.yml run $fr_id" "status=$fr_status"
  fi
}

# Gate: make_latest flip -----------------------------------------------------

gate_make_latest() {
  header "GitHub Release marked latest"
  require_bin gh
  require_bin jaq
  local repo tag
  repo=$(resolve_repo)
  tag=$(resolve_tag)

  # Release exists + non-draft + non-prerelease + correct asset count
  local rel
  rel=$(gh release view "$tag" --repo "$repo" --json isDraft,isPrerelease,assets 2>/dev/null || true)
  if [[ -z "$rel" ]]; then
    gate_skip "Release $tag" "release.yml hasn't created it yet"
    return
  fi
  local is_draft is_prerelease asset_count
  is_draft=$(printf '%s' "$rel" | jaq -r .isDraft)
  is_prerelease=$(printf '%s' "$rel" | jaq -r .isPrerelease)
  asset_count=$(printf '%s' "$rel" | jaq -r '.assets | length')

  if [[ "$is_draft" == "true" ]]; then
    gate_fail "Release $tag draft" "isDraft=true (release.yml should publish non-draft)"
  elif [[ "$is_prerelease" == "true" ]]; then
    gate_fail "Release $tag prerelease" "isPrerelease=true (release.yml should publish stable)"
  else
    gate_pass "Release $tag published non-draft, non-prerelease, $asset_count assets"
  fi

  # /releases/latest must resolve to this tag (set by finalize-release flipping make_latest)
  local latest
  latest=$(gh api "repos/$repo/releases/latest" --jq .tag_name 2>/dev/null || true)
  if [[ "$latest" == "$tag" ]]; then
    gate_pass "releases/latest = $tag (finalize-release flipped make_latest=true)"
  elif [[ -n "$latest" ]]; then
    gate_skip "releases/latest" "currently $latest, expected $tag (homebrew dispatch chain may still be running)"
  else
    gate_skip "releases/latest" "no latest release found"
  fi
}

# Gate: main → dev backport --------------------------------------------------
#
# Prod-only. The release-branch concept doesn't apply on staging (deploys
# happen directly from dev), so the gate SKIPs cleanly when --env staging.

gate_backport() {
  header "main → dev backport"
  if [[ "$ENV" == "staging" ]]; then
    gate_skip "main → dev backport" "prod-only gate (staging deploys from dev directly)"
    return
  fi
  require_bin gh
  require_bin jaq
  local repo tag version
  repo=$(resolve_repo)
  tag=$(resolve_tag)
  version="${tag#v}"

  # Look for a merged PR to dev with the version in the title. The backport
  # carries more than CHANGELOG.md (cliff.toml, README polish, RELEASES.md
  # meta-edits, etc. — anything the release-branch flow touched on main that
  # didn't round-trip to dev), so checking a single file's content can lie
  # both ways. The merged PR is the durable signal that the backport
  # operation ran, regardless of which files it included.
  # `gh pr list --search` is GitHub Search API syntax; "<text> in:title" silently
  # returns an empty result. Pass the tag for server-side filtering (the search
  # index tokenizes `v3.0.0` as one word, so a bare `3.0.0` misses it), then
  # jaq-filter the title for precision (`v?` accepts either spelling) and sort
  # by mergedAt descending so
  # the BACKPORT PR beats the FEATURE PR when both carry the version in their
  # titles (e.g., a "feat(api)!: vX.Y.Z — …" PR would otherwise be returned by
  # `--jq '.[0]'` without sort and falsely pass the gate).
  local pr=""
  pr=$(gh pr list --repo "$repo" --base dev --state merged --limit 20 \
    --search "$tag" \
    --json number,title,mergedAt,headRefName \
    --jq "[.[] | select(.title | test(\"v?$version\"))] | sort_by(.mergedAt) | reverse | .[0]" \
    2>/dev/null || true)
  [[ "$pr" == "null" ]] && pr=""

  if [[ -n "$pr" ]]; then
    local pr_num pr_title pr_head
    pr_num=$(printf '%s' "$pr" | jaq -r .number)
    pr_title=$(printf '%s' "$pr" | jaq -r .title)
    pr_head=$(printf '%s' "$pr" | jaq -r .headRefName)
    gate_pass "backport PR #$pr_num merged to dev from $pr_head: $pr_title"
  else
    gate_skip "main → dev backport" \
      "no PR carrying $tag merged to dev; run scripts/sync-dev-after-release.sh $tag (RELEASES-POSTFLIGHT.md § backport)"
  fi
}

# Gate: crates.io ------------------------------------------------------------

gate_crates() {
  header "crates.io publish"
  local crate
  crate=$(resolve_crate || true)
  if [[ -z "$crate" ]]; then
    gate_skip "crates.io publish" "no Cargo.toml [package].name — non-Rust repo (pass --crate NAME to force)"
    return
  fi
  require_bin cargo
  local tag version
  tag=$(resolve_tag)
  version="${tag#v}"

  local found
  found=$(cargo search "$crate" --limit 1 2>/dev/null | grep -E "^${crate} = " | head -1 || true)
  if [[ -z "$found" ]]; then
    gate_skip "crates.io $crate" "no matching crate in index (publish may still be in flight)"
    return
  fi

  # cargo search prints `name = "version" # description`
  local published
  published=$(printf '%s' "$found" | sed -E 's/.*"([^"]+)".*/\1/')
  if [[ "$published" == "$version" ]]; then
    gate_pass "crates.io shows $crate $published (matches $tag)"
  else
    gate_skip "crates.io $crate" "index shows $published, expected $version (publish may still be replicating)"
  fi
}

# Gate: surface-smoke (optional delegation) ----------------------------------
#
# Multi-env site/service repos with a deployed HTTP / MCP / API surface put
# the suite in scripts/release/surface-smoke.sh and the `all` runner picks it
# up automatically. The sub-script must accept --result-file PATH per the
# contract in _lib.sh's delegate_to_subscript helper. Same suite as preflight
# uses against the local dev server; only the URL differs.

gate_surface_smoke() {
  local surface_script
  surface_script="$(dirname "$0")/surface-smoke.sh"
  [[ -x "$surface_script" ]] || return 0
  header "Surface smoke (delegated to surface-smoke.sh against $ENV)"
  local url
  url=$(resolve_env_url)
  if [[ -z "$url" ]]; then
    gate_skip "surface-smoke" "no URL configured for env=$ENV (pass --staging-url / --prod-url)"
    return
  fi
  if ! curl -fsS --max-time 5 "$url/" >/dev/null 2>&1; then
    gate_skip "surface-smoke" "URL $url not reachable (deploy still in flight?)"
    return
  fi
  delegate_to_subscript "$surface_script" "$url"
}

# Main dispatcher ------------------------------------------------------------

case "$SUBCMD" in
  release) gate_release ;;
  tap) gate_tap ;;
  finalize) gate_finalize ;;
  make-latest) gate_make_latest ;;
  crates) gate_crates ;;
  backport) gate_backport ;;
  surface-smoke) gate_surface_smoke ;;
  all)
    gate_release
    gate_tap
    gate_finalize
    gate_make_latest
    gate_crates
    gate_backport
    gate_surface_smoke
    ;;
esac

print_summary

[[ $FAIL_COUNT -eq 0 ]] || exit 1
