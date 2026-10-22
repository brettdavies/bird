---
title: Shared CI jobs for bird and xurl-rs - Plan
type: ci
date: 2026-10-22
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# Shared CI jobs for bird and xurl-rs - Plan

## Goal Capsule

- **Objective:** a bird pull request is held to the same checks as a xurl-rs pull request (doc build, MSRV, completions
  freshness, output discipline, workflow concurrency, agent-native audit, documentation-only scoping, and the changelog
  bump rule), and every one of those checks has one definition that both repositories call.
- **Means:** opt-in inputs on `rust-ci.yml` and `lint-basics.yml` and two new reusable workflows in
  `brettdavies/.github`, each running a script from the consumer's checkout (KTD1, KTD2).
- **Authority:** the parity backlog's Rules (`docs/plans/2026-10-22-001-xurl-rs-parity-backlog.md`), then this plan,
  then the code.
- **Stop conditions:** merging a `brettdavies/.github` PR waits for the user's approval, because it changes shared CI
  inputs. bird's side writes no code in xurl-rs: its adoption of the shared jobs is a request to the xurl-rs team (U10).
- **Execution profile:** the `.github` PRs and bird's consumer PRs are opened together; bird's PRs stay drafts that call
  the shared workflow at the `.github` branch ref until that PR merges, then move to `@main`.

---

## Product Contract

### Summary

Seven jobs and one PR-body check that xurl-rs runs in its own `ci.yml` and `changelog-bump.yml` move into
`brettdavies/.github`. bird turns them on through its thin callers, and the xurl-rs team is asked to swap its in-repo
copies for the same callers. `lint-basics.yml` gains a shellcheck job for the repositories with shell scripts and no
Rust.

### Problem Frame

bird's CI is the bare `rust-ci.yml` call. It builds no rustdoc with warnings denied, checks no MSRV, never notices that
every file in `completions/` is stale against the binary, runs its writer-injection guard as a one-off workflow, and has
no anc gate, no change scoping, and no rule tying a CLI surface change to a minor changelog section. xurl-rs has all of
these, but as in-repo jobs, so copying them into bird would leave two definitions to drift.

### Requirements

**Shared jobs**

- R1. `rust-ci.yml` offers a doc build, an MSRV check, a completions freshness check, an output-discipline check, a
  workflow-concurrency check, and an anc audit, each off unless its caller turns it on.
- R2. A consumer that sets none of the new inputs keeps every job it runs today, with unchanged names and verdicts.
- R3. A documentation-only change, as the consumer's own classifier decides it, skips every Rust job and still satisfies
  required checks.
- R4. A pull request whose CLI or schema surface grows fails unless its `## Changelog` block has a bullet under
  `### Added`, `### Deprecated`, or `### Breaking changes`.
- R9. `lint-basics.yml` offers a shellcheck job, off unless its caller turns it on, for repositories `rust-ci.yml` never
  reaches (bird-skill and xurl-rs-skill each run an in-repo copy).

**bird**

- R5. bird's CI runs every check in R1, R3, and R4; `completions/` is regenerated in the change that turns the freshness
  check on.
- R6. bird's writer-injection rule (no output macro outside `src/main.rs` and `src/error/fatal.rs`) is enforced by the
  shared output-discipline job, and the separate `guard-r20.yml` workflow is removed.
- R7. bird's anc gate fails on any `fail` or `error` row, not only MUST-tier ones.

**xurl-rs**

- R8. The xurl-rs team receives a request, with the exact caller inputs, to call the shared jobs in place of its in-repo
  copies, keeping its repo-specific jobs (`semver`, `consumer`, `features`, `parity`, `feature-powerset`) in-repo.

### Scope Boundaries

- Outside this plan: the template hooks themselves and the ruleset JSON (lane A1 landed both in stack 1), and the stack
  2 work that clears bird's two anc failures. This plan changes the hook only where it mirrors a moved check.

#### Deferred to Follow-Up Work

- Making `cargo-inputs.sh` and `check-surface-bump.sh` canonical templates in `github-repo-setup`, once both
  repositories run per-repo copies that agree on an interface.
- bird-skill and xurl-rs-skill calling U12's shellcheck job in place of their in-repo copies.

---

## Planning Contract

### Key Technical Decisions

- KTD1. **Opt-in inputs on `rust-ci.yml`, not one reusable workflow per job.** One caller block keeps every check under
  the `ci /` prefix and lets every new job honor `docs_only` the way the existing jobs do. The cost is that a consumer
  which opts into nothing sees the new jobs listed as skipped; a skipped job counts as passing and is required nowhere,
  so R2 holds.
- KTD2. **Scripts run from the consumer's checkout.** The shared job owns the runner, toolchain, and gate; the consumer
  owns the script at a conventional path (`scripts/lint-stdio.sh`, `scripts/generate-completions.sh`,
  `scripts/check-workflow-concurrency.sh`, `scripts/cargo-inputs.sh`, `scripts/check-surface-bump.sh`). This is the
  pattern `guard-main-docs.yml` and its vendored `scripts/release/guarded-paths.sh` already use. The rules the
  output-discipline and surface checks enforce are per-repository (file paths, binary name, changelog heading), so they
  cannot live in the shared workflow.
- KTD3. **Change scope is its own reusable workflow, `rust-change-scope.yml`, feeding the existing `docs_only` input.**
  Putting a scope job inside `rust-ci.yml` would add a `needs:` edge to every existing job of every consumer. A separate
  workflow is the shape xurl-rs already runs in-repo, and `docs_only` was written for exactly this caller. The scope job
  fails open: on a non-PR event, or when the classifier fails, it reports `rust=true` or no output, and the caller's
  `docs_only: ${{ needs.scope.outputs.rust == 'false' }}` runs the full battery.
- KTD4. **The changelog bump check is its own reusable workflow, `rust-changelog-bump.yml`.** Its verdict depends on the
  PR body, so it needs the `edited` trigger, which a CI run does not have. The caller owns the triggers (`opened`,
  `edited`, `synchronize`, `reopened`; `branches-ignore: [main]`) and the concurrency group.
- KTD5. **bird's R20 guard becomes bird's `scripts/lint-stdio.sh`.** The rule is unchanged. The script passes `src` to
  `rg` explicitly, because with no path and a non-terminal stdin `rg` searches stdin: inside a pre-push hook, whose
  stdin is git's ref pipe, a pathless call reads the pipe and passes without looking at `src/`.
- KTD8. **The hook and CI share one classifier and one mirror table.** The pre-push hook scopes its Rust battery with
  `scripts/cargo-inputs.sh`, the script `rust-change-scope.yml` runs, so a push and its PR skip on the same changes.
  `tests/hook_ci_parity.rs` maps the msrv, doc, output-discipline, and concurrency steps to their opt-in `rust-ci.yml`
  jobs, fails when `ci.yml` does not set the input, and fails when the hook scopes with anything else.
- KTD6. **The anc job audits the source tree and the release binary together** (`anc audit . --bin <bin>` after
  `cargo build --release --locked --bin <bin>`), so source rows such as `code-unwrap` count. Inputs: `anc_version`
  (empty means off; the pin makes an anc release reach a consumer only through a PR that bumps it), `anc_bin`,
  `anc_score_floor`, `anc_blocking_tiers` (`must` by default, `any` for bird per R7), and `release_size_ceiling_bytes`
  (0 means off).
- KTD7. **The doc build carries an optional docs.rs pass** (`docs_rs_package`): README doctests and a nightly
  `--cfg docsrs` build of one library package, so xurl-rs can drop its `docs` job without losing either.

### High-Level Technical Design

How a bird pull request reaches each check once U5 to U9 land:

```mermaid
flowchart TB
  PR[bird pull request] --> CI[ci.yml]
  PR --> CB[changelog-bump.yml]
  CI --> SC[scope: rust-change-scope.yml: scripts/cargo-inputs.sh]
  SC -->|docs_only| RC[ci: rust-ci.yml]
  RC --> J1[Fmt, clippy, test / Windows / audit / package / shellcheck]
  RC --> J2[Doc build / MSRV / Completions freshness]
  RC --> J3[Output discipline: scripts/lint-stdio.sh: Workflow concurrency: scripts/check-workflow-concurrency.sh]
  RC --> J4[Agent-native audit: anc pinned]
  CB --> RB[rust-changelog-bump.yml: scripts/check-surface-bump.sh]
```

### Inputs added to the shared workflows

| Workflow                  | Input                                                                                           | Type, default                                                       | Job name it controls                 |
| ------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------ |
| `rust-ci.yml`             | `docs`                                                                                          | bool, false                                                         | Doc build                            |
| `rust-ci.yml`             | `docs_rs_package`                                                                               | string, empty                                                       | Doc build (docs.rs steps)            |
| `rust-ci.yml`             | `msrv`                                                                                          | bool, false                                                         | MSRV                                 |
| `rust-ci.yml`             | `completions`                                                                                   | bool, false                                                         | Completions freshness                |
| `rust-ci.yml`             | `lint_stdio`                                                                                    | bool, false                                                         | Output discipline                    |
| `rust-ci.yml`             | `workflow_concurrency`                                                                          | bool, false                                                         | Workflow concurrency                 |
| `rust-ci.yml`             | `anc_version`, `anc_bin`, `anc_score_floor`, `anc_blocking_tiers`, `release_size_ceiling_bytes` | string empty, string empty, number 0, string `must`, number 0       | Agent-native audit                   |
| `rust-change-scope.yml`   | `script`                                                                                        | string, `scripts/cargo-inputs.sh`                                   | Change scope (output `rust`)         |
| `rust-changelog-bump.yml` | `script`, `surface_paths`                                                                       | string `scripts/check-surface-bump.sh`, string `completions schema` | Surface growth needs a minor section |
| `lint-basics.yml`         | `shellcheck`, `shellcheck-files`, `shellcheck-severity`                                         | bool false, string `*.sh scripts/hooks/*`, string `warning`         | shellcheck                           |

### Assumptions

- The `ubuntu-22.04` runner's `jq` and Python 3.10 are enough for `generate-completions.sh` and `check-surface-bump.sh`;
  xurl-rs's script already carries the `tomllib` shim for 3.10.
- bird's MSRV (`rust-version = "1.94"`) compiles the tree with `--all-features --locked`; the pre-push hook runs the
  same check when the toolchain is installed.
- No Rust source in bird names a markdown file in a string literal today, so every markdown path except `README.md` is
  documentation only for bird's classifier.

### Risks & Dependencies

- bird's anc gate is red until stack 2 clears `code-unwrap` and `p6-must-sigterm`, so U9 lands after stack 2.
- Turning on the shared jobs in xurl-rs renames its required checks (`Doc build` becomes `ci / Doc build`). Its rulesets
  must change in the same window, which is a live-settings change for the user to approve.
- Requiring U11's contexts before U5 to U8 merge would block every bird merge on checks that never report.

---

## Implementation Units

### U1. Opt-in doc, MSRV, completions, output-discipline, and concurrency jobs in `rust-ci.yml`

- **Goal:** R1 (all but anc), R2, R3.
- **Files:** `.github/workflows/rust-ci.yml`, `README.md` (in `brettdavies/.github`).
- **Approach:** five jobs, each `if: inputs.<flag> && !inputs.docs_only`, with the toolchain, cache, and pins the
  existing jobs use. The MSRV job reads `rust-version` from the root `Cargo.toml` and checks
  `--workspace --all-features --locked` on it. KTD1, KTD2, KTD7.
- **Test scenarios:** a bird draft PR with all five on runs them green; the same PR with one stale completions file
  fails `Completions freshness`; a `println!` added under `src/` fails `Output discipline`; a consumer passing no new
  input shows the five as skipped and its existing jobs unchanged.
- **Verification:** actionlint and `scripts/check-workflow-concurrency.sh` pass in `.github`; the draft PR's runs are
  linked from the `.github` PR.

### U2. Agent-native audit job in `rust-ci.yml`

- **Goal:** R1 (anc), R7.
- **Dependencies:** U1 (same file; stacked).
- **Files:** `.github/workflows/rust-ci.yml`, `README.md`.
- **Approach:** KTD6. The scorecard and the sorted row list upload as an artifact on every run.
- **Test scenarios:** bird with `anc_blocking_tiers: any` fails today, naming `code-unwrap` and `p6-must-sigterm`; with
  `must` it fails on `p6-must-sigterm` only; a floor above the score fails with the score in the message.
- **Verification:** the failing runs above, linked from the PR; a green run follows once stack 2 lands.

### U3. `rust-change-scope.yml`

- **Goal:** R3.
- **Files:** `.github/workflows/rust-change-scope.yml`, `README.md`.
- **Approach:** KTD3. On `pull_request` it pipes `git diff --no-renames --name-only HEAD^1 HEAD` from a depth-2 checkout
  into the caller's script; on any other event it answers `rust=true`.
- **Test scenarios:** a bird PR touching only `docs/` reports `rust=false` and every Rust job skips; one touching
  `README.md` (the manifest's `readme`) or any `.rs` file reports `rust=true`.
- **Verification:** both runs linked from the PR.

### U4. `rust-changelog-bump.yml`

- **Goal:** R4.
- **Files:** `.github/workflows/rust-changelog-bump.yml`, `README.md`.
- **Approach:** KTD4. Extract `surface_paths` from `HEAD^1` into `$RUNNER_TEMP/base`, write the PR body from the
  environment to a file, run `bash <script> check <base> . <body>`.
- **Test scenarios:** bird PR adding a flag with only `### Changed` fails; the same PR edited to add `### Added` passes
  on the `edited` event without a push; a PR that changes no surface passes.
- **Verification:** the three runs linked from the PR.

### U5. bird: doc, MSRV, concurrency, and output-discipline jobs on

- **Goal:** R5, R6.
- **Dependencies:** U1.
- **Files:** `.github/workflows/ci.yml`, `scripts/lint-stdio.sh` (new), `scripts/check-workflow-concurrency.sh`
  (vendored verbatim from `github-repo-setup`), `.github/workflows/guard-r20.yml` (removed).
- **Approach:** KTD5, KTD8. The pre-push hook's `output discipline` step calls `scripts/lint-stdio.sh`, and
  `tests/hook_ci_parity.rs` maps the hook's msrv, doc, output-discipline, and concurrency steps to their opt-in jobs.
- **Test scenarios:** the script passes on `dev`; an added `eprintln!` in `src/cli/` fails with its file and line;
  `src/error/fatal.rs` stays exempt.
- **Verification:** the draft PR's runs.

### U6. bird: completions regenerated and held fresh

- **Goal:** R5.
- **Dependencies:** U5 (stacked).
- **Files:** `completions/bird.{bash,zsh,fish,elvish,powershell}`, `.github/workflows/ci.yml`.
- **Approach:** `scripts/generate-completions.sh`, then `completions: true` in the same change.
- **Test scenarios:** `scripts/generate-completions.sh --check` exits 0 after regeneration and 1 before it.
- **Verification:** `Completions freshness` green on the PR.

### U7. bird: documentation-only scoping

- **Goal:** R3, R5.
- **Dependencies:** U3, U6 (stacked).
- **Files:** `scripts/cargo-inputs.sh` (new), `.github/workflows/ci.yml`.
- **Approach:** a markdown path is documentation only unless a tracked `.rs` file names it in a string literal or it is
  a README, which cargo packages; `.context/`, the license texts, and the `.github` files no test reads are
  documentation too, and every other path is a cargo input. The `ci` job takes `needs: scope`,
  `if: ${{ !cancelled() }}`, and `docs_only` from the scope output. The pre-push hook scopes with the same script
  (KTD8).
- **Test scenarios:** `printf 'docs/x.md\n' | bash scripts/cargo-inputs.sh` prints nothing; `README.md`, `src/main.rs`,
  and `.github/workflows/ci.yml` each print themselves.
- **Verification:** the scope job's runs on a docs-only and a code commit.

### U8. bird: changelog bump check

- **Goal:** R4, R5.
- **Dependencies:** U4, U6 (it reads `completions/bird.bash`).
- **Files:** `scripts/check-surface-bump.sh` (new; xurl-rs's script with `completions/bird.bash`, the root `Cargo.toml`,
  and a `## Changelog` heading), `.github/workflows/changelog-bump.yml` (new caller).
- **Test scenarios:** `check-surface-bump.sh surface .` lists bird's commands and schema leaves; `check` against a base
  without one flag fails with a `### Changed`-only body and passes with an `### Added` bullet.
- **Verification:** the U4 runs, executed on this PR.

### U9. bird: anc gate

- **Goal:** R5, R7.
- **Dependencies:** U2, and stack 2 of the backlog merged.
- **Files:** `.github/workflows/ci.yml`.
- **Approach:** `anc_version: "0.6.0"`, `anc_bin: bird`, `anc_blocking_tiers: any`, and the floor set to the score stack
  2 leaves.
- **Verification:** `ci / Agent-native audit` green.

### U10. Request to the xurl-rs team: call the shared jobs

- **Goal:** R8.
- **Dependencies:** U1 to U4 merged.
- **Approach:** bird writes no code in xurl-rs; this is the request, sent once U1 to U4 are on `main`. xurl-rs keeps its
  scripts (`scripts/cargo-inputs.sh`, `scripts/lint-stdio.sh`, `scripts/generate-completions.sh`,
  `scripts/check-workflow-concurrency.sh`, `scripts/check-surface-bump.sh`) and its repo-specific jobs, and its `ci.yml`
  drops the `scope`, `lint-stdio`, `docs`, `completions`, `msrv`, and `anc` jobs for this caller:

  ```yaml
  jobs:
    scope:
      uses: brettdavies/.github/.github/workflows/rust-change-scope.yml@main
    ci:
      needs: scope
      if: ${{ !cancelled() }}
      uses: brettdavies/.github/.github/workflows/rust-ci.yml@main
      with:
        advisories_blocking: true
        docs_only: ${{ needs.scope.outputs.rust == 'false' }}
        docs: true
        docs_rs_package: xdk-rs
        msrv: true
        completions: true
        lint_stdio: true
        workflow_concurrency: true
        anc_version: "0.6.0"
        anc_bin: xr
        anc_score_floor: 98
        release_size_ceiling_bytes: 9600000
  ```

  Its `changelog-bump.yml` keeps its triggers and concurrency group and replaces its `surface` job with
  `uses: brettdavies/.github/.github/workflows/rust-changelog-bump.yml@main` under a `changelog-bump` job key. The
  required contexts in both rulesets move to the prefixed names (`Doc build` becomes `ci / Doc build`, `Change scope`
  becomes `scope / Change scope`, `Surface growth needs a minor section` becomes
  `changelog-bump / Surface growth needs a minor section`), a live-settings change for the user to approve.
- **Verification:** xurl-rs CI green with each moved check reporting under its new name; the xurl-rs team owns it.

### U12. Opt-in shellcheck job in `lint-basics.yml`

- **Goal:** R9.
- **Files:** `.github/workflows/lint-basics.yml` (in `brettdavies/.github`).
- **Approach:** `shellcheck: true` lints the tracked files `shellcheck-files` names (git pathspecs) at
  `shellcheck-severity`; a pathspec set that matches nothing fails, so a renamed directory cannot pass with nothing
  checked. xurl-rs-skill's caller would pass `shellcheck-files: 'scripts/*.sh tests/*.sh fixtures/bin/xr'` and
  `shellcheck-severity: style`.
- **Test scenarios:** a bird probe with the defaults passes over bird's tracked scripts; a seeded SC2164 fails it.
- **Verification:** both probe runs linked from the PR.

### U11. Required-check names for the rulesets

- **Goal:** lane A1's ruleset work names checks that exist.
- **Approach:** no code. bird's callers produce `scope / Change scope`, `ci / Doc build`, `ci / MSRV`,
  `ci / Completions freshness`, `ci / Output discipline`, `ci / Workflow concurrency`, `ci / Agent-native audit`, and
  `changelog-bump / Surface growth needs a minor section`. The last runs only on PRs whose base is not `main`, so it
  belongs in protect-dev and never in protect-main.
- **Test expectation:** none, it is a naming handoff.

---

## Verification Contract

| Check                  | Command or evidence                                                                                              | Applies to |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------- | ---------- |
| Workflow lint          | `actionlint` and `bash scripts/check-workflow-concurrency.sh` in `brettdavies/.github`                           | U1-U4, U12 |
| Shared jobs work       | a bird draft PR calling each workflow at the `.github` branch ref, green, plus the negative runs each unit names | U1-U8, U12 |
| bird scripts           | `shellcheck --severity=warning` and `shfmt -i 2 -ci -bn -d` on every new script                                  | U5, U7, U8 |
| bird gates             | `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, `cargo deny check`   | U5 to U9   |
| No consumer regression | the `.github` PR's own lint run, and a consumer run with no new input showing unchanged job verdicts             | U1, U2     |

## Definition of Done

- U1 to U4 and U12 are open as `.github` PRs with green lint and linked evidence runs, and merged once the user
  approves.
- U5 to U8 are merged in bird, calling `@main`; U9 is merged after stack 2.
- `guard-r20.yml` is gone and the R20 rule runs only through `scripts/lint-stdio.sh`.
- U10 has gone to the xurl-rs team as a request.
- No experimental workflow edits or scratch branches remain in either repository.

## Open Questions

- Whether xurl-rs's anc gate moves from its binary-only audit (`--command target/release/xr`, floor 98) to KTD6's
  source-and-binary audit; the shared job runs the latter, so its floor of 98 is re-measured on adoption.
- Whether `ci / Agent-native audit` becomes a required check on bird's `main`, given that an anc release can only reach
  it through a pin bump.
