# Post-release verification: `bird`

Operational post-flight checklist. Runs **after** the `release/v<version> → main` PR merges and you push the tag (`git
push origin vX.Y.Z`) per [`RELEASES.md` § Tagging and publishing](./RELEASES.md#tagging-and-publishing). Verifies that
the tag-triggered pipeline landed cleanly across `release.yml` → `homebrew-tap` → `finalize-release.yml`, and that the
published artifacts resolve on the public distribution channels.

Companion to [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md), which gates the release-branch cut. Both docs follow
the same go/no-go shape: every box is explicit, an unchecked or red item holds the next release (or motivates a hotfix).

## Quick start: run the automated gates

```bash
scripts/release/postflight.sh all
```

The script (`scripts/release/postflight.sh`) is a verbatim copy of the github-repo-setup skill's template and covers
the automatable post-tag gates: `release.yml` end-to-end, homebrew-tap dispatch, `finalize-release.yml` callback,
GitHub Release `make_latest` flip, crates.io publish verification, and the `main → dev` backport check.
Install-on-fresh-machine smokes (`cargo install`, `brew install`, `cargo binstall`) are documented but not driven from
the script: running them on the local dev machine pollutes its toolchain and doesn't actually exercise the
fresh-machine semantics. Drive those on a throwaway container or a sibling machine.

`--env staging|prod` is optional and defaults to `prod`. bird is a single-env CLI, so every gate behaves identically and
the flag can be ignored; the `surface-smoke` gate auto-SKIPs because no `scripts/release/surface-smoke.sh` is vendored
(there is no deployed surface to smoke).

Sub-commands let you re-run one verification in isolation:

| Sub-command     | What it checks                                                                                                   | Source of truth                           |
| --------------- | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| `release`       | `release.yml` on the tag push: `gh run view ... --json conclusion` is `"success"`                                | `gh run view`                             |
| `tap`           | `brettdavies/homebrew-tap` `update-formula` (repository_dispatch) + `Publish bottles` (workflow_run) ran SUCCESS | `gh run list -R brettdavies/homebrew-tap` |
| `finalize`      | `finalize-release.yml` callback ran in this repo (cross-repo dispatch loop closed)                               | `gh run list -e repository_dispatch`      |
| `make-latest`   | GitHub Release `vX.Y.Z` is non-draft, non-prerelease, and `releases/latest` resolves to it                       | `gh api /releases/latest`                 |
| `crates`        | `crates.io` shows `bird vX.Y.Z` published (`cargo search bird` returns the tag)                                  | `crates.io` index API                     |
| `backport`      | a merged PR to `dev` carrying the released tag in its title (durable signal that the sync ran)                   | `gh pr list --base dev --state merged`    |
| `surface-smoke` | auto-SKIPs: bird vendors no `surface-smoke.sh`                                                                   | n/a                                       |
| `all`           | every above                                                                                                      | all of the above                          |

The `tap` and `finalize` gates accept only downstream runs created at or after this tag's `release.yml` run, and SKIP
until that run exists: the tap repo is shared across every CLI, so a name-only match would return another release's
runs while this one is still queued.

Flags:

- `--env staging|prod`: target environment (default: `prod`); ignored here
- `--repo OWNER/REPO`: override the auto-detected nameWithOwner
- `--tap-repo OWNER/REPO`: override the homebrew-tap repo (default: `brettdavies/homebrew-tap`)
- `--tag vX.Y.Z`: override auto-detection (default: derived from `Cargo.toml` version, falls back to latest git tag)
- `--crate NAME`: override the crate name for the `crates` gate (default: `Cargo.toml` `[package].name`)
- `--staging-url URL` / `--prod-url URL`: surface-smoke URLs; unused here

## Checklist

Run immediately after the tag push triggers `release.yml`.

- [ ] **`release.yml` green end-to-end.** `gh run watch <id> --exit-status` then verify with `gh run view <id> --json
  conclusion --jq .conclusion` because the watcher exit code alone is not authoritative (a completed watcher is not a
  green watcher). Builds the five cross-compile targets, publishes to crates.io via OIDC Trusted Publishing, and
  dispatches `update-formula` into the homebrew-tap. Run `scripts/release/postflight.sh release` for the automated
  check.
- [ ] **Homebrew-tap dispatch landed.** `gh run list -R brettdavies/homebrew-tap --limit 5` should show a recent
  `update-formula` (event=repository_dispatch) and a `Publish bottles` (event=workflow_run) both SUCCESS. The bottles
  workflow auto-merges the formula bump PR and pushes a `bird: add <version> bottle.` commit to tap `main`. Run
  `scripts/release/postflight.sh tap` for the automated check.
- [ ] **`finalize-release.yml` callback ran.** After the bottles publish, the tap dispatches back to this repo and the
  callback flips the GitHub Release `make_latest: true`. Check `gh run list -e repository_dispatch --limit 3`; expect a
  `finalize-release` SUCCESS. Run `scripts/release/postflight.sh finalize` for the automated check.
- [ ] **GitHub Release marked latest.** `gh api repos/brettdavies/bird/releases/latest --jq .tag_name` returns
  `vX.Y.Z`, not the previous tag. Confirms `finalize-release.yml` actually flipped the flag. Run
  `scripts/release/postflight.sh make-latest` for the automated check.
- [ ] **`crates.io` shows the new version published.** `cargo search bird` lists the new version. Run
  `scripts/release/postflight.sh crates` for the automated index check.
- [ ] **`cargo install bird --version <new>`** on a clean environment resolves and runs. Drive on a fresh container or
  a sibling machine so the local `~/.cargo/bin` isn't polluted. Confirms the crates.io publish landed all package data
  and `cargo install` can reconstruct the binary from source.
- [ ] **`brew update && brew install brettdavies/tap/bird`** on a fresh prefix resolves the new bottle and `bird
  --version` reports the new tag. Drive on a throwaway prefix (`HOMEBREW_PREFIX=/tmp/brew-postflight-X brew ...`).
  Confirms the homebrew-tap end of the cross-repo dispatch chain landed cleanly and the published bottle SHA matches the
  formula.
- [ ] **`cargo binstall bird`** (without `--version`) resolves to the new tag and installs the matching prebuilt binary.
  Confirms the GitHub Release asset layout (binary + completions + licenses, `bird-<target>.tar.gz` naming per
  `[package.metadata.binstall]`) matches binstall's asset-resolution rules. Drive on a clean container.
- [ ] **Last-good identifier recorded.** Before the release goes live, note the three identifiers a rollback needs
  somewhere reachable under incident pressure: the previous crate version on crates.io (`cargo search bird`), the
  previous GitHub Release tag (`gh api repos/brettdavies/bird/releases/latest --jq .tag_name`), and the formula bump
  and bottle commits on `brettdavies/homebrew-tap` `main` (`git log -2 --format=%H -- Formula/bird.rb`). Commands are
  in [`RELEASES.md` § Rollback](./RELEASES.md#rollback).
- [ ] **Rollback path confirmed.** If this release is bad, roll back at the surface first (`cargo yank`, re-point
  `releases/latest`, revert the formula bump), then land a `fix` or `revert` through the normal `dev` to `release/*` to
  `main` flow so `main` reconverges with what is live.
- [ ] **Sync `dev` with the release** via a **merged PR to `dev` carrying the released tag in its title.**
  `scripts/sync-dev-after-release.sh v<X.Y.Z>` cuts the branch, writes the released version into `Cargo.toml`,
  refreshes the crate's `Cargo.lock` entry offline, copies `CHANGELOG.md` from `main`, adopts the release-prep edits it
  discovers since the previous tag, and opens the PR; merge it once CI is green. Contested paths (both branches moved
  them) are listed and withheld: run with `--dry-run` first to see both lists without creating a branch, then name the
  contested paths to take with `--only PATH` (see
  [`RELEASES.md` § After publish](./RELEASES.md#after-publish-sync-dev-with-the-release)). Keeps the next release's
  PREFLIGHT `diff-B` step quiet so a real missed change stands out instead of hiding in expected divergence noise.

  The gate (`scripts/release/postflight.sh backport`) is signal-agnostic about which files moved: it searches merged PRs
  to `dev` by the tag (the search index tokenizes `v0.3.0` as one word, so a bare `0.3.0` misses it) and accepts either
  spelling in the title. Its SKIP names the sync command to run.

  ```bash
  scripts/sync-dev-after-release.sh v<X.Y.Z>
  ```

## Related docs

- [`RELEASES-PREFLIGHT.md`](./RELEASES-PREFLIGHT.md): pre-cut go/no-go checklist (runs BEFORE this one).
- [`RELEASES.md`](./RELEASES.md): operational runbook for the full release lifecycle.
- [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md): release-flow rationale.
