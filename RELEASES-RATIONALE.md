# Releases rationale

Companion to [`RELEASES.md`](./RELEASES.md). RELEASES.md is the runbook (commands, paths, decision tables). This file
holds the WHY behind those rules: branching model, PR conventions, release pipeline, CHANGELOG generation, prose-check
pipeline, branch-protection pitfalls.

Read this when:

- A rule in RELEASES.md doesn't make sense and you're tempted to change it.
- A new contributor asks "why do we do X this way".
- You're adding a new release-flow rule and need to know where it fits the existing model.

## Branching model

### Forever `dev`, ephemeral release branches

`dev` is never deleted, even after a release. The next release cycle reuses the same `dev`. The repo's
`deleteBranchOnMerge: true` setting doesn't touch `dev` as long as `dev` is never the head of a PR. Using a short-lived
`release/*` head is what keeps the setting compatible with a forever integration branch.

Engineering docs (`docs/plans/`, `docs/solutions/`, `docs/brainstorms/`, `docs/reviews/`) live on `dev` only. They never
reach `main`. `guard-main-docs.yml` blocks them from PRs targeting `main`, and `guard-release-branch.yml` rejects any PR
to main whose head isn't `release/*`.

### Why cherry-pick from `main`, not branch from `dev`

Branching from `dev` and then `gio trash`-ing the guarded paths seems simpler but produces `add/add` merge conflicts
whenever `dev` and `main` have diverged (which they always do after the first squash merge). The file appears as "added"
on both sides with different content. Always branch from `origin/main` and cherry-pick the dev commits onto it.

### Version branch naming

Branch naming `release/v<version>` or `release/v<version>-<slug>` (e.g. `release/v0.2.0`,
`release/v0.2.0-watchlist-api`) makes release branches sortable and unambiguous when multiple cuts are in flight. The
`v<version>` prefix is required: `generate-changelog.sh` extracts the version from the branch name. Slug is kebab-case,
short, descriptive.

## PR body conventions

### No explainer prose in the body

Every section of a PR body is user-facing substance only: the **net diff**, what is changing for the consumer that was
not already there, not the commit history or intermediate state that produced it. Workflow mechanics (cherry-pick,
regenerate, pre-push gate, CI behavior) are documented in RELEASES.md and `.github/`, NOT in the PR body. Triple-diff
output ("A: 12 files, B: none, C: clean"), leak-check narration ("`guard-main-docs` runs clean", "no guarded paths
leaked"), patch-id cherry-check counts, pre-push gate results, CI check status, exclusion rationale, and other
verification artifacts stay local; anomalies get fixed before push, not audit-trailed in the body.

The PR body is read by humans reviewing what shipped. Workflow mechanics and tool-fix provenance are noise from that
perspective; they belong in this file, the script outputs, and the commit history respectively.

### Why `feat`/`fix` are preferred over `chore`

`cliff.toml` drops commits whose subject starts with `chore`, `style`, `test`, `ci`, or `build` regardless of body
content. Mistyping a user-facing change as `chore` silently strips it from release notes. Prefer `feat` / `fix` when the
change has any user-observable effect (config defaults, env vars, default behaviors, new shortcut commands, output
format changes, cache mode changes).

Security advisory bumps in particular use `fix(deps):`, never `chore(deps):`, so they appear in the changelog. A bumped
dependency that closes a CVE is user-visible value, not internal tooling.

### Why required-when-empty sub-headers

`Related Issues/Stories` has four labels (`Story:` / `Issue:` / `Architecture:` / `Related PRs:`). `Files Modified` has
four sub-headers (`Modified` / `Created` / `Renamed` / `Deleted`). All four must appear in every PR, even when empty:
write `- None.` or `n/a` rather than deleting the label. Reason: scanners and humans both rely on a known section shape.
Conditionally-absent sections force every reader to mentally check "did the author skip this or does it not apply?"

### Why no AI attribution

`Co-Authored-By: Claude …`, robot emoji / "Generated with Claude Code" trailers, or any similar AI-attribution trailer
is banned from commit messages and PR bodies. Commits and PRs stand on their own technical content. Attribution trailers
are noise and they age poorly as tools shift.

### Why no hard line wraps

Author each paragraph and each bullet as one logical line, however long. GitHub soft-wraps for display. Hard wraps
within prose produce visible mid-sentence breaks in some renderers and interfere with the prose-check pipeline: Vale's
line-anchored output reports findings against split lines, and LanguageTool's input handling can choke on certain
control-char interactions. The auto-format hook skips `/tmp/` paths so the body keeps its authored shape; don't undo
that with manual wrapping during composition. Same rule applies to commit messages composed via heredoc.

### Why release-PR bodies repeat changelog entries from upstream PRs

The release PR carries the same `### Added` / `### Changed` / `### Fixed` / `### Documentation` bullets as the feature
PRs it cherry-picks. The repetition is intentional and harmless: `cliff.toml` already skips its own changelog-update
commit, so the release-PR squash commit can't be double-counted in any future regeneration.

### Why internal-tooling commits don't appear in `## Changelog`

`chore(cliff): ...`, `chore(ci): ...`, and similar internal-tooling commits don't appear in the PR body's `##
Changelog`. They are not user-facing. They belong in commit history and in the Files Modified section of the PR body,
not in the source-of-truth release notes.

## Triple-diff verification

The release-PR procedure runs three diffs (A: main→release, B: release→dev for non-doc paths, C: dev→main) plus a
patch-id cherry check. This is belt-and-suspenders because missed cherry-picks have shipped to `main` on this and
sibling repos before, and the file-level diff in B alone doesn't catch the patch-id false-negative class.

### Why patch-id cherry-check output is noisy

In a squash-merge workflow, `git cherry HEAD origin/dev` produces many `+` lines that need human triage. They do NOT
auto-block the release. Expected sources of false positives:

1. **Historical commits squash-merged in prior releases.** The squash commit on main has a different patch-id than the
   dev commits it consolidates, so old commits show as `+` forever. Anything older than the previous release tag is
   almost always this.
2. **Cherry-picks where conflict resolution stripped guarded paths** (`docs/plans/`, `docs/brainstorms/`, etc.) or
   otherwise altered the tree. Same source-code intent, different patch-id.
3. **Intentionally skipped commits** (docs-only commits, release-prep backports, revert-and-redo prep steps).

A real miss looks like: a recent feat/fix/chore commit on dev whose *file content* is not yet on main. To triage a `+`
line:

```bash
git show <sha> --stat                       # what did it touch?
git diff origin/main..HEAD -- <those-files> # already on release?
```

If every touched file is guarded (`docs/plans/`, `docs/brainstorms/`, etc.) OR the content is already on main via a
prior squash, it's a false positive (no action). Otherwise cherry-pick the commit and re-run the triple-diff.

## CHANGELOG generation

### Generated, never hand-written

`scripts/generate-changelog.sh` (vendored from the `rust-tool-release` skill, with the repo-local `cliff.toml`) is the
only sanctioned way to update `CHANGELOG.md`. The script runs `git-cliff` to prepend a versioned entry for commits since
the last tag, then walks each squash-merged PR's body to extract the `## Changelog → ### Added / Changed / Fixed /
Documentation` subsections, replacing the auto-generated bullets with the curated PR-body content (with author and
PR-link attribution).

If a PR's `## Changelog` section is empty, that PR's entry is omitted from the changelog (empty section = no user-facing
change). To fix a wrong CHANGELOG entry, fix the input: edit the squash-merged PR body, then re-run the script. Do
**not** edit `CHANGELOG.md` directly.

CI enforces that `CHANGELOG.md` is modified in every PR to main (`ci / Changelog` required status check) and that it
contains a versioned section, not `[Unreleased]`. The release workflow extracts the latest section for the GitHub
Release body.

### Why `cliff.toml` skips chore/style/test/ci/build

These commit types do not produce user-facing content. If a cherry-picked PR has user-facing `## Changelog` content but
its commit subject starts with one of those types, its bullets get silently dropped. After running the script,
cross-check the generated section against `gh pr view <num> --json body` for each cherry-picked PR; correct mistyped PR
titles (e.g. `chore` → `feat`) and re-amend the cherry-pick subject before re-running. See "Prefer `feat`/`fix` over
`chore`" in global CLAUDE.md for prevention.

## Release pipeline

### Annotated tags + Trusted Publishing

Always use annotated tags (`-a -m`). Bare `git tag <name>` silently fails with `fatal: no tag message?` on machines
where `tag.gpgsign=true` is set globally (a brettdavies dotfile default). See
[solutions: git tag fails with tag.gpgsign, use annotated tags](https://github.com/brettdavies/solutions-docs/blob/main/best-practices/git-tag-fails-with-tag-gpgsign-use-annotated-tags-2026-04-13.md).

Subsequent releases use the OIDC Trusted Publishing flow built into `release.yml`: no static token in CI. The initial
publish (`v0.1.0` for this repo) required a regular crates.io API token because Trusted Publishing needs the crate to
exist first; that token has since been removed.

### Why `make_latest: false` then `finalize-release`

The GitHub Release is created visible-but-not-latest (`make_latest: false`) so `cargo-binstall` and `/releases/latest`
don't 404 during the bottle-build window, but the release isn't yet promoted to "Latest" while bottles upload. After the
homebrew-tap workflow uploads bottles to this repo's release assets, it dispatches `finalize-release` back to this repo,
which idempotently flips `make_latest: true`. End result: crate on crates.io, GitHub Release marked latest, Homebrew
formula updated with bottles, all atomically advertised.

### Why backport `main` → `dev` after publish

Once `finalize-release.yml` has flipped the GitHub Release to `published`, the release-bookkeeping files on `main`
(`Cargo.toml` version, `Cargo.lock`, `CHANGELOG.md`) need to reach `dev` so future builds from `dev` report the released
version and so the next dev work starts from the released baseline.

This repo backports with a `git merge` from `main` into `dev`. The merge preserves full history on both branches, lands
the release commits on dev as expected, and is signed via the normal commit-signing path. No surgical script is needed;
the squash-merge structure on both sides means the merge is effectively trivial.

```bash
git checkout dev && git pull
git merge --no-ff origin/main -m "Merge remote-tracking branch 'origin/main' into dev"
git push origin dev
```

If a dependency or rust-version bump on `dev` is newer than the released version on `main`, the merge resolves cleanly
in favor of `dev`'s newer values; only `version`, `Cargo.lock`, and `CHANGELOG.md` actually change.

### Cross-compile target matrix

Five targets, no musl rows: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. bird is a CLI client over a hosted API (X / Twitter, via the `xurl`
subprocess transport); the binaries are consumed by interactive shell users on the three desktop platforms and on Linux
CI runners. The musl static-link rows (added in sibling repos for Alpine and other glibc-free hosts) carry an additional
cross-compile cost and no demand signal here yet. Add them when a real Alpine consumer surfaces.

The Windows row uses `x86_64-pc-windows-msvc` (MSVC ABI) rather than GNU because the subprocess transport spawns `xr`
(xurl-rs) via the platform-native exec path; MSVC is the path of least resistance on Windows. CI also runs a separate
`ci / Windows check` job on every PR (not just at release) so MSVC build failures surface immediately, not at tag time.

## Prose scrubbing scope

Three release-flow artifacts live outside any automated prose check and need a manual scrub before they ship:

- **PR bodies.** `gh pr create` and `gh pr edit` send body text directly to GitHub; no automated prose check has reach
  there.
- **`CHANGELOG.md`.** A generated artifact built from upstream PR bodies; it inherits whatever prose those PR bodies
  carry, so scrubbing happens at generation time on the release branch.
- **Release-PR bodies.** The `release/v<version>` PR to `main` carries contributor-authored wrap-up text composed after
  `CHANGELOG.md` has been generated, and the same out-of-repo gap applies.

The canonical Vale + LanguageTool rule packs and orchestrator behavior live in the agentnative-spec repo at
[`~/dev/agentnative-spec/docs/architecture/voice-enforcement.md`](../agentnative-spec/docs/architecture/voice-enforcement.md).
This repo does not vendor a local copy of those packs; point Vale at the spec checkout via `--config`.

Scrub-before-submit (author in `/tmp/`, scrub there, submit via `--body-file`) avoids the round-trip of "submit, scrub,
edit, scrub again". Every fix lands locally and the public PR sees only clean text. The auto-format hook skips `/tmp/`
paths so the body keeps its authored shape and no soft-wrapping is injected.

For a `CHANGELOG.md` finding, fix the upstream PR body (which `generate-changelog.sh` re-fetches every run) and
regenerate. Hand-editing `CHANGELOG.md` directly produces drift the next regeneration overwrites.

## Branch protection

### Status-check context strings

The `required_status_checks[].context` strings in `protect-main.json` MUST match exactly what GitHub publishes for each
check:

- **Inline job** (with `name:` field): published as just `<job-name>` (no workflow-name prefix).
- **Reusable-workflow caller** (`uses: .../foo.yml@ref`): published as `<caller-job-id> / <reusable-job-id-or-name>`.

Mixing these produces a stuck-but-green PR: all actual checks report green, but the ruleset waits forever on a context
that will never appear. Confirm the real contexts after a first CI run with:

```bash
gh api repos/brettdavies/bird/commits/<sha>/check-runs --jq '.check_runs[].name'
```

### Why rulesets live in-repo

Committing the JSON alongside code means ruleset changes land via the same review process as workflow changes. A
`chore(ci): tighten protect-main` change goes through dev → release/* → main like anything else.

## Related docs

- [`RELEASES.md`](./RELEASES.md) (operational runbook: commands, paths, decision tables)
- [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md) (pre-cut checklist gating the release-branch cut)
- [`README.md`](README.md) (install channels, CLI reference, X API integration)
- [`.github/pull_request_template.md`](.github/pull_request_template.md) (PR body structure with changelog sections)
