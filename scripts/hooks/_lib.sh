#!/usr/bin/env bash
# Shared helpers for the git hook pair. Sourced by scripts/hooks/pre-commit and
# scripts/hooks/pre-push, never executed directly.
#
# Seeded to: scripts/hooks/_lib.sh
# Activate the pair per clone: git config core.hooksPath scripts/hooks
#
# Contract every check_* helper honours, so the two hooks stay interchangeable:
#   - takes the file list as positional arguments (the *_all variants take none)
#   - returns 0 on a pass, $HOOK_SKIPPED when it did not run, and the tool's own
#     exit status on a failure. Three outcomes, not two: a skip that returned 0
#     would print a pass line, which is the lie the skip notice exists to
#     prevent.
#   - announces a skip caused by a missing tool on stderr. A silent skip is
#     indistinguishable from a passing check, which is how an under-provisioned
#     machine comes to look green. An empty file list is not announced: nothing
#     staged of a given type is not a gap in the gate.
#   - is driven by `step`, which turns those three outcomes into output and a
#     0/1 status every caller aggregates the same way: `step ... || fail=1`.
#
# Callers cd to the repo root before sourcing, so relative paths resolve.
#
# CUSTOMIZE: this library ships the checks the two templates use. Drop the
# helpers a project has no use for rather than carrying dead code, and add a
# project's own checks here, not inline in one hook, whenever both hooks need
# them.

# Require bash >= 4.4: mapfile, and safe empty-array expansion under `set -u`.
# Sourced, so return rather than exit, which would kill an interactive shell;
# the sourcing hook aborts on the non-zero return. The hooks re-exec under a
# newer bash before they source this, so this fires only for another caller.
if ((BASH_VERSINFO[0] < 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] < 4))); then
  printf 'error: bash >= 4.4 required, but this is bash %s.\n' "${BASH_VERSION:-unknown}" >&2
  printf 'Install a newer bash: brew install bash\n' >&2
  # shellcheck disable=SC2317  # `exit` runs only when executed rather than sourced
  return 1 2>/dev/null || exit 1
fi

# ---------------------------------------------------------------------------
# Output and step dispatch
# ---------------------------------------------------------------------------

# Colour only when stdout is a terminal: a hook run through a pager, a CI log,
# or an agent harness should not carry escape codes.
if [ -t 1 ]; then
  _HOOK_RED=$'\033[0;31m'
  _HOOK_GREEN=$'\033[0;32m'
  _HOOK_BOLD=$'\033[1m'
  _HOOK_RESET=$'\033[0m'
else
  _HOOK_RED=''
  _HOOK_GREEN=''
  _HOOK_BOLD=''
  _HOOK_RESET=''
fi

banner() { printf '%s%s%s\n' "$_HOOK_BOLD" "$1" "$_HOOK_RESET"; }
pass() { printf '  %s✓%s %s\n' "$_HOOK_GREEN" "$_HOOK_RESET" "$1"; }
step_failed() { printf '  %s✗%s %s\n' "$_HOOK_RED" "$_HOOK_RESET" "$1" >&2; }

# The status meaning "this check did not run": distinct from both 0 and a tool
# failure, so `step` can tell a skip from a pass.
HOOK_SKIPPED=99

# Every announced skip goes through here, so no missing tool skips silently.
# Returns HOOK_SKIPPED, so a check guards with `have foo || skip ... || return`.
skip() {
  printf '  - %s (skipped: %s)\n' "$1" "$2" >&2
  return "$HOOK_SKIPPED"
}

have() { command -v "$1" >/dev/null 2>&1; }

# A step left out on purpose, such as the Rust battery for a push that reaches
# no cargo input. Not a skip: no tool is missing and nothing goes unchecked.
note() { printf '  - %s\n' "$1"; }

# Run one check and report it. `label` names the pass line, `hint` names the
# command to rerun after a failure. Returns 0 on a pass or a skip, 1 on a
# failure.
step() {
  local label="$1" hint="$2" rc
  shift 2
  "$@"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    pass "$label"
  elif [ "$rc" -ne "$HOOK_SKIPPED" ]; then
    step_failed "$hint"
    return 1
  fi
}

# Checks that share no resource run together, so the slowest one sets the
# wall-clock instead of the sum. Each buffers its own output and status;
# step_wait replays them in the order they were queued, so the report reads the
# same as a serial run whatever order they finished in.
#
# Never queue two checks that contend for the same lock. Every cargo step takes
# the build lock, so queueing them wins nothing and only hides the contention.
# Anything asserting on wall-clock (a startup-latency budget, a timing test)
# must run alone, before the queue, for the same reason.
_HOOK_BG_DIR=''
_HOOK_BG_LABEL=()
_HOOK_BG_HINT=()

_hook_bg_cleanup() {
  [ -n "$_HOOK_BG_DIR" ] && rm -rf "$_HOOK_BG_DIR"
  _HOOK_BG_DIR=''
  _HOOK_BG_LABEL=()
  _HOOK_BG_HINT=()
  return 0
}

# step_bg <label> <hint> <command>...: queue a check. Same contract as `step`.
step_bg() {
  local label="$1" hint="$2" slot
  shift 2
  if [ -z "$_HOOK_BG_DIR" ]; then
    _HOOK_BG_DIR="$(mktemp -d)"
    # step_wait cleans up on the happy path; the trap covers an early exit.
    trap '_hook_bg_cleanup' EXIT
  fi
  slot="${#_HOOK_BG_LABEL[@]}"
  _HOOK_BG_LABEL+=("$label")
  _HOOK_BG_HINT+=("$hint")
  {
    "$@" >"$_HOOK_BG_DIR/$slot.out" 2>&1
    printf '%s' "$?" >"$_HOOK_BG_DIR/$slot.rc"
  } &
}

# Wait for every queued check and report them in queue order. Returns 1 if any
# failed, 0 when all passed or skipped, matching `step`.
step_wait() {
  local rc=0 i status
  [ -n "$_HOOK_BG_DIR" ] || return 0
  wait
  for i in "${!_HOOK_BG_LABEL[@]}"; do
    status="$(cat "$_HOOK_BG_DIR/$i.rc" 2>/dev/null || true)"
    if [ -s "$_HOOK_BG_DIR/$i.out" ]; then
      cat "$_HOOK_BG_DIR/$i.out" >&2
    fi
    if [ "$status" = "0" ]; then
      pass "${_HOOK_BG_LABEL[$i]}"
    elif [ "$status" != "$HOOK_SKIPPED" ]; then
      step_failed "${_HOOK_BG_HINT[$i]}"
      rc=1
    fi
  done
  _hook_bg_cleanup
  return "$rc"
}

# ---------------------------------------------------------------------------
# Git environment
# ---------------------------------------------------------------------------

# without_git_repo_env <command>...: run a command, or a function from this
# library, in a subshell without git's repository variables.
#
# Git exports GIT_DIR into a hook started from a linked worktree, and
# GIT_INDEX_FILE into pre-commit. A tool that runs git in another directory
# under them (a test that builds a scratch repository, cargo-deny's advisory
# fetch, a build reading --show-toplevel) acts on this repository instead: it
# commits onto the branch, resets it, or resolves the wrong top level.
# Pre-push clears them at its top, because git starts a hook at the worktree
# root and discovery finds the same repository. Pre-commit keeps them and wraps
# such a tool here, because its staged set has to come from the temporary index
# of `commit -a` and `commit <paths>`.
without_git_repo_env() (
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY \
    GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_PREFIX GIT_NAMESPACE
  "$@"
)

# ---------------------------------------------------------------------------
# File selection
# ---------------------------------------------------------------------------

# Staged paths matching the given pathspecs. ACMR only: a deletion cannot fail
# a lint, and the linter would fail trying to open the missing path.
staged() { git diff --cached --name-only --diff-filter=ACMR -- "$@"; }

# Tracked paths matching the given pathspecs that still exist on disk. A file
# deleted from the working tree but not yet committed stays in the index, so
# `git ls-files` still lists it; filtering here is what --diff-filter=ACMR
# already does for the staged set.
tracked() {
  local f
  while IFS= read -r f; do
    [ -e "$f" ] && printf '%s\n' "$f"
  done < <(git ls-files -- "$@")
}

# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------

# Both preconditions every cargo step shares: the toolchain is installed and
# this is a cargo workspace. Checked by file rather than by letting cargo fail,
# so a non-Rust repo that seeded the pair reports a skip instead of an error.
cargo_ready() {
  have cargo || skip "$1" "cargo not installed" || return
  [ -f Cargo.toml ] || skip "$1" "no Cargo.toml in the repo root" || return
}

# Format-check specific files. `cargo fmt --check` is whole-crate, so a
# staged-scoped hook drives rustfmt directly. Reports every offending file
# rather than stopping at the first.
check_rustfmt() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  have rustfmt || skip "rustfmt" "install via \`rustup component add rustfmt\`" || return
  local f rc=0
  for f in "$@"; do
    [ -f "$f" ] || continue
    rustfmt --edition 2024 --check "$f" || rc=1
  done
  return "$rc"
}

# Whole-crate format check, the pre-push mirror of check_rustfmt.
check_cargo_fmt() {
  cargo_ready "fmt" || return
  cargo fmt -- --check
}

# Warnings-as-errors, CI's RUSTFLAGS=-Dwarnings passed as a clippy argument.
# Cargo keys every unit, dependencies included, on RUSTFLAGS, so setting it here
# would check every dependency again under flags no other local build uses.
# Arguments are cargo's and go ahead of the `--`.
check_clippy() {
  cargo_ready "clippy" || return
  cargo clippy --quiet "$@" -- -D warnings 2>&1
}

check_tests() {
  cargo_ready "test" || return
  cargo test --quiet "$@" 2>&1
}

# Doc-comment breakage (broken intra-doc links, missing code-fence markers)
# that would render badly on docs.rs.
check_doc() {
  cargo_ready "doc" || return
  RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --quiet 2>&1
}

# The MSRV a crate promises its consumers. Empty when the manifest declares
# none. Read separately from check_msrv so a caller can name the version in the
# step label.
msrv_version() {
  awk -F\" '/^rust-version *= *"/ {print $2; exit}' Cargo.toml 2>/dev/null || true
}

# Compile against the rust-version declared in Cargo.toml. A library crate
# declares a lower MSRV than the rust-toolchain.toml pin, and this validates
# the promise a consumer on that MSRV depends on.
check_msrv() {
  local msrv
  cargo_ready "msrv" || return
  msrv=$(msrv_version)
  [ -n "$msrv" ] || skip "msrv" "no rust-version in Cargo.toml" || return
  # Here-string, not a pipe: `grep -q` exits at its first match, and under
  # `pipefail` a producer killed that way makes a matching pipeline non-zero.
  # See references/pre-push-hook-steps.md § "A pipeline's exit status is not a
  # verdict". rustup's output is far too small to trip it today; the form is
  # what keeps that true when a check like this is copied somewhere it matters.
  grep -q "^${msrv}" <<<"$(rustup toolchain list 2>/dev/null)" \
    || skip "msrv (${msrv})" "install via \`rustup toolchain install ${msrv}\`" || return
  cargo "+${msrv}" check --quiet 2>&1
}

# Licences and advisories.
check_deny() {
  cargo_ready "deny" || return
  cargo deny --version >/dev/null 2>&1 \
    || skip "deny" "install via \`cargo install cargo-deny\`" || return
  cargo deny check 2>&1
}

# Print the resolved toolchain so a stale machine sees the version at the top of
# the run. Rustup installs the rust-toolchain.toml pin on the first cargo
# invocation, so this is a banner, never a gate.
show_toolchain() {
  pass "toolchain ($(rustc --version 2>/dev/null | awk '{print $2}' || echo unknown))"
}

# ---------------------------------------------------------------------------
# Shell
# ---------------------------------------------------------------------------

# Correctness. severity=warning catches real bugs and leaves info/style noise
# alone; raise it to style once a repo is clean there.
check_shell() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  have shellcheck || skip "shellcheck" "install via \`brew install shellcheck\`" || return
  shellcheck --severity=warning "$@"
}

# Layout. shfmt owns formatting, shellcheck owns correctness. -d reports a diff
# rather than rewriting staged content out from under the commit.
check_shfmt() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  have shfmt || skip "shfmt" "install via \`brew install shfmt\`" || return
  shfmt -i 2 -ci -bn -d "$@"
}

# The tracked shell set: *.sh plus the hooks themselves, which carry no
# extension. The *.sh glob never matches *.bats, which shfmt cannot parse, so
# that exclusion is by file selection rather than by flag.
tracked_shell() {
  tracked '*.sh' 'scripts/hooks/pre-commit' 'scripts/hooks/pre-push' 'scripts/hooks/_lib.sh' | sort -u
}

check_shell_all() {
  local files
  mapfile -t files < <(tracked_shell)
  check_shell "${files[@]}"
}

check_shfmt_all() {
  local files
  mapfile -t files < <(tracked_shell)
  check_shfmt "${files[@]}"
}

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

# No test resets HOME, USER, TMPDIR, an XDG_* directory, a tool home, or the
# whole value of PATH. The guard ignores paths that are not test files and
# reads its allowlist from beside itself. The sweep also fails on allowlist
# entries that no longer match a finding.
check_core_env() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  [ -x scripts/hooks/core-env-guard.sh ] || skip "core-env guard" "scripts/hooks/core-env-guard.sh is not in this checkout" || return
  ./scripts/hooks/core-env-guard.sh "$@"
}

check_core_env_all() {
  [ -x scripts/hooks/core-env-guard.sh ] || skip "core-env guard" "scripts/hooks/core-env-guard.sh is not in this checkout" || return
  ./scripts/hooks/core-env-guard.sh
}

# ---------------------------------------------------------------------------
# Workflows and markdown
# ---------------------------------------------------------------------------

check_actionlint() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  have actionlint || skip "actionlint" "install via \`brew install actionlint\`" || return
  actionlint "$@"
}

check_actionlint_all() {
  local files
  mapfile -t files < <(tracked '.github/workflows/*.yml' '.github/workflows/*.yaml')
  check_actionlint "${files[@]}"
}

# Concurrency-group namespacing. The file list gates whether it runs; the script
# reads every workflow itself, because the rule is a property of the set. Its
# success line goes to stdout and its findings to stderr, so dropping stdout
# keeps a pass as quiet as every other check and leaves a failure loud.
check_workflow_concurrency() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  [ -x scripts/check-workflow-concurrency.sh ] \
    || skip "concurrency namespacing" "scripts/check-workflow-concurrency.sh is not in this checkout" || return
  ./scripts/check-workflow-concurrency.sh >/dev/null
}

check_workflow_concurrency_all() {
  local files
  mapfile -t files < <(tracked '.github/workflows/*.yml' '.github/workflows/*.yaml')
  check_workflow_concurrency "${files[@]}"
}

# Markdown syntax. Refuses to run when the repo's markdownlint config sets
# `fix: true`, because markdownlint-cli2 then rewrites every file it reads, with
# or without --fix. A gate that mutates the tree mid-commit stages content
# nobody reviewed, and it corrupts any file whose bytes are load-bearing: a
# vendored upstream copy, a fixture, anything with a recorded checksum. A repo
# that wants the edit-time auto-fix ergonomic keeps `fix: true` and loses this
# check, which is the trade the notice states out loud.
check_markdown() {
  [ "$#" -gt 0 ] || return "$HOOK_SKIPPED"
  have markdownlint-cli2 || skip "markdownlint" "install via \`brew install markdownlint-cli2\`" || return
  local cfg
  for cfg in .markdownlint-cli2.yaml .markdownlint-cli2.yml .markdownlint-cli2.jsonc .markdownlint-cli2.json; do
    [ -f "$cfg" ] || continue
    if grep -Eq '^[[:space:]]*"?fix"?[[:space:]]*:[[:space:]]*true' "$cfg"; then
      skip "markdownlint" "$cfg sets fix: true, which would rewrite files mid-commit" || return
    fi
  done
  markdownlint-cli2 "$@"
}
