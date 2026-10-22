#!/usr/bin/env bash
# Filter changed paths down to the ones that can change what cargo builds,
# tests, documents, or packages. Reads paths on stdin, one per line, and
# prints the cargo inputs among them in their input order; prints nothing for
# a change that is documentation only.
#
# A path is documentation only when it is markdown and its file name is
# neither named in a string literal by a tracked Rust source (a test that
# reads a document, an `include_str!`) nor a README, which cargo packages
# from a package root even when the manifest names none. The names are
# collected on every run, so a test that starts reading a document is
# covered the day it lands. Matching is by file name, so `../AGENTS.md`
# covers every AGENTS.md.
#
# A few other paths no build, test, or package step reads are documentation
# too: the gitignored .context/, the license texts, and the .github files no
# test reads (issue forms, rulesets, the Dependabot config). Workflows stay
# cargo inputs because tests/hook_ci_parity.rs reads them.
#
# Every other path is a cargo input, including one this script has never
# seen. CI's `Change scope` job and the pre-push hook skip the Rust battery on
# this verdict, and a wrong skip costs more than a wrong run.
#
# Exit codes: 0 = filtered (the output may be empty), 2 = not in a git work
# tree.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 2

read_by_cargo=$(
  {
    echo README.md
    git ls-files -z -- '*.rs' | xargs -0 -r grep -hoE '"[A-Za-z0-9_./-]*\.md"' | tr -d '"'
  } | sed 's#.*/##' | LC_ALL=C sort -u
) || true

while IFS= read -r path; do
  [ -n "$path" ] || continue
  case "$path" in
    *.md) grep -qxF -- "${path##*/}" <<<"$read_by_cargo" || continue ;;
    .context/* | LICENSE-* | .github/ISSUE_TEMPLATE/* | .github/rulesets/* | .github/dependabot.yml) continue ;;
  esac
  printf '%s\n' "$path"
done
