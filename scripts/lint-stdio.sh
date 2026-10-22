#!/usr/bin/env bash
# Output discipline guard: writer injection.
#
# Every call site that emits user-visible output writes to an injected
# `&mut dyn Write` or calls an `OutputConfig::print_*` method. No
# `println!`, `eprintln!`, `print!`, or `eprint!` appears under src/, and
# neither do the retired `out_println!`, `out_print!`, and `diag!` macros.
# Two files are exempt:
#
#   - src/main.rs, the binary shim
#   - src/error/fatal.rs, the fatal-error chokepoint, bare by design
#
# Anywhere else in src/, src/error/ included, the guard fires. Comments are
# matched too, so a commented-out macro call is a finding.
#
# Exit codes: 0 = clean, 1 = a forbidden macro, named with its file and line.
set -euo pipefail

cd "$(dirname "$0")/.."

# `src` is passed explicitly: with no path and a non-terminal stdin, rg
# searches stdin instead of the tree, and a pre-push hook's stdin is git's
# ref list.
forbidden=$(
  rg -n '\b(println|eprintln|print|eprint|out_println|out_print|diag)!' \
    --glob '*.rs' \
    --glob '!src/main.rs' \
    --glob '!src/error/fatal.rs' \
    src || true
)

if [ -n "$forbidden" ]; then
  echo "FAIL: output macro in src/ outside src/main.rs and src/error/fatal.rs:" >&2
  echo "$forbidden" >&2
  echo "" >&2
  echo "Write to an injected &mut dyn Write writer or call an OutputConfig::print_* method." >&2
  exit 1
fi

echo "OK: no output macros in src/ outside src/main.rs and src/error/fatal.rs"
