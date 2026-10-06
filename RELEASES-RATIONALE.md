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

### Why the release branch is cut from `main`, never from `dev`

Every release squash-merges into `main`, so `dev` and `main` diverge in history even as their content converges. In this
repo the two branches share no merge-base at all. Cutting the release branch from `dev` (or merging `dev` into `main`)
forces a 3-way merge across that divergence: `add/add` collisions on files both sides changed, plus rename/delete pairs
git cannot auto-resolve. The conflict pile is an artifact of the lineage, not of the content shipping.

Always cut the release branch from `origin/main` and bring `dev`'s content onto it as a forward diff, never by
reconciling histories. The default is the whole-tree overlay (`git checkout origin/dev -- .`, then strip the guarded
set): `main` ships `dev`'s tree minus a small, known exclusion set, so asserting that end-state directly is simpler and
safer than hand-resolving a merge. The overlay commit carries no per-PR history, so the changelog is built from the PRs
merged into `dev` since the previous release (`generate-changelog.py --from-dev-prs`) rather than from the branch's
commits; the result is the same per-PR section a cherry-picked branch would yield. Cherry-picking the dev squash-commits
is kept only as an exception for a repo with a stated reason it cannot overlay, at the cost of guarded-path conflict
handling.

Either way, the release must start from a `main` that `dev` fully contains. Security PRs, hotfixes, and config edits
land on `main` first, and both constructions take `dev`'s content for the files they touch, so anything `main` holds
that `dev` never received is reverted by the release. `scripts/release/drift.sh` lists that set and the cut waits until
it is empty.

### Why a script builds the release branch

The overlay and its checks are where a hand-typed command goes wrong quietly. `git checkout origin/dev -- .` writes
`dev`'s paths but leaves every file `main` carries and `dev` deleted, so those have to be read off a diff and removed,
and that diff and the added-docs listing both need `--no-renames`: with rename detection on, a deleted file pairs with
any similar addition, reports as `R`, and drops out of a filter on `D` or `A`. `scripts/release/cut-release-branch.sh`
asserts the tree with `git read-tree -u --reset`, which carries the deletions with no diff to read, and runs its checks
as code the `github-repo-setup` bats suite covers. The version bumps stay with the operator: which files carry a version
is project-specific, and a workspace member's next version is a judgment read from the PRs' changelog blocks, not an
input the script can take.

### Version branch naming

Branch naming `release/v<version>` or `release/v<version>-<slug>` (e.g. `release/v0.3.0`,
`release/v0.3.0-watchlist-api`) makes release branches sortable and unambiguous when multiple cuts are in flight. The
`v<version>` prefix is required: `generate-changelog.py` extracts the version from the branch name. Slug is kebab-case,
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

`cliff.toml` drops commits whose subject starts with `chore`, `style`, `test`, `ci`, or `build`, unless the subject
carries `!` or the body carries `BREAKING CHANGE:`, which route to the Breaking group first. Mistyping a user-facing
change as `chore` silently strips it from release notes. Prefer `feat` / `fix` when the change has any user-observable
effect (config defaults, env vars, default behaviors, new shortcut commands, output format changes, cache mode changes).

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
control-char interactions.

### Why release-PR bodies repeat changelog entries from upstream PRs

The release PR carries the same changelog bullets as the feature PRs it ships. The repetition is intentional and
harmless: the release PR merges into `main`, so `--from-dev-prs`, which reads `dev`'s history, never sees it, and
`cliff.toml` skips the `release:` squash commit on the git-cliff path, so it is never counted twice.

### Why internal-tooling commits don't appear in `## Changelog`

`chore(cliff): ...`, `chore(ci): ...`, and similar internal-tooling commits don't appear in the PR body's `##
Changelog`. They are not user-facing. They belong in commit history and in the Files Modified section of the PR body,
not in the source-of-truth release notes.

## Triple-diff verification

The overlay recipe screens the staged release tree twice before the commit (A: release→dev for paths outside the guarded
set and the version files, B: no guarded path in release→main) and enumerates what the release adds (D). The cherry-pick
exception runs three diffs (A: main→release, B: release→dev for paths outside the guarded set, C: dev→main) plus a
patch-id cherry check. This is belt-and-suspenders because missed cherry-picks have shipped to `main` on this and
sibling repos before, and the file-level diff in B alone doesn't catch the patch-id false-negative class.

B excludes only the guarded set, not all of `docs/`. `docs/CLI_DESIGN.md`, `docs/DEVELOPER.md`, and `docs/SECRETS.md`
ship to `main`, so a wholesale `docs/` exclusion hides a missed change there.

### Why the guarded set resolves from the workflow

`guard-main-docs` is what CI enforces on a PR to `main`: the reusable workflow's hardcoded base list plus this repo's
`extra_paths`. Every hand-kept copy of that union (runbook, checklist, preflight script) drifted from it, and a copy
that omits a guarded path reports a real leak as clean while CI turns red after the push.
`scripts/release/guarded-paths.sh` reads `extra_paths` out of the caller workflow and adds the base list, so registering
a path in the workflow is the only edit a new guarded path needs. The base list is the one copy that still needs a
manual edit when the reusable changes, because it lives in another repo. Entries are globs with one rule set shared by
the reusable and the script (`**/` any depth, `*` and `?` within a segment, trailing slash guards the subtree), so
`**/.agent/` guards that directory wherever it appears and the two never disagree about what is guarded.

### Why the release enumerates what it adds

The leak check screens the diff against the registered set, so it says nothing about a category nobody registered. A new
engineering directory or a stray note under `docs/` passes the local check and `guard-main-docs` alike. Step D lists
every `docs/` file and every markdown file the release adds to `main` outside the guarded set and puts them in front of
a human; each one needs a reason to ship, or it gets registered in `extra_paths` and dropped from the branch. Root-level
markdown is in scope because an agent-facing glossary at the repo root is exactly the kind of addition a `docs/`-only
listing misses.

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

`scripts/generate-changelog.py` (vendored from the `github-repo-setup` skill, with the repo-local `cliff.toml`) is the
only sanctioned way to update `CHANGELOG.md`. On an overlay-built release branch it runs as `--from-dev-prs`: the PRs
merged into `dev` since the previous release are the entries, and each PR's body supplies its `## Changelog → ###
Breaking changes / Added / Changed / Deprecated / Fixed / Documentation` subsections (with author and PR-link
attribution). On a cherry-picked branch it runs `git-cliff` first to prepend a versioned entry from the branch's
commits, then expands the same way.

If a PR's body offers no changelog section at all, its title becomes a bullet under the group `cliff.toml` gives the
same commit (`type!:` under Breaking changes, `feat` under Added, `fix` under Fixed, `docs` under Documentation,
anything else under Changed), except for `chore`, `ci`, `build`, `style`, and `test` PRs, which stay out unless they
carry a `## Changelog` of their own, and the `release:` PR, which is bookkeeping. A body that carries the `## Changelog`
heading and leaves it empty is the PR template's way of saying the PR ships nothing user-facing, and the fallback
respects that whatever the title: internal work also lands as `fix(ci)`, `fix(hooks)`, or `fix(release)`, which the skip
list does not cover. `preflight.sh changelog-sections` names every PR whose entry would fall back to its title. To fix a
wrong CHANGELOG entry, fix the input: edit the squash-merged PR body, then re-run the script. Do **not** edit
`CHANGELOG.md` directly.

The `[0.1.0]` and `[0.1.1]` sections are the one exception: their commits predate the PR-body flow and are not
reconstructible from history, so `cliff.toml` lists both tags under `ignore_tags` and the generator never touches those
sections. The generator prepends or rewrites only the section for the version being cut.

On a PR to `main`, the `ci / Changelog` required check fails when a published crate's manifest changed and the changelog
beside it did not (a member that keeps none is held to the root's); `publish = false` crates are exempt. `preflight.sh
mechanics` checks that the release changelog opens on the release version with no `[Unreleased]` placeholder. The
release workflow cuts the GitHub Release body from the section whose heading names the tag's version, and falls back to
generated notes, with a warning, when there is none.

### Why `cliff.toml` skips chore/style/test/ci/build

These commit types do not produce user-facing content, so neither generation path emits a bullet for one on the strength
of its subject alone. What differs is the handling when such a PR *does* carry `## Changelog` content, and the two paths
differ in a way that decides how much the title matters:

- **`--from-dev-prs`.** The generator enumerates merged PRs and reads each body directly. A body with `## Changelog`
  content is extracted whatever the title says, so a `test:` or `chore:` PR carrying real bullets still lands in the
  section. The skip list applies only to the no-body fallback, where the title would otherwise become a bullet on its
  own.
- **`git-cliff` (the cherry-pick path).** The commit parsers drop these types from the skeleton before any PR body is
  fetched, so the PR number never enters the section, the expansion pass never reaches it, and its bullets are silently
  lost.

So on a cherry-picked branch a mistyped subject loses content: cross-check the generated section against `gh pr view
<num> --json body`, correct the title (e.g. `chore` → `feat`), re-amend the cherry-pick subject, and re-run. On a
`--from-dev-prs` branch the title costs only the fallback bullet, so the check worth making there is narrower: a PR with
an *empty* `## Changelog` and a skipped type that nonetheless shipped something user-facing. Either way the fix is to
the input, never to `CHANGELOG.md`.

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

The backport is a PR opened by `scripts/sync-dev-after-release.sh`, never a merge of `main` into `dev` and never a
direct push. The two branches share no history, so a merge conflicts on every file both sides touched, and a direct push
to `dev` bypasses its required status checks. The script writes the released version into `Cargo.toml`, copies
`CHANGELOG.md` from `main`, refreshes the crate's `Cargo.lock` entry from the synced manifest, and opens the PR; the
postflight backport gate treats that merged PR as the durable signal that the backport ran. The lock is refreshed rather
than copied because `main`'s copy would revert every dependency update `dev` merged after the release.

A release branch also takes edits nobody predicts (a doc fix, a reverted payload, a deleted config). Each is made
against `main`'s base, so it reaches `dev` only through the backport; left behind, the next release's overlay restores
`dev`'s copy over it and silently undoes the edit. A fixed list of files misses these, so the script discovers every
path the two branches disagree about. The previous release tag bounds that discovery, because it is the last point the
branches agreed: a path `dev` has not touched since the tag is release-prep and is adopted, while a path both sides
moved is contested and is only reported, so widening the copy cannot revert `dev`'s unreleased work. The operator adopts
contested paths by name (`--only`) or all at once (`--include-contested`). Guarded paths (the engineering docs) never
enter discovery, so the backport cannot remove them.

### Rollback

Rollback happens at the surface users consume (crates.io, the GitHub Release, the Homebrew formula), not in git. Yanking
a version, re-pointing `releases/latest`, or reverting a formula bump is fast and reversible; rewriting `main` is
neither, and the release flow exists so that `main` only ever moves forward through a PR. After the rollback, the fix or
revert lands through `dev`, a release branch, and `main` like any other change, so the branch reconverges with what is
live. Recording the last-good identifiers before the release is what makes the rollback a single command under incident
pressure.

### Cross-compile target matrix

Five targets, no musl rows: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. bird is a CLI client over a hosted API (X / Twitter, via the `xurl`
subprocess transport); the binaries are consumed by interactive shell users on the three desktop platforms and on Linux
CI runners. The musl static-link rows (added in sibling repos for Alpine and other glibc-free hosts) carry an additional
cross-compile cost and no demand signal here yet. Add them when a real Alpine consumer surfaces.

The Windows row uses `x86_64-pc-windows-msvc` (MSVC ABI) rather than GNU because the subprocess transport spawns `xr`
(xurl-rs) via the platform-native exec path; MSVC is the path of least resistance on Windows. CI also runs a separate
`ci / Windows check` job on every PR (not just at release) so MSVC build failures surface immediately, not at tag time.

### Known CI transient: cargo-deny Docker Hub timeout

The `EmbarkStudios/cargo-deny-action` container image is pulled from Docker Hub at job start. Docker Hub timeouts
periodically cause this step to fail before any real work runs. Recovery: `gh run rerun <id> --failed`. Don't treat the
first red as a real advisory failure until the second attempt also fails.

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

For a `CHANGELOG.md` finding, fix the upstream PR body (which `generate-changelog.py` re-fetches every run) and
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
- [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md) (post-tag verification of the publish chain)
- [`README.md`](README.md) (install channels, CLI reference, X API integration)
- [`.github/pull_request_template.md`](.github/pull_request_template.md) (PR body structure with changelog sections)
