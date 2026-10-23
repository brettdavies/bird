# Contributing to bird

bird is an X API command-line client with a local entity store, search and thread tools, and watchlists, built on the
[`xdk-rs`](https://github.com/brettdavies/xurl-rs) library. Bug reports, feature requests, and code are welcome. A solo
maintainer cannot promise merge windows; real PRs land. bird owns its commands, its cache, and its output; HTTP, OAuth,
and token storage live in xdk-rs, so a transport or auth problem usually belongs on the
[xurl-rs tracker](https://github.com/brettdavies/xurl-rs/issues).

## Dev setup

```bash
git clone https://github.com/brettdavies/bird && cd bird
cargo build                                # binary at ./target/debug/bird
cargo test                                 # unit + integration
git config core.hooksPath scripts/hooks    # activate the hook pair
```

That one command activates both hooks, which share `scripts/hooks/_lib.sh`. `pre-commit` is staged-file-scoped and fast:
rustfmt on staged `.rs`, shellcheck and `shfmt -i 2 -ci -bn` on staged shell, actionlint and the concurrency-group check
on a staged workflow, and the core-env guard on a staged test script.

`pre-push` mirrors CI: `cargo fmt`, `cargo clippy --all-targets` with warnings denied, `cargo test`, the MSRV check, the
doc build with `-D warnings`, `cargo deny check`, the output-discipline check (`scripts/lint-stdio.sh`), a Windows
compatibility scan, and the Windows cross-target clippy, then shellcheck, shfmt, actionlint, and the guards over every
tracked file. It runs the Rust battery only when the push reaches a cargo input, so a markdown-only push finishes in
seconds. The scoping fails open: an unrecognized path runs everything, and running the hook by hand sweeps the whole
repo. A step whose tool is missing says so on its own line, so a skip never reads as a pass. `tests/hook_ci_parity.rs`
names the CI job behind every step.

The package check (`cargo publish --dry-run`) and the markdownlint job run in CI only. The live-API checks stay manual
by design; [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) lists them, and every live call is billed per object, so
reach for `--cache-only` or a single-object read before a list command.

## Branch and PR flow

Branch from `dev` as `feat/<slug>` or `fix/<slug>`, then PR back to `dev`. `main` is what consumers install and receives
code only through a release PR. [`RELEASES.md`](RELEASES.md) is the full runbook.

- **Commits and PR titles** follow [Conventional Commits](https://www.conventionalcommits.org/). The PR title becomes
  the squash-merge subject and lands in `CHANGELOG.md` at release time, so write it as the history entry you want.
- **PR bodies** fill [`.github/pull_request_template.md`](.github/pull_request_template.md). Its `## Changelog` section
  is the source of truth for `CHANGELOG.md`, which is generated and never hand-edited.
- **Engineering docs** (`docs/plans/`, `docs/brainstorms/`, `docs/research/`, `docs/reviews/`, `CONCEPTS.md`) commit
  straight to `dev` and are blocked from `main`. Everything a consumer reads, markdown included, goes through the branch
  and PR flow.
- **Quality bar, testing, and architecture** live in [`AGENTS.md`](AGENTS.md). Tests run against TempDir-backed paths
  and the `MockXurlClient` double, never the real home directory or the live API.

## Error contract

Text output is written for humans and the structured formats for agents, and the two need not match word for word: text
carries prose and a help pointer, structured output carries stable fields to branch on. Every structured error carries a
kebab-case `error` id from a closed set, an `exit_code`, and a human `message`; the ids and codes xdk-rs classifies come
from `xdk::Error` unchanged. Prefer additive envelope changes: add keys rather than renaming or retyping existing ones,
and update the matching document under `schema/` in the same PR, which `tests/schema_parity.rs` enforces.

## Filing issues

Use a [bug report](https://github.com/brettdavies/bird/issues/new?template=bug-report.yml), a
[feature request](https://github.com/brettdavies/bird/issues/new?template=feature-request.yml), or a
[skill-bundle report](https://github.com/brettdavies/bird/issues/new?template=skill-bundle.yml). Auth flows, token
storage, and HTTP behavior belong on the [xurl-rs tracker](https://github.com/brettdavies/xurl-rs/issues), and X API
platform questions (rate limits, enrollment, pricing) in the [X developer community](https://devcommunity.x.com/).
