#!/usr/bin/env bash
# Canonical version: 2026.11.03 (rust)
#
# Filter changed paths down to the ones that can change what cargo builds,
# tests, documents, or packages. Reads paths on stdin, one per line, and prints
# the cargo inputs among them in their input order; prints nothing for a change
# that is documentation only.
#
# Rules, in the order they apply:
#
#   1. Every path under crates/ is an input, markdown included, because cargo
#      packages a workspace member's files.
#   2. A path some tracked Rust source reads is an input, whatever its type:
#      - its file name matches the file name of a string literal (a test that
#        reads a document, an `include_str!`), so `../AGENTS.md` covers every
#        AGENTS.md. README.md always counts: cargo packages one from a package
#        root even when the manifest names none.
#      - a string literal holding a `/` names the path itself, a directory
#        above it, or a trailing part of it, so a build script that walks
#        `src/spec/` through `fs::read_dir` covers every file under it. The
#        slash is read before normalizing, so a one-part literal such as
#        `"/docs"` (from `concat!(env!("CARGO_MANIFEST_DIR"), "/docs")`),
#        `"spec/"`, or `"./tools"` names its directory.
#      - an entry of scripts/cargo-inputs.paths does the same. The file is the
#        repository's own: one path or directory per line, `#` comments and
#        blank lines ignored, for an input no literal names (a path assembled
#        at run time).
#      Literals are normalized before matching: leading `/`, `./`, and `../`
#      and trailing `/` are stripped, and an empty result names nothing.
#   3. Any other markdown file is documentation only.
#   4. A few other paths no build, test, or package step reads are
#      documentation too: the gitignored .context/, the license texts, and the
#      .github files no test reads (issue forms, rulesets, the Dependabot
#      config). Rule 2 runs first, so a test that reads one keeps it an input.
#      Workflows stay inputs, since a repo's tests may read them.
#   5. Every other path is an input, including one this script has never seen.
#
# The literals are collected on every run, so a test that starts reading a
# file is covered the day it lands. The pre-push hook and CI's change-scope job
# skip the Rust battery on this verdict, and a wrong skip costs more than a
# wrong run, which is why matching errs toward an input.
#
# Exit codes: 0 = filtered (the output may be empty), 2 = not in a git work
# tree.
set -euo pipefail

root=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "cargo-inputs: not inside a git work tree" >&2
  exit 2
}
cd "$root"

# Strips leading `/`, `./`, `../` and trailing `/` from each line, and drops
# lines left empty.
normalize() {
  sed -E -e ':a' -e 's#^\.{0,2}/##' -e 'ta' -e 's#/+$##' | grep -v '^$' || true
}

raw=$(git ls-files -z -- '*.rs' | xargs -0 -r grep -hoE '"[A-Za-z0-9_./-]+"' | tr -d '"') || true
literals=$(printf '%s\n' "$raw" | normalize)
paths=$({ printf '%s\n' "$raw" | grep / || true; } | normalize)

declared=''
if [ -f scripts/cargo-inputs.paths ]; then
  declared=$(sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' scripts/cargo-inputs.paths | normalize)
fi

# The read set, one entry per line, tagged `N ` for a file name and `P ` for a
# path or directory. Each entry carries its own kind rather than sitting below a
# separator line, because any separator could also be a literal (`"--"` is a
# common one).
read_set() {
  printf 'N README.md\n'
  printf '%s\n' "$literals" | sed -e 's#.*/##' -e 's/^/N /'
  printf '%s\n' "$paths" | LC_ALL=C sort -u | sed 's/^/P /'
  printf '%s\n' "$declared" | sed 's/^/P /'
}

awk '
  FNR == NR {
    entry = substr($0, 3)
    if (entry == "") next
    if (substr($0, 1, 1) == "N") name[entry] = 1
    else if (!(entry in seen)) { seen[entry] = 1; read_path[++n] = entry }
    next
  }
  function read_by_rust(p,   base, i, d, lp, ld) {
    base = p
    sub(/.*\//, "", base)
    if (base in name) return 1
    lp = length(p)
    for (i = 1; i <= n; i++) {
      d = read_path[i]
      ld = length(d)
      if (p == d) return 1
      if (lp > ld && substr(p, 1, ld + 1) == d "/") return 1
      if (lp > ld && substr(p, lp - ld) == "/" d) return 1
    }
    return 0
  }
  $0 == "" { next }
  /^crates\// { print; next }
  read_by_rust($0) { print; next }
  /\.md$/ { next }
  /^(\.context\/|LICENSE-|\.github\/ISSUE_TEMPLATE\/|\.github\/rulesets\/)/ { next }
  $0 == ".github/dependabot.yml" { next }
  { print }
' <(read_set) -
