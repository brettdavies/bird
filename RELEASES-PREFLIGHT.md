# Pre-release verification: `bird`

Operational pre-flight checklist. Runs **before** step 1 of
[`RELEASES.md` § Releasing dev to main](./RELEASES.md#releasing-dev-to-main). Gates the cut of the `release/v<version>`
branch, not the daily dev integration. Each box is an explicit go/no-go. If any item is unchecked or red, hold the
release.

CI (fmt, clippy, test, cargo-deny, Windows-compat, package-check) catches mechanical regressions inside this repo. This
checklist covers what CI structurally can't:

- Behavioral drift against the live X (Twitter) API. CI runs unit and integration tests against mocked HTTP and stubbed
  transports, not the real endpoints, so API-contract regressions land silently until a user hits them.
- Subprocess-transport contract correctness against a real `xr` (xurl-rs) or `xurl` (Go fallback) on a fresh machine
  (the discovery / override / missing-binary paths are not exercised by the in-repo tests in the same way they are on
  first run).
- Distribution paths that only exercise on real artifacts (cross-compile binaries, `cargo install` from a clean machine,
  `cargo-binstall` against a published GitHub Release, `brew install` once the Homebrew dispatch finishes).
- Local state correctness on a fresh machine: bird does NOT store auth tokens itself (xurl owns the token store), but it
  does write `~/.config/bird/config.toml` (0644) for watchlist and preferences, and `~/.config/bird/bird.db` (0600) for
  the SQLite entity cache. Permissions and round-trip behavior need confirmation on a fresh `XDG_CONFIG_HOME`.

## Establish the surface

Everything below assumes you know what's changing. Run this first.

```bash
LAST_TAG=$(git tag --sort=-version:refname | head -n 1)
git log "$LAST_TAG..dev" --oneline                              # commits going out
git diff "$LAST_TAG..dev" --stat                                # file-level scope
git diff "$LAST_TAG..dev" -- src/ openapi/                      # surface area: code + API spec
git log "$LAST_TAG..dev" --grep '^[a-z]\+!:' --oneline          # Conventional-Commits breaking markers
```

Every `!:` commit drives the major-version decision and gets a row in the release's `### Breaking changes` section.

## Checklist

### `embedded-xurl` feature-flag transition (active 3-PR cutover window)

Active for the duration of the xurl-rs embedding refactor (PR1 -> PR2 -> PR3 on `dev`). Removed once PR3 lands the
cutover and the `embedded-xurl` feature flag itself is gone.

- [ ] `cargo build` (default features = subprocess transport) succeeds on `dev` HEAD.
- [ ] `cargo build --features embedded-xurl` succeeds on `dev` HEAD.
- [ ] `cargo build --no-default-features --features embedded-xurl` succeeds on `dev` HEAD.
- [ ] `cargo test` (default features) is green on `dev` HEAD — the subprocess transport contract is locked.
- [ ] `.github/workflows/embedded-xurl-build.yml` is present and green on the most recent `dev` push. If it is missing,
  the cutover has either not started or has completed; reconcile against the active plan in
  `docs/plans/2026-06-05-001-refactor-embed-xurl-crate-plan.md`.

### Dependabot preflight

Run before any other checklist work, before `Cargo.toml` is bumped, before any release branch is cut. Surfaces pending
dependency updates so they can be merged on dev (or rejected) instead of arriving as Dependabot PRs the moment the
release commit lands on the target branch — Cargo.lock churn triggers Dependabot's out-of-cycle re-evaluation, and at
that point the release is already tagged and they miss the cut.

- [ ] Trigger the workflow: GitHub → Actions → "Dependabot Preflight" → "Run workflow" (head = `dev`). Pinned at
  `brettdavies/.github/.github/workflows/dependabot-preflight.yml`.
- [ ] Review the `cargo` job's `cargo outdated --workspace --depth 1` report in the run summary. For each direct dep
  with a newer compatible version, decide: merge an update PR on dev now, accept the stale version this release, or rule
  out the update with a `Cargo.toml` constraint.
- [ ] Review the `github-actions` job's pin-drift table. For every drifted action, bump the pinned SHA on dev and update
  the trailing `# <version>` comment.
- [ ] (Optional) Trigger Dependabot to open PRs for whatever the preflight surfaced: GitHub → Insights → Dependency
  graph → Dependabot → "Check for updates". Wait for the PRs to land; merge anything that passes CI on dev.

Only after this list is empty do you cut the release branch.

### Command-surface contract

bird's contract is the union of the typed shortcut commands (`me`, `bookmarks`, `search`, `thread`, `profile`,
`watchlist`, `usage`, `cache`, write commands, `doctor`), the generic raw-HTTP commands (`get` / `post` / `put` /
`delete`), the `completions` generator, and the subprocess transport surface that delegates to xurl.

- [ ] `bird help` lists the same shortcut commands as the previous release plus any net additions or removals. Diff
  `$LAST_TAG`'s `bird help` against `dev`'s and confirm every removed or renamed command has a `!:` commit and a `###
  Changed` (or `### Breaking changes`) bullet in the release changelog.
- [ ] Per-command `--help` shape unchanged for stable commands. Spot-check `bird me --help`, `bird bookmarks --help`,
  `bird search --help`, `bird raw --help` — flag changes (renames, defaults, types) are user-facing and must show up in
  the changelog.

### Real-world smoke (live X API, via xurl)

The in-repo tests mock the HTTP layer. The following exercises only fire end-to-end against a logged-in xurl install and
the live X API. Pick fresh targets each release.

- [ ] `bird --version` returns the new `vX.Y.Z` value (post-bump).
- [ ] `bird doctor` exits 0 on a healthy machine, 78 on missing xurl, 78 on bad config. Spot-check `bird doctor <cmd>`
  for at least one shortcut command (e.g. `bird doctor me`) and confirm it reports the correct auth requirement.
- [ ] `bird me --pretty`, `bird me --output json`, `bird me -q` round-trip via the entity store on a logged-in account.
  The `--pretty` form shows ANSI color and hyperlinks on a TTY.
- [ ] `bird bookmarks` paginates and streams (does not collect into memory before printing).
- [ ] `bird search "rust" --sort likes --min-likes 100 --pages 2 --output json` returns sorted JSON.
- [ ] `bird thread <tweet_id>` reconstructs a multi-page thread.
- [ ] `bird profile <handle>` resolves via `schema::validate_username` (strips `@`, enforces charset).
- [ ] `bird watchlist add @x`, `bird watchlist list`, `bird watchlist check`, `bird watchlist remove @x` round-trip via
  the local config (`~/.config/bird/config.toml`).
- [ ] `bird usage --local`, `bird usage --sync` exercise the usage subsystem; both respect `--pretty`.
- [ ] `bird cache stats --pretty` shows store path, size, tweet/user/raw counts; `bird cache clear` drops counts to
  zero.
- [ ] Write commands round-trip via xurl passthrough: `bird tweet "..."`, `bird reply <id> "..."`, `bird like <id>`,
  `bird unlike <id>`, `bird repost <id>`, `bird unrepost <id>`, `bird follow <user>`, `bird unfollow <user>`, `bird dm
  <user> "..."`, `bird block`, `bird unblock`, `bird mute`, `bird unmute`. Each must reject `--cache-only` with a
  Command error (exit code 1).
- [ ] Raw passthrough: `bird get /2/users/me -p id=123 -q expansions=author_id --pretty`, plus `bird post`, `bird put`,
  `bird delete` against a safe endpoint (e.g. a test workspace).
- [ ] Completions: `bird completions bash`, `zsh`, `fish`, `powershell`, `elvish` each produce a parseable script (`bash
  -n`, `zsh -n`, `fish --no-execute`).
- [ ] Error paths: drive at least one auth failure (XurlError::Auth) and confirm bird exits 77 with the stderr error
  envelope; one config failure (missing xurl, bad TOML) and confirm exit 78; one command failure (e.g. `--cache-only` on
  a write command) and confirm exit 1.

### Subprocess transport contract

bird delegates all HTTP and auth to `xurl`. The transport contract has four observable behaviors that only verify on a
real shell.

- [ ] Override resolution: `BIRD_XURL_PATH=/path/to/xr bird raw /2/users/me` uses the overridden binary.
- [ ] Default discovery: with `BIRD_XURL_PATH` unset, bird finds `xr` (xurl-rs) first, then falls back to `xurl` (Go).
- [ ] Missing-xurl exit: with `BIRD_XURL_PATH` pointing at a non-existent path, `bird me` exits 78 (config) with the
  install hint surfaced on stderr.
- [ ] SIGPIPE: `bird bookmarks --output jsonl | head -1` exits cleanly (no broken-pipe panic). The Unix-only
  `libc::signal(SIGPIPE, SIG_DFL)` is `#[cfg(unix)]`-gated; the pre-push hook's libc-grep is the always-on backstop.

### Output and color contract

bird picks output format and color from a precedence chain (CLI flag > env var > TTY detection). Each rule has a
specific behavioral fixture.

- [ ] `bird me --pretty` shows ANSI color and hyperlinks on a TTY.
- [ ] `bird me --plain` strips color and hyperlinks.
- [ ] `bird me --no-color` or `NO_COLOR=1 bird me` strips color only (hyperlinks remain).
- [ ] `bird me 2>/tmp/err 1>/tmp/out` (stderr non-TTY) auto-selects JSON for the stderr error envelope.
- [ ] `bird me --output json` and `BIRD_OUTPUT=json bird me` both force JSON on stderr errors.
- [ ] `bird me -q` suppresses informational stderr diagnostics.

### Cache modes

bird's SQLite cache is gated by three mutually-exclusive flags (`--refresh`, `--no-cache`, `--cache-only`). Write
commands reject `--cache-only`.

- [ ] `bird search "rust" --refresh` bypasses store, refreshes store.
- [ ] `bird search "rust" --no-cache` neither reads nor writes the store.
- [ ] `bird search "rust" --cache-only` serves from store only and never invokes xurl. Write commands reject
  `--cache-only` with exit code 1.

### Token & file permissions

bird does not store tokens itself (xurl does), but it owns the local config and cache files.

- [ ] `~/.config/bird/config.toml` is created at `0644` on first watchlist or preference write.
- [ ] `~/.config/bird/bird.db` (SQLite entity cache) is created at `0600`.
- [ ] No bird-managed files end up world-readable that shouldn't be (audit with `find ~/.config/bird -ls`).

### Exit-code contract

Three exit codes, stable across releases.

- [ ] `0` on success.
- [ ] `77` on auth failures detected via XurlError::Auth.
- [ ] `78` on config failures (missing xurl binary, invalid `config.toml`, broken `XDG_CONFIG_HOME`).
- [ ] `1` on everything else (Command errors, write-command-with-`--cache-only`, etc.).
- [ ] Stderr JSON envelope on `--output json`: `{"error":..., "kind":"config"|"auth"|"command", "code":78|77|1,
  "command":..., "status":...}`. Verify shape on at least one error from each kind.

### Distribution and install paths

The release builds cross-compiled binaries and the homebrew tap dispatches downstream. None of this runs in `cargo
test`.

- [ ] Last green run of `release.yml` (on this branch or a sibling) cross-compiled all five targets listed in
  `RELEASES.md` § Tagging and publishing. If the workflow has changed since, dry-run with `cargo build --release
  --target <target>` for each.
- [ ] In a clean container or fresh machine: download a prior release archive (`bird-<target>.tar.gz` or `.zip` for
  Windows), run `bird --version` and one read-only shortcut. Confirms the archive layout (binary + completions +
  licenses) still works without the project's toolchain.
- [ ] `cargo install bird --version <new>` from a clean environment resolves and runs once the crates.io publish
  completes (post-tag check, see below).

### Release mechanics sanity

These items duplicate steps in `RELEASES.md` deliberately: easy to skip, expensive to recover from. Confirm explicitly.

- [ ] `Cargo.toml` `version` bumped to the new tag value (`check-version` in `release.yml` enforces this; catch early).
- [ ] `Cargo.lock` regenerated via `cargo update -p bird`, committed.
- [ ] Rebuild locally, confirm `bird --version` prints the new tag value.
- [ ] Every PR merged since `$LAST_TAG` has a non-empty `## Changelog` section. Spot-check via `gh pr list --base dev
  --state merged --search "merged:>$(git log -1 --format=%aI $LAST_TAG)"` then `gh pr view <num> --json body`.
- [ ] `rust-toolchain.toml` last bumped ≥7 days ago (supply-chain quarantine). If a bump landed inside the window, hold
  or revert it before tagging.
- [ ] No unmerged dependency advisories from `cargo deny check advisories`. The full local pre-push check
  (`scripts/hooks/pre-push`) mirrors CI; run it explicitly before pushing the release branch.
- [ ] Triple-diff verification before tag: `git diff origin/main..HEAD`, `git diff HEAD..origin/dev` (no non-doc paths),
  `git diff origin/dev..origin/main` (sanity) — all three agree on intended scope.
- [ ] Leak check: `git diff origin/main..HEAD --name-only | grep -E
  '^(docs/plans|docs/brainstorms|docs/ideation|docs/reviews|docs/solutions|\.context)'` returns nothing. If cherry-picks
  pulled in guarded paths via rename detection, resolve per `RELEASES.md` § Cherry-pick conflicts on guarded paths.
- [ ] `CHANGELOG.md` versioned section has no `[Unreleased]` placeholder and matches the bumped `Cargo.toml` version.

### Post-tag verification

Run immediately after the tag push triggers `release.yml`.

- [ ] `release.yml` green end-to-end. `gh run watch <id> --exit-status` then verify with `gh run view <id> --json
  conclusion --jq .conclusion` — the watcher exit code alone is not authoritative.
- [ ] Homebrew-tap `update-formula` dispatch completed (check `gh run list -R brettdavies/homebrew-tap`), then
  `finalize-release.yml` ran back here and flipped the GitHub Release `make_latest: true`.
- [ ] `crates.io` shows the new version published. `cargo install bird --version <new>` from a clean environment
  resolves and runs.
- [ ] `brew update && brew install brettdavies/tap/bird` on a fresh prefix resolves the new bottle and `bird --version`
  reports the new tag. Confirms the homebrew-tap end of the cross-repo dispatch chain landed cleanly.
- [ ] `cargo binstall bird` (without `--version`) resolves to the new tag and installs the matching prebuilt binary.
  Confirms the GitHub Release asset layout matches binstall's expectations.
- [ ] Backport `main` → `dev` per `RELEASES.md` § After publish, then `git push origin dev`.

## Related docs

- [`RELEASES.md`](./RELEASES.md): operational runbook this checklist gates.
- [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md): release-flow rationale.
- [`AGENTS.md`](./AGENTS.md): project structure, transport contract, output formats.
