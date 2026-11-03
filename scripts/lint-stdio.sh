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
# in which case a commented-out macro call is a finding too.
#
# Skeleton: set the CUSTOMIZE values below for this repository.
#
# Exit codes: 0 = clean, 1 = a forbidden macro or handle, named with its file
# and line, 2 = rg is not installed.
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
[ "$MATCH_COMMENTS" = true ] && not_comment=''

exclusions=()
for glob in "${PRINT_ALLOWED[@]}"; do
  exclusions+=(--glob "!$glob")
done

forbidden=(println eprintln print eprint ${EXTRA_MACROS[@]+"${EXTRA_MACROS[@]}"})
alternation=$(
  IFS='|'
  echo "${forbidden[*]}"
)

# The search paths are passed explicitly: with no path and a non-terminal
# stdin, rg searches stdin instead of the tree, and a pre-push hook's stdin is
# git's ref list.
macros=$(
  rg --no-heading --line-number --pcre2 --glob '*.rs' "${exclusions[@]}" \
    "${not_comment}.*\b(${alternation})!" "${SEARCH_PATHS[@]}" || true
)
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
  handles=$(
    rg --no-heading --line-number --pcre2 "${library[@]}" \
      "${not_comment}.*(\bio::(stdout|stderr)\(\)|\bio::(Stdout|Stderr)\b|\bfrom_raw_fd\b)" "${SEARCH_PATHS[@]}" || true
  )
  if [ -n "$handles" ]; then
    echo "FAIL: terminal handle reached from library code (${LIBRARY_GLOBS[*]}):" >&2
    echo "$handles" >&2
    echo "Return the output to the caller, or emit a tracing event." >&2
    exit 1
  fi
fi

echo "OK: no stdio macros outside ${PRINT_ALLOWED[*]}"
