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

## Execution

A stack that changes a contract across repositories gets a plan before code; the rest is coded directly. The lanes run
in parallel. The xurl-rs team is cutting a release, so work that needs xurl-rs code (lane C's PRs, lane E) waits for it;
everything else starts now.

| Lane | Work                                                                                                  | Plan first?                                                                                         |
| ---- | ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| A    | Stack 1 (bird side); Stack 2; Stack 6 items 2 to 5                                                    | No: stacked PRs                                                                                     |
| B    | Stack 1 items 5-6: CI jobs and the changelog-bump check in `brettdavies/.github`                      | Short plan: jobs, inputs; xurl-rs adopts them after its release                                     |
| C    | Stack 3                                                                                               | Design now; its xdk-rs and xurl-rs PRs open after the xurl-rs team's release                        |
| D    | Stack 4; stack 5 items 1-2 (bird's own wire vocabulary and store, raw HTTP and block/unblock removal) | One plan now; renames of verbs `xr` will supply wait for lane C                                     |
| E    | Stack 5 item 3 (pagination)                                                                           | Blocked: xdk-rs has no list loop yet, only per-call `pagination_token`; joins lane C's xurl-rs work |
| F    | Stack 7                                                                                               | No for the bird-skill repo structure; its content follows the settled CLI surface                   |

bird ships once, after every stack lands, as the release the 2026-09-02 cutover plan targets (1.0.0). Its critical path
runs through xurl-rs: the current xurl-rs release, which bird adopts from crates.io (stack 6 item 1); stack 3's xdk-rs
request hook, xurl-rs command-layer surface, and the list loop; the xdk-rs release that carries them; then bird's
composition and the cut.

## Landed

| PR   | Change                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------------ |
| #123 | rustls, rustls-webpki, webpki-roots advisories cleared                                                 |
| #124 | `release-matrix-check.yml`: the seven release targets build on dependency and toolchain changes        |
| #125 | xdk-rs replaces the xurl-rs 2.x library; xdk's error reason and exit code; store-only credentials work |
| #126 | `bird doctor` reports whether the embedded client was built; subprocess-era residue removed            |
| #127 | `error-envelope` schema admits exit codes 3, 4, 5; envelope docs match the emitter                     |
| #128 | login, doctor, and the runner take the token store, xdk environment, and stdin as injected inputs      |
| #120 | `docs/SECRETS.md` names the release token by UUID                                                      |
| #121 | Dependabot: `dtolnay/rust-toolchain` bump                                                              |
| #122 | Dependabot: cargo minor/patch group                                                                    |

## Stack 1: repo environment

Lands before any product work: every later PR runs under these hooks and checks.

1. **Changelog categories upstream.** `Removed` sits between Deprecated and Fixed in `generate-changelog.py`'s
   `CATEGORIES`, with a section-order test (brettdavies/agent-skills `b4318aa`). The canonical PR template gains the
   `Breaking changes`, `Deprecated`, and `Removed` headings (brettdavies/dotfiles#288). Then re-vendor the PR template,
   `generate-changelog.py`, `cliff.toml`, and the release-doc templates into bird and xurl-rs.
2. **Canonical drift.** Re-vendor `.github/pull_request_template.md` and `cliff.toml` (drifted), and vendor
   `scripts/check-workflow-concurrency.sh`, `scripts/hooks/core-env-guard.sh`, `scripts/hooks/_lib.sh`,
   `scripts/release/release.env`, and `scripts/release/surface-smoke.sh`.
3. **Hooks.** Adopt the template `pre-commit` (staged-file fmt, clippy, markdownlint, shellcheck) and the template
   `pre-push` (push-range scoping from stdin, bash re-exec, git-env isolation). Decide whether bird's R20
   writer-injection step stays a local extension or becomes the shared output-discipline check (item 5). Add the
   `shfmt -i 2 -ci -bn` gate for shell scripts and a test that asserts the hook and the CI workflow run the same gates.
4. **Completions.** Regenerate `completions/` (every file is stale against the binary) in the same PR that adds a
   freshness check, so CI holds it.
5. **CI jobs bird lacks, via shared workflows.** xurl-rs runs these in-repo: change scope (`cargo-inputs.sh`), workflow
   concurrency check, rustdoc build with `-D warnings`, completions freshness, MSRV, output discipline (`lint-stdio`),
   and the anc audit. For each: use the shared `rust-ci.yml` input if one exists, otherwise add the job to
   `brettdavies/.github` so xurl-rs can drop its copy too. Also adopt the `pull_request`-only trigger with
   `concurrency: cancel-in-progress`, and pass `advisories_blocking: true`: without it the shared workflow's advisories
   row soft-fails and a run with a live advisory still passes.
6. **Changelog bump check.** xurl-rs's `changelog-bump.yml` fails a PR whose CLI surface grows while its `## Changelog`
   names no minor-or-higher section (`scripts/check-surface-bump.sh`). Adopt it through a shared workflow, with a bird
   surface comparison.
7. **`deny.toml`.** xurl-rs's shape: the seven release triples as `targets`, `all-features = true`, and an explicit
   `skip` list with reasons.
8. **Rulesets (live settings).** Protect-dev requires status checks as xurl-rs's does; protect-main requires
   `ci / Security audit (advisories)`; the committed `protect-main.json` matches the live
   `require_extra_approval_for_unattributed_changes: true`.
9. **Repo files.** `CLAUDE.md` loader (`@AGENTS.md`), `CONTRIBUTING.md`, `SECURITY.md`, `CONCEPTS.md` (registered in
   `guard-main-docs` `extra_paths`), `.anc.toml` with `[p6] domain_verbs`, the issue forms (`bug-report.yml`,
   `feature-request.yml`, `skill-bundle.yml`), and the `oauth` repository topic.
10. **Small parity items.** `build-examples.yml` pins `dtolnay/rust-toolchain` to `master` where xurl-rs pins `stable`;
    `Cargo.toml` lacks the template's `[package.metadata.binstall.overrides.x86_64-pc-windows-msvc]` (check it against
    the Windows archive format); xurl-rs's `tests/env_mutation_guard.rs` guards against tests that set environment
    variables.

## Stack 2: agent-native compliance

From `anc audit` 0.6.0: 63 pass, 2 warn, 2 fail, badge eligible.

1. `code-unwrap` (fail): the `stderr.lock().unwrap()` sites in `src/db/client/`, `src/db/store/mod.rs`, and the runner.
2. `p6-must-sigterm` (fail): a SIGTERM handler for the long-running paths, cancelling the xdk call in flight.
3. `p4-should-structured-enum` (warn): the auditor looks for `src/error.rs`; bird has `src/error/mod.rs`. Fix in anc or
   record an opt-out.
4. `p6-may-standard-names` (warn): closed by `.anc.toml` domain verbs (stack 1.9) or by verb parity (stack 5).
5. `skill_install` reads `$HOME` from the process (`src/skill_install/mod.rs`); route it through the runner's injected
   environment, as `xr` does with xdk's `EnvOverrides::home`, so tests never install into the real home directory.

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
4. bird: compose `xr`'s tree; retire each bird verb `xr` already has. `xr usage credits` arrives this way.

A composed `xr` command arrives with `xr`'s verbs, flags, and envelope, so this stack decides which verb renames in
stack 5 remain. Stack 4 and the rest of stack 5 do not wait for it.

## Stack 4: output contract parity with xr

bird's own commands (cache, watchlist, thread, schema, local usage, doctor, skill) print `xr`'s envelope too, so output
matches whether a command is composed from `xr` or bird's own.

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

## Stack 6: release

v0.2.0 is the last release; `dev` carries 228 commits past it, among them the embedded-transport cutover and breaking
exit-code and error-id changes. `scripts/release/preflight.sh` gates the cut; on `dev` today `drift` fails, and
`mechanics` fails only its leak check, which the release branch's overlay resolves.

1. **xdk-rs from crates.io.** bird pins a xurl-rs `dev` commit for xdk-rs (serde-saphyr, xurl-rs#348) under `version =
   "0.2.0"`, but the release workflow's `cargo publish -p bird` resolves the crates.io `xdk-rs 0.2.0`, which still links
   `serde_yaml` and lacks `xdk::store::locate_store`, so `cargo install bird` would carry RUSTSEC-2024-0449. When the
   xurl-rs team's release publishes both, move bird onto it from crates.io and collapse `ResolvedPaths::token_store`
   from `Option` to xdk's resolved default path; stack 3's hook then needs one more xdk-rs release before bird's cut.
   Decide whether bird adopts the directory store layout as `xr` does (`adopt_directory_layout`) or keeps opening a
   single-file `~/.xurl` in place.
2. **Backport `main` to `dev`.** `main` carries `c2f7789` (`.github/workflows/dependabot-preflight.yml`) and 16 newer
   `Cargo.lock` entries `dev` never received (`preflight.sh drift`).
3. **Release caller.** `linux_musl_required: true` (both musl rows build clean on #124 and #125), and `attest: true`
   with `attestations: write`.
4. **Homebrew formula.** Convert bird's formula to install the release's prebuilt archives, as xurl-rs's does: the tap
   verifies each archive against its attestation before pinning its checksum (needs item 3), and the formula no longer
   builds `aws-lc-sys` from source inside Homebrew. The tap's open prebuilt-archive PRs (brettdavies/homebrew-tap#153,
   #157, #158) cover that path. The `xdevplatform/tap/xurl` caveat, which the tap's update workflow never rewrites, is
   replaced by brettdavies/homebrew-tap#123, which reaches the tap's `main` with bird's version bump.
5. **Runbooks.** Compare `RELEASES.md`, `RELEASES-PREFLIGHT.md`, `RELEASES-POSTFLIGHT.md`, and `RELEASES-RATIONALE.md`
   against the template and xurl-rs; confirm the `docs/designs/` documents `mechanics` lists as unguarded are meant to
   ship to `main`.
6. **Version and cut** (`RELEASES.md`): one release after every stack, as 1.0.0, then the live `preflight.sh smoke`, the
   tag, and `postflight.sh`.

## Stack 7: external surfaces

1. bird-skill: replace the subprocess transport, `BIRD_XURL_PATH`, and `xurl.available` guidance, and re-validate it
   against `schema/doctor.schema.json`. The repo also lacks what xurl-rs-skill carries: `VERSION`, `bin/check-update`,
   `AGENTS.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `cliff.toml`, `getting-started.md`, `tests/run.sh`,
   shellcheck in CI, the evals, and `templates/tests/`.
2. agentnative-site: rescore bird and add the README badge.

## Local housekeeping

- This clone's `core.hooksPath` is `.git/hooks`, which holds no hooks; `AGENTS.md` documents `scripts/hooks`.
