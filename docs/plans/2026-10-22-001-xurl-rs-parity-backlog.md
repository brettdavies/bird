---
title: xurl-rs parity backlog, landed in stacks
date: 2026-10-22
status: active
owner: brettdavies
---

# xurl-rs parity backlog

## Rules

- **xdk-rs and xurl-rs are correct when bird differs.** Where bird and its upstream disagree on behavior, vocabulary,
  output shape, or repo tooling, bird moves to the upstream form.
- **xurl-rs stays fully usable on its own.** Anything added upstream for bird is additive; `xr` never depends on bird.
- **Shared workflows first.** CI and release logic lives in the `brettdavies/.github` reusable workflows. bird keeps
  thin callers; a capability bird lacks is consumed from, or added to, the shared workflow, never copied from xurl-rs's
  in-repo `ci.yml`.
- **Canonical templates first.** Files `github-repo-setup/templates/canonical-sources.json` names are fixed at their
  source (agent-skills or dotfiles) and re-vendored, never edited in bird alone.
  `github-repo-setup/scripts/check-canonical-drift.sh` reports the drift.
- **One stack per row group, one concern per PR.** Each stack lands with `gh stack merge` once every head is green.

## Landed

| PR   | Change                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------------ |
| #123 | rustls, rustls-webpki, webpki-roots advisories cleared                                                 |
| #124 | `release-matrix-check.yml`: the seven release targets build on dependency and toolchain changes        |
| #125 | xdk-rs replaces the xurl-rs 2.x library; xdk's error reason and exit code; store-only credentials work |
| #120 | `docs/SECRETS.md` names the release token by UUID                                                      |
| #121 | Dependabot: `dtolnay/rust-toolchain` bump                                                              |
| #122 | Dependabot: cargo minor/patch group                                                                    |

## Stack 0: in flight

| Item | Change                                                                                                                       |
| ---- | ---------------------------------------------------------------------------------------------------------------------------- |
| #126 | `bird doctor` reports whether the embedded client was built; `xurl.path` removed; subprocess-era prose and help text removed |
| B3   | `error-envelope` schema admits exit codes 3, 4, 5; README and AGENTS.md envelope docs match the emitter                      |

## Stack 1: repo environment

Lands before any product work: every later PR runs under these hooks and checks.

1. **Changelog categories upstream.** Add `### Removed` (and the `Breaking changes` / `Deprecated` headings the
   generator already reads) to the canonical PR template
   (`dotfiles/stow/github/dot-config/github/pull_request_template.md`) and `Removed` to `CATEGORIES` in
   `agent-skills/github-repo-setup/templates/generate-changelog.py`, with its section-order tests. Then re-vendor both
   into bird and xurl-rs.
2. **Canonical drift.** Re-vendor `.github/pull_request_template.md` and `cliff.toml` (drifted), and vendor
   `scripts/check-workflow-concurrency.sh`, `scripts/hooks/core-env-guard.sh`, `scripts/hooks/_lib.sh`, and
   `scripts/release/release.env`.
3. **Hooks.** Adopt the template `pre-commit` (staged-file fmt, clippy, markdownlint, shellcheck) and the template
   `pre-push` (push-range scoping from stdin, bash re-exec, git-env isolation). Decide whether bird's R20
   writer-injection step stays a local extension or becomes the shared output-discipline check (item 5).
4. **Completions.** Regenerate `completions/` (every file is stale against the binary) in the same PR that adds a
   freshness check, so CI holds it.
5. **CI jobs bird lacks, via shared workflows.** xurl-rs runs these in-repo: change scope (`cargo-inputs.sh`), workflow
   concurrency check, rustdoc build with `-D warnings`, completions freshness, MSRV, output discipline (`lint-stdio`),
   and the anc audit. For each: use the shared `rust-ci.yml` input if one exists, otherwise add the job to
   `brettdavies/.github` so xurl-rs can drop its copy too. Also adopt the `pull_request`-only trigger with
   `concurrency: cancel-in-progress`.
6. **`deny.toml`.** xurl-rs's shape: the seven release triples as `targets`, `all-features = true`, and an explicit
   `skip` list with reasons.
7. **Release caller.** `linux_musl_required: true` (both musl rows build clean on #124 and #125) and `attest: true` with
   `attestations: write`; the Homebrew tap verifies attestations before pinning, so this pairs with a tap change.
8. **Rulesets (live settings).** Protect-dev requires status checks as xurl-rs's does; protect-main requires
   `ci / Security audit (advisories)`; the committed `protect-main.json` matches the live
   `require_extra_approval_for_unattributed_changes: true`.
9. **Repo files.** `CLAUDE.md` loader (`@AGENTS.md`), `CONTRIBUTING.md`, `SECURITY.md`, `CONCEPTS.md` (registered in
   `guard-main-docs` `extra_paths`), `.anc.toml` with `[p6] domain_verbs`, and the issue forms (`bug-report.yml`,
   `feature-request.yml`, `skill-bundle.yml`).

## Stack 2: agent-native compliance

From `anc audit` 0.6.0: 63 pass, 2 warn, 2 fail, badge eligible.

1. `code-unwrap` (fail): the `stderr.lock().unwrap()` sites in `src/db/client/`, `src/db/store/mod.rs`, and the runner.
2. `p6-must-sigterm` (fail): a SIGTERM handler for the long-running paths, cancelling the xdk call in flight.
3. `p4-should-structured-enum` (warn): the auditor looks for `src/error.rs`; bird has `src/error/mod.rs`. Fix in anc or
   record an opt-out.
4. `p6-may-standard-names` (warn): closed by `.anc.toml` domain verbs (stack 1.9) or by verb parity (stack 4).

## Stack 3: bird extends xr in-process (design first)

bird is the extended client (cache, knowledge-base and database integration, PAYG cost reduction); `xr` is the limited,
X-approvable CLI. bird composes `xr`'s commands instead of re-implementing them.

1. **Design doc across xdk-rs, xurl-rs, and bird.** A request-layer hook in xdk-rs so an embedder can answer a request
   from its store and record what X returned; a supported library surface in xurl-rs for `xr`'s command tree (the clap
   tree plus a `run` that takes an injected `xdk::Client` and writers); bird composing that tree with its own commands
   (cache, watchlist, schema, local usage) and installing its cache hook on the client it passes in. Revises the seam
   doc's "flag parsing is always presentation" rule.
2. xdk-rs: the request hook.
3. xurl-rs: the public command-layer surface, `xr` unchanged on its own.
4. bird: compose `xr`'s tree; retire each bird verb `xr` already has.

This stack decides how much of stacks 4 and 5 remains: a composed `xr` command arrives with `xr`'s verbs, flags, and
envelope. Write the design before starting either.

## Stack 4: output contract parity with xr

What remains after stack 3 for bird's own commands.

1. Error envelope in `xr`'s shape:
   `{"status":"error","reason","exit_code","message","next_step":{"action","command", "docs"}}`, filling `next_step`
   from `xdk::Error::next_action` and `docs_url`.
2. Success and dry-run envelopes in `xr`'s shape (`"status":"ok"` / `"status":"dry_run"`), with every schema under
   `schema/` and `bird schema` updated.
3. Login envelopes (`awaiting_callback`) in `xr`'s shape.

## Stack 5: post vocabulary, verbs, pagination

`docs/plans/2026-09-02-2314-feat-xurl-v3-post-vocabulary-cutover-plan.md`, refreshed for xdk-rs 0.2+ (async client,
`Call<T>` builders) in place of xurl-rs 3.0.0.

1. Wire vocabulary: send `post.fields`, read `referenced_posts`, `repost_count`, `includes.posts`; purge pre-cutover
   cache rows once.
2. Verbs under `xr`'s names; raw HTTP verbs and `block` / `unblock` removed.
3. The pagination and cost contract on the crate's list loop.

## Stack 6: release and external surfaces

1. Version and cut (`RELEASES.md`); the release carries every breaking change above.
2. Homebrew tap: drop the `xdevplatform/tap/xurl` caveat from the bird formula.
3. bird-skill: replace the subprocess transport, `BIRD_XURL_PATH`, and `xurl.available` guidance.
4. agentnative-site: rescore bird and add the README badge.

## Local housekeeping

- This clone's `core.hooksPath` is `.git/hooks`, which holds no hooks; `AGENTS.md` documents `scripts/hooks`.
- Worktrees `bird-release-token-ref` (`docs/release-token-uuid-ref`, merged as #120) and `bird-rustls`
  (`fix/rustls-advisories`, superseded by #123) can be removed.
- `docs/research/2026-09-02-catch-up-triage.md` is untracked; it is the source of several rows above.
