#!/usr/bin/env bash
# Canonical version: 2026.11.01 (rust)
#
# Filter changed paths down to the ones that can change what cargo builds,
# tests, documents, or packages. Reads paths on stdin, one per line, and prints
# the cargo inputs among them in their input order; prints nothing for a change
# that is documentation only.
#
# A markdown file is documentation only when it sits outside crates/, its file
# name is not a README (cargo packages one from a package root even when the
# manifest names none), and no tracked Rust source names that file name in a
# string literal (a test that reads a document, an `include_str!`). The names
# are collected on every run, so a test that starts reading a document is
# covered the day it lands. Matching is by file name, so `../AGENTS.md` covers
# every AGENTS.md. Every path under crates/ is an input, markdown included,
# because cargo packages a workspace member's files.
#
# A few other paths no build, test, or package step reads are documentation
# too: the gitignored .context/, the license texts, and the .github files no
# test reads (issue forms, rulesets, the Dependabot config). Workflows stay
# inputs, since a repo's tests may read them.
#
# Every other path is a cargo input, including one this script has never seen.
# The pre-push hook and CI's change-scope job skip the Rust battery on this
# verdict, and a wrong skip costs more than a wrong run.
#
# Exit codes: 0 = filtered (the output may be empty), 2 = not in a git work
# tree.
set -euo pipefail

root=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "cargo-inputs: not inside a git work tree" >&2
  exit 2
}
cd "$root"

read_by_cargo=$(
  {
    echo README.md
    git ls-files -z -- '*.rs' | xargs -0 -r grep -hoE '"[A-Za-z0-9_./-]*\.md"' | tr -d '"'
  } | sed 's#.*/##' | LC_ALL=C sort -u
) || true

while IFS= read -r path; do
  [ -n "$path" ] || continue
  case "$path" in
    crates/*) ;;
    *.md) grep -qxF -- "${path##*/}" <<<"$read_by_cargo" || continue ;;
    .context/* | LICENSE-* | .github/ISSUE_TEMPLATE/* | .github/rulesets/* | .github/dependabot.yml) continue ;;
  esac
  printf '%s\n' "$path"
done
