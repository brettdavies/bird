#!/usr/bin/env bash
# Fail a pull request whose `bird` surface grows while its `## Changelog`
# block names no minor-or-higher section.
#
# Each PR's changelog sections decide the release's version bump, and an
# addition to the contract is a minor. An addition filed under `### Changed`
# reads as a patch and nothing in review makes it stand out, so this compares
# the surface two trees describe:
#
#   - every command, subcommand, flag, and enumerated flag value, read from
#     completions/bird.bash, which the `Completions freshness` job holds equal
#     to the clap definition;
#   - every leaf of every schema under schema/ except `description` and
#     `title`, so a new envelope key, `reason`, `action`, or response field
#     counts and reworded prose does not.
#
# Growth passes when the block has a bullet under `### Added`, `### Deprecated`,
# or `### Breaking changes`. A removal only warns: a change back to the
# documented contract can ship as a patch, and the release preflight diffs
# every removal against the last tag. Env vars, exit codes, and the store
# format have no generated artifact and are not compared.
#
# Usage:
#   check-surface-bump.sh surface <tree>
#   check-surface-bump.sh check <base-tree> <head-tree> <pr-body-file>
#
# Exit codes: 0 = pass, 1 = the surface grew and the block lacks the section,
# 2 = a usage error or a tree the parser cannot read.
set -euo pipefail

readonly MINOR_OR_HIGHER='Added|Deprecated|Breaking changes'
readonly LISTED=20

die() {
  echo "check-surface-bump: $1" >&2
  exit 2
}

# annotate <level> <message> — an Actions annotation in CI, plain text elsewhere.
annotate() {
  if [ "${GITHUB_ACTIONS:-}" = true ]; then
    echo "::$1 title=Changelog bump::$2" >&2
  else
    echo "$1: $2" >&2
  fi
}

# surface <tree> — one line per contract element, unsorted.
surface() {
  local tree="$1" completions="$1/completions/bird.bash" cli file
  [ -f "$completions" ] || die "no completions/bird.bash under $tree"
  [ -d "$tree/schema" ] || die "no schema/ under $tree"

  # clap_complete writes one `case "${cmd}"` arm per command path, labelled
  # with the words joined by `__subcmd__`; each arm lists its words in
  # `opts` and the values of an enumerated flag in a `compgen -W` under that
  # flag's own arm.
  cli=$(awk '
        /^    case "\$\{cmd\}" in$/ { dispatch = 1; next }
        !dispatch { next }
        /^        [A-Za-z0-9_]+\)$/ {
            cmd = $1; sub(/\)$/, "", cmd); gsub(/__subcmd__/, " ", cmd); flag = ""; next
        }
        /^            opts="/ {
            line = $0; sub(/^ *opts="/, "", line); sub(/"$/, "", line)
            n = split(line, words, " ")
            for (i = 1; i <= n; i++) printf "cli\t%s\t%s\n", cmd, words[i]
            next
        }
        /^                -[^ ]*\)$/ { flag = $1; sub(/\)$/, "", flag); next }
        flag != "" && /compgen -W "/ {
            line = $0; sub(/.*compgen -W "/, "", line); sub(/".*/, "", line)
            n = split(line, values, " ")
            for (i = 1; i <= n; i++) printf "cli\t%s\t%s=%s\n", cmd, flag, values[i]
            flag = ""; next
        }
    ' "$completions")
  [ -n "$cli" ] || die "found no commands in $completions; did the clap_complete output format change?"
  printf '%s\n' "$cli"

  # Array indices become `[]` so reordering a list is not growth; the value
  # is JSON-encoded so every leaf stays on one line.
  while IFS= read -r file; do
    jq -r --arg file "$file" '
            paths(scalars) as $p
            | select($p[-1] != "description" and $p[-1] != "title")
            | "schema\t\($file)\t\($p | map(if type == "number" then "[]" else . end) | join("."))=\(getpath($p) | tojson)"
        ' "$tree/$file" || die "cannot read $tree/$file as JSON"
  done < <(cd "$tree" && find schema -type f -name '*.json' | LC_ALL=C sort)
}

# filled_sections <head-tree> <body-file> — each `###` heading in the bird
# changelog block that holds at least one bullet. The block is read by the
# release's own parser, `extract_changelog_sections` in generate-changelog.py,
# under `## Changelog` or the heading a `[package.metadata.changelog]` table in
# Cargo.toml names, so this check and the release notes cannot disagree about
# which sections a PR filled.
filled_sections() {
  python3 -B - "$1" "$2" <<'PY'
import importlib.util
import re
import sys
import types
from pathlib import Path

# generate-changelog.py imports tomllib for its cliff.toml reader, which this
# check never calls, and Python 3.10 (the reusable CI's ubuntu-22.04) has no
# tomllib; the heading below is read without it for the same reason.
try:
    import tomllib  # noqa: F401
except ModuleNotFoundError:
    sys.modules["tomllib"] = types.ModuleType("tomllib")

tree, body = Path(sys.argv[1]), Path(sys.argv[2])
spec = importlib.util.spec_from_file_location(
    "generate_changelog", tree / "scripts/generate-changelog.py"
)
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


def table_value(manifest: str, table: str, key: str) -> str | None:
    """A quoted string value under `[table]` in a Cargo manifest."""
    section = re.search(
        rf"^\[{re.escape(table)}\][ \t]*$(.*?)(?=^\[|\Z)", manifest, re.M | re.S
    )
    if section is None:
        return None
    value = re.search(rf'^{re.escape(key)}[ \t]*=[ \t]*"([^"]*)"', section.group(1), re.M)
    return value.group(1) if value else None


manifest = (tree / "Cargo.toml").read_text(encoding="utf-8")
label = table_value(manifest, "package.metadata.changelog", "heading") or "Changelog"
sections = generator.extract_changelog_sections(
    body.read_text(encoding="utf-8"), rf"^## {re.escape(label)}\s*$"
)
for name, bullets in sections.items():
    if bullets:
        print(name)
PY
}

# list <lines> — the first LISTED lines, indented, and a count of the rest.
# sed reads the whole here-string: a `| head` stage would stop reading early,
# and under pipefail the writer's SIGPIPE would end the script.
list() {
  local total
  total=$(wc -l <<<"$1")
  sed -n "1,${LISTED}s/^/  /p" <<<"$1" >&2
  [ "$total" -le "$LISTED" ] || echo "  ... and $((total - LISTED)) more" >&2
}

check() {
  local base="$1" head="$2" body="$3" base_surface head_surface added removed
  [ -f "$body" ] || die "no PR body at $body"
  base_surface=$(surface "$base" | LC_ALL=C sort -u)
  head_surface=$(surface "$head" | LC_ALL=C sort -u)
  added=$(LC_ALL=C comm -13 <(printf '%s\n' "$base_surface") <(printf '%s\n' "$head_surface"))
  removed=$(LC_ALL=C comm -23 <(printf '%s\n' "$base_surface") <(printf '%s\n' "$head_surface"))

  if [ -n "$removed" ]; then
    annotate warning "the bird surface lost entries; a removal or rename belongs under ### Breaking changes unless it restores the documented contract"
    list "$removed"
  fi

  if [ -z "$added" ]; then
    echo "check-surface-bump: the bird surface did not grow"
    return 0
  fi

  local sections
  sections=$(filled_sections "$head" "$body") \
    || die "could not read the changelog block with $head/scripts/generate-changelog.py"
  if grep -qxE "$MINOR_OR_HIGHER" <<<"$sections"; then
    echo "check-surface-bump: the bird surface grew and the changelog block names a minor-or-higher section"
    return 0
  fi

  annotate error "the bird surface grew, so the changelog block needs a bullet under ### Added (or ### Deprecated or ### Breaking changes); an addition is a minor"
  list "$added"
  return 1
}

command -v jq >/dev/null || die "jq is not installed"

case "${1:-}" in
  surface)
    [ "$#" -eq 2 ] || die "usage: $0 surface <tree>"
    surface "$2"
    ;;
  check)
    [ "$#" -eq 4 ] || die "usage: $0 check <base-tree> <head-tree> <pr-body-file>"
    command -v python3 >/dev/null || die "python3 is not installed"
    check "$2" "$3" "$4"
    ;;
  *)
    die "usage: $0 surface <tree> | check <base-tree> <head-tree> <pr-body-file>"
    ;;
esac
