#!/usr/bin/env bash
# Canonical version: 2026.11.03 (rust)
#
# Output discipline guard for a Rust CLI. Two invariants:
#
# 1. No `println!`, `eprintln!`, `print!`, `eprint!`, or a macro EXTRA_MACROS
#    names appears in the searched trees outside the files allowed to print, so
#    every user-visible line goes through the CLI's output layer.
# 2. When library globs are set, no library module reaches a terminal handle
#    (`io::stdout()`, `io::stderr()`, their types, or a raw fd): the binary owns
#    the terminal, and a library reports through its caller or `tracing`.
#
# Doc comments and ordinary comments are ignored unless MATCH_COMMENTS is true,
# in which case a commented-out macro call is a finding too. Skipping comments
# takes a lookahead, so only then does a search need a ripgrep built with PCRE2.
#
# Skeleton: set the CUSTOMIZE values below for this repository.
#
# Exit codes: 0 = clean, 1 = a forbidden macro or handle, named with its file
# and line, 2 = rg is not installed or a search failed (rg's own error, then
# the fix).
set -euo pipefail

# CUSTOMIZE: the trees to search, the files allowed to print (rg globs, each
# relative to the repository root), the repo's own print macros to forbid
# beside the four std ones (names without the `!`, such as a retired output
# macro), and the library sources that must never reach a terminal handle
# (empty to skip invariant 2).
SEARCH_PATHS=(src)
PRINT_ALLOWED=('src/main.rs' 'src/error/fatal.rs')
EXTRA_MACROS=(out_println out_print diag)
LIBRARY_GLOBS=()
MATCH_COMMENTS=true

command -v rg >/dev/null || {
  echo "lint-stdio: rg is not installed (brew install ripgrep)" >&2
  exit 2
}

cd "$(dirname "$0")/.."

not_comment='^(?!\s*///|\s*//!|\s*//|\s*\*)'
pcre=(--pcre2)
if [ "$MATCH_COMMENTS" = true ]; then
  not_comment=''
  pcre=()
fi

# search VAR ARGS...: runs rg over the search paths and stores its matches in
# VAR. rg exits 1 when nothing matches and 2 or more on an error, such as a
# ripgrep built without PCRE2; an error ends the run, since its empty output
# would otherwise read as clean. The search paths are passed explicitly: with
# no path and a non-terminal stdin, rg searches stdin instead of the tree, and a
# pre-push hook's stdin is git's ref list.
search() {
  local var=$1 out rc=0
  shift
  out=$(rg --no-heading --line-number ${pcre[@]+"${pcre[@]}"} "$@" "${SEARCH_PATHS[@]}") || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "lint-stdio: rg exited $rc; its error is above" >&2
    echo "fix: install a ripgrep built with PCRE2 (brew install ripgrep), or set MATCH_COMMENTS=true so no search needs PCRE2" >&2
    exit 2
  fi
  printf -v "$var" '%s' "$out"
}

exclusions=()
for glob in "${PRINT_ALLOWED[@]}"; do
  exclusions+=(--glob "!$glob")
done

forbidden=(println eprintln print eprint ${EXTRA_MACROS[@]+"${EXTRA_MACROS[@]}"})
alternation=$(
  IFS='|'
  echo "${forbidden[*]}"
)

search macros --glob '*.rs' "${exclusions[@]}" "${not_comment}.*\b(${alternation})!"
if [ -n "$macros" ]; then
  echo "FAIL: $(printf '%s!/' "${forbidden[@]}" | sed 's|/$||') outside ${PRINT_ALLOWED[*]}:" >&2
  echo "$macros" >&2
  echo "Route the line through the CLI's output layer, or add its file to PRINT_ALLOWED." >&2
  exit 1
fi

if [ "${#LIBRARY_GLOBS[@]}" -gt 0 ]; then
  library=()
  for glob in "${LIBRARY_GLOBS[@]}"; do
    library+=(--glob "$glob")
  done
  search handles "${library[@]}" \
    "${not_comment}.*(\bio::(stdout|stderr)\(\)|\bio::(Stdout|Stderr)\b|\bfrom_raw_fd\b)"
  if [ -n "$handles" ]; then
    echo "FAIL: terminal handle reached from library code (${LIBRARY_GLOBS[*]}):" >&2
    echo "$handles" >&2
    echo "Return the output to the caller, or emit a tracing event." >&2
    exit 1
  fi
fi

echo "OK: no stdio macros outside ${PRINT_ALLOWED[*]}"
