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
# staging deploy) ignore --env entirely; postflight runs identically.
# Multi-env site/service repos use --env staging after a dev push and
# --env prod after the release/* → main merge.
#
# The tag shape selects the release line. `vX.Y.Z` is the binary's, driving
# release.yml; `<crate>-vX.Y.Z` is a workspace library member's, driving
# release-lib.yml. A library release publishes one crate and stops, so the
# Homebrew, finalize, and make-latest gates adjust rather than fail.
#
# Subcommands:
#   release        release.yml (release-lib.yml on a library tag) on the tag push (conclusion=success)
#   tap            homebrew-tap update-formula + Publish bottles SUCCESS (SKIPs on a library tag)
#   finalize       finalize-release.yml callback ran (cross-repo dispatch loop closed; SKIPs on a library tag)
#   make-latest    GitHub Release is non-draft, non-prerelease, and releases/latest matches.
#                  On a library tag the check inverts: latest must NOT be it (make_latest: false)
#   crates         crates.io index shows <crate> v<X.Y.Z> published (Rust only; auto-skips otherwise)
#   tags           every workspace member the release moved is tagged <crate>-v<X.Y.Z> on the binary tag's commit
#   backport       dev has a merged PR carrying the released version in its title (prod only; SKIPs on staging)
#   surface-smoke  Delegates to scripts/release/surface-smoke.sh against the env's deployed URL (optional)
#   all            run every above sequentially
#
# Flags:
#   --env staging|prod      Target environment (default: prod). Single-env repos can ignore this.
#   --repo OWNER/REPO       Override the auto-detected nameWithOwner
#   --tap-repo OWNER/REPO   Override the homebrew-tap repo (default: brettdavies/homebrew-tap)
#   --tag vX.Y.Z            Override the tag (default: derived from Cargo.toml; falls back to latest git tag).
#                           Pass it explicitly for a library release: <crate>-vX.Y.Z
#   --crate NAME            Override the crate name for the `crates` gate. Default: the root Cargo.toml
#                           [package].name, or the sole workspace default member when the root is virtual
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
  print_usage_header
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
    release | tap | finalize | make-latest | crates | tags | backport | surface-smoke | all)
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
    # Through the shared reader, so a workspace whose root is a virtual manifest
    # reads the version from RELEASE_MANIFEST. Falling through to the newest tag
    # instead picks a library tag whenever the library's name sorts after `v`.
    version=$(cd "$REPO_ROOT" && project_version)
  elif [[ -f "$REPO_ROOT/package.json" ]] && have_bin jaq; then
    version=$(jaq -r '.version // empty' "$REPO_ROOT/package.json")
  elif [[ -f "$REPO_ROOT/pyproject.toml" ]]; then
    version=$(grep -m1 '^version = ' "$REPO_ROOT/pyproject.toml" | sed -E 's/^version = "(.*)"/\1/' || true)
  elif [[ -f "$REPO_ROOT/VERSION" ]]; then
    version=$(tr -d '[:space:]' <"$REPO_ROOT/VERSION")
  fi
  if [[ -n "$version" ]]; then
    echo "v${version#v}"
    return
  fi
  # Fallback: the newest tag on the binary's line.
  local git_tag
  git_tag=$(cd "$REPO_ROOT" && last_release_tag)
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
  # A library tag names its own crate, which settles the workspace case the
  # manifest cannot: a workspace holding a binary and a library has no single
  # "the crate", but a given tag always belongs to exactly one of them. The
  # crate is the member whose declared tag_prefix the tag carries, read the way
  # every release script reads it; the `<crate>-v` default is only a default,
  # so cutting the prefix off the tag would name a package that need not exist.
  local tag
  tag=$(resolve_tag)
  case "$tag" in
    v[0-9]*) ;;
    *-v[0-9]*)
      member_for_tag "$tag"
      return
      ;;
  esac
  # The release package: the root [package], or in a virtual workspace the
  # member release.env names as RELEASE_MANIFEST.
  local release_name
  release_name=$(cd "$REPO_ROOT" && project_crate 2>/dev/null || true)
  if [[ -n "$release_name" ]]; then
    echo "$release_name"
    return
  fi
  # A virtual root with no RELEASE_MANIFEST carries [workspace] and no
  # [package], so there is no release manifest to read and the repo is not
  # "non-Rust". With --no-deps, cargo lists exactly the workspace members; one
  # member is unambiguous, and more than one needs --crate because a binary tag
  # carries no package name to read.
  have_bin cargo || return 1
  have_bin jaq || return 1
  local members count
  members=$(cargo metadata --format-version 1 --no-deps --manifest-path "$REPO_ROOT/Cargo.toml" 2>/dev/null \
    | jaq -r '.packages[].name')
  count=$(printf '%s\n' "$members" | grep -c . || true)
  [[ "$count" -eq 1 ]] || return 1
  printf '%s\n' "$members"
}

# The version a tag carries, for either release line. The binary's tag is
# `v1.4.0`; a library member's is `<crate>-v0.2.0`, because rust-lib-release.yml
# defaults tag_prefix to `<crate>-v` so one tag push starts exactly one
# pipeline. Stripping a bare leading `v` from the library shape would leave the
# crate name glued to the version and every index comparison would miss.
tag_version() {
  local tag="$1"
  case "$tag" in
    v[0-9]*) echo "${tag#v}" ;;
    *-v[0-9]*) echo "${tag##*-v}" ;;
    *) echo "${tag#v}" ;;
  esac
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
# Which release line the tag belongs to. rust-lib-release.yml namespaces a
# library member's tag as `<crate>-v<version>` precisely so it can never match
# the binary caller's `v[0-9]+.[0-9]+.[0-9]+` filter, which makes the tag shape
# the authoritative signal here too. A library release publishes one crate and
# stops: no archives, no Homebrew dispatch, no finalize callback, and
# make_latest stays false on purpose so the binary keeps the repo's latest.
release_line() {
  case "$(resolve_tag)" in
    v[0-9]*) echo "binary" ;;
    *-v[0-9]*) echo "library" ;;
    *) echo "binary" ;;
  esac
}

release_workflow() {
  if [[ "$(release_line)" == "library" ]]; then
    echo "release-lib.yml"
  else
    echo "release.yml"
  fi
}

release_started_at() {
  local repo tag
  repo=$(resolve_repo)
  tag=$(resolve_tag)
  gh run list --repo "$repo" --workflow "$(release_workflow)" --branch "$tag" --limit 1 \
    --json createdAt --jq '.[0].createdAt // empty' 2>/dev/null || true
}

gate_release() {
  local wf
  wf=$(release_workflow)
  header "$wf on tag push"
  require_bin gh
  require_bin jaq
  local repo tag run
  repo=$(resolve_repo)
  tag=$(resolve_tag)

  run=$(gh run list --repo "$repo" --branch "$tag" --workflow "$wf" --limit 1 \
    --json databaseId,status,conclusion --jq '.[0]' 2>/dev/null || true)
  if [[ -z "$run" || "$run" == "null" ]]; then
    gate_skip "$wf run for $tag" "no run found on tag $tag yet (push the tag?)"
    return
  fi

  local status conclusion run_id
  status=$(printf '%s' "$run" | jaq -r .status)
  conclusion=$(printf '%s' "$run" | jaq -r .conclusion)
  run_id=$(printf '%s' "$run" | jaq -r .databaseId)

  if [[ "$status" != "completed" ]]; then
    gate_skip "$wf run $run_id" "status=$status (still running; re-run after watcher exits)"
    return
  fi
  if [[ "$conclusion" == "success" ]]; then
    gate_pass "$wf run $run_id conclusion=success"
  else
    gate_fail "$wf run $run_id" "conclusion=$conclusion (see gh run view $run_id --log-failed)"
  fi
}

# Gate: homebrew-tap ---------------------------------------------------------

gate_tap() {
  header "homebrew-tap dispatch + bottles publish"
  if [[ "$(release_line)" == "library" ]]; then
    gate_skip "tap chain" "library release ($(resolve_tag)): rust-lib-release.yml dispatches no Homebrew update"
    return
  fi
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
  if [[ "$(release_line)" == "library" ]]; then
    gate_skip "finalize callback" "library release ($(resolve_tag)): nothing attaches after the Release, so nothing calls back"
    return
  fi
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

  local latest
  latest=$(gh api "repos/$repo/releases/latest" --jq .tag_name 2>/dev/null || true)

  # A library release is the inverse check. rust-lib-release.yml sets
  # make_latest: false and nothing ever flips it, so the repo's latest release
  # must still be the binary's. Latest resolving to a library tag means the
  # flag was overridden somewhere and a visitor now lands on a release with no
  # archive to download.
  if [[ "$(release_line)" == "library" ]]; then
    if [[ "$latest" == "$tag" ]]; then
      gate_fail "releases/latest" "resolves to library tag $tag; rust-lib-release.yml sets make_latest: false so the binary keeps latest"
    elif [[ -n "$latest" ]]; then
      gate_pass "releases/latest = $latest, not the library tag $tag (make_latest: false held)"
    else
      gate_skip "releases/latest" "no latest release found"
    fi
    return
  fi

  # /releases/latest must resolve to this tag (set by finalize-release flipping make_latest)
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
  # meta-edits, etc.: anything the release-branch flow touched on main that
  # didn't round-trip to dev), so checking a single file's content can lie
  # both ways. The merged PR is the durable signal that the backport
  # operation ran, regardless of which files it included.
  # `gh pr list --search` is GitHub Search API syntax; "<text> in:title" silently
  # returns an empty result. Pass the tag for server-side filtering (the search
  # index tokenizes `v3.0.0` as one word, so a bare `3.0.0` misses it), then
  # jaq-filter the title for precision (`v?` accepts either spelling) and sort
  # by mergedAt descending so
  # the BACKPORT PR beats the FEATURE PR when both carry the version in their
  # titles (e.g., a "feat(api)!: vX.Y.Z ..." PR would otherwise be returned by
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
    return
  fi

  # No PR names this tag, which is not the same as no backport. A workspace
  # releases each member on its own tag line and syncs every member's
  # changelog back in one PR, titled for the binary's tag, so a member tag's
  # bookkeeping can arrive under another tag's name. The backport exists so dev
  # carries what the release wrote; for a member that declares a changelog,
  # ask that directly before reporting nothing happened.
  local crate changelog on_main on_dev
  crate=$(resolve_crate 2>/dev/null || true)
  changelog=""
  [[ -n "$crate" ]] && changelog=$(cd "$REPO_ROOT" && crate_changelog_path "$crate")
  if [[ -n "$changelog" ]]; then
    on_main=$(git -C "$REPO_ROOT" rev-parse --verify --quiet "origin/main:$changelog" || true)
    on_dev=$(git -C "$REPO_ROOT" rev-parse --verify --quiet "origin/dev:$changelog" || true)
    if [[ -n "$on_main" && "$on_main" == "$on_dev" ]]; then
      gate_pass "no PR names $tag, but dev carries main's $changelog (synced under another tag's backport)"
      return
    fi
    if [[ -n "$on_main" && -n "$on_dev" ]]; then
      gate_fail "main → dev backport" \
        "dev's $changelog differs from main's; run scripts/sync-dev-after-release.sh $tag (RELEASES-POSTFLIGHT.md § backport)"
      return
    fi
  fi

  gate_skip "main → dev backport" \
    "no PR carrying $tag merged to dev; run scripts/sync-dev-after-release.sh $tag (RELEASES-POSTFLIGHT.md § backport)"
}

# The workspace member that releases on TAG's line, matched on the tag_prefix
# its [package.metadata.changelog] table declares; nothing when no member
# declares the prefix or cargo cannot answer.
member_for_tag() {
  local tag="$1" prefix="" name member_prefix
  [[ "$tag" =~ ^(.*-v)[0-9] ]] && prefix="${BASH_REMATCH[1]}"
  [[ -n "$prefix" ]] || return 1
  while IFS=$'\t' read -r name _ member_prefix _ _; do
    if [[ "$member_prefix" == "$prefix" ]]; then
      echo "$name"
      return 0
    fi
  done < <(cd "$REPO_ROOT" && release_members)
  return 1
}

# Gate: crates.io ------------------------------------------------------------

gate_crates() {
  header "crates.io publish"
  local crate
  crate=$(resolve_crate || true)
  if [[ -z "$crate" ]]; then
    local unmatched
    unmatched=$(resolve_tag)
    # A library tag no member claims is a mistake in the tag or the manifest,
    # not a publish still in flight: report it rather than skip past it.
    if [[ "$unmatched" == *-v[0-9]* && "$unmatched" != v[0-9]* ]] \
      && [[ -n "$(cd "$REPO_ROOT" && release_members)" ]]; then
      gate_fail "crates.io publish" \
        "$unmatched names no workspace member: no [package.metadata.changelog] tag_prefix matches it; fix the tag or pass --crate NAME"
      return
    fi
    if [[ -f "$REPO_ROOT/Cargo.toml" ]]; then
      gate_skip "crates.io publish" "workspace with more than one member and a tag that names none of them; pass --crate NAME"
    else
      gate_skip "crates.io publish" "no Cargo.toml [package].name; non-Rust repo (pass --crate NAME to force)"
    fi
    return
  fi
  require_bin cargo
  local tag version
  tag=$(resolve_tag)
  version=$(tag_version "$tag")

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

# Gate: tags -----------------------------------------------------------------
#
# A workspace releases each publishable library member on its own
# `<crate>-vX.Y.Z` line. When a release moves a member's version, the member's
# tag belongs on the binary tag's commit: release-lib publishes whatever its
# tag points at, a moved member with no tag never publishes, and tooling keyed
# to the binary tag, such as the dev backport, reaches the member's release
# only through that shared commit. Checked from the binary tag, which the
# runbook pushes last.

# The [package] version a manifest declares at a commit, following
# `version.workspace = true` to the root's [workspace.package].
manifest_version_at() {
  local commit="$1" path="$2" text
  text=$(git -C "$REPO_ROOT" show "$commit:$path" 2>/dev/null) || return 0
  if printf '%s\n' "$text" | grep -Eq '^version(\.workspace *= *true| *= *\{ *workspace *= *true)'; then
    text=$(git -C "$REPO_ROOT" show "$commit:Cargo.toml" 2>/dev/null) || return 0
    printf '%s\n' "$text" | awk '
      /^\[workspace\.package\]/ { in_ws = 1; next }
      /^\[/                     { in_ws = 0 }
      in_ws && /^version *= *"/ { sub(/^version *= *"/, ""); sub(/".*/, ""); print; exit }
    '
    return 0
  fi
  printf '%s\n' "$text" | awk '
    /^\[package\]/            { in_pkg = 1; next }
    /^\[/                     { in_pkg = 0 }
    in_pkg && /^version *= *"/ { sub(/^version *= *"/, ""); sub(/".*/, ""); print; exit }
  '
}

gate_tags() {
  header "Release tags"
  local tag
  tag=$(resolve_tag)
  case "$tag" in
    v[0-9]*) ;;
    *)
      gate_skip "release tags" "$tag is a library tag; run this gate on the binary tag, which the runbook pushes last"
      return
      ;;
  esac
  if [[ ! -f "$REPO_ROOT/Cargo.toml" ]]; then
    gate_skip "release tags" "no Cargo.toml; non-Rust repo"
    return
  fi
  if ! have_bin cargo || ! have_bin jaq; then
    gate_skip "release tags" "needs cargo and jaq to list the workspace members"
    return
  fi
  local commit
  if ! commit=$(git -C "$REPO_ROOT" rev-parse --verify --quiet "$tag^{commit}"); then
    gate_fail "release tags" "$tag is not a local tag; run git fetch origin --tags"
    return
  fi

  # Publishable members other than the binary's own package, each with the
  # tag line its [package.metadata.changelog] table names.
  local members
  members=$(cd "$REPO_ROOT" && release_members)
  if [[ -z "$members" ]]; then
    local binary
    binary=$(cd "$REPO_ROOT" && project_crate)
    gate_skip "release tags" "no publishable member besides ${binary:-the binary} releases on its own tag line"
    return
  fi

  # The binary tag before this one, whose commit says what this release moved.
  local prev prev_commit=""
  prev=$(git -C "$REPO_ROOT" tag --list 'v[0-9]*' --sort=-version:refname \
    | awk -v cur="$tag" 'seen { print; exit } $0 == cur { seen = 1 }')
  [[ -n "$prev" ]] && prev_commit=$(git -C "$REPO_ROOT" rev-parse --verify --quiet "$prev^{commit}" || true)

  local name path prefix now before member_tag tagged
  while IFS=$'\t' read -r name path prefix _ _; do
    now=$(manifest_version_at "$commit" "$path")
    if [[ -z "$now" ]]; then
      gate_skip "$name tag" "no version in $path at $tag"
      continue
    fi
    before=""
    [[ -n "$prev_commit" ]] && before=$(manifest_version_at "$prev_commit" "$path")
    member_tag="$prefix$now"
    tagged=$(git -C "$REPO_ROOT" rev-parse --verify --quiet "$member_tag^{commit}" || true)
    if [[ "$tagged" == "$commit" ]]; then
      gate_pass "$member_tag is on $tag's commit"
    elif [[ "$now" == "$before" ]]; then
      gate_pass "$name $now is unchanged since $prev, so $tag carries no $name release"
    elif [[ -z "$tagged" ]]; then
      gate_fail "$name tag" \
        "$tag moves $name to $now but $member_tag does not exist; tag the release commit: git tag -a -m \"Release $member_tag\" $member_tag ${commit:0:12} && git push origin $member_tag"
    else
      gate_fail "$name tag" \
        "$tag moves $name to $now but $member_tag points at ${tagged:0:12}, not $tag's ${commit:0:12}; the library published from another commit"
    fi
  done <<<"$members"
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
  tags) gate_tags ;;
  backport) gate_backport ;;
  surface-smoke) gate_surface_smoke ;;
  all)
    gate_release
    gate_tap
    gate_finalize
    gate_make_latest
    gate_crates
    gate_tags
    gate_backport
    gate_surface_smoke
    ;;
esac

print_summary

[[ $FAIL_COUNT -eq 0 ]] || exit 1
