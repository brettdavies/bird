# Pre-release verification: `bird`

Operational pre-flight checklist. Runs **before** step 1 of
[`RELEASES.md` § Releasing dev to main](./RELEASES.md#releasing-dev-to-main). Gates the cut of the `release/v<version>`
branch, not the daily dev integration. Each box is an explicit go/no-go. If any item is unchecked or red, hold the
release.

CI (fmt, clippy, test, cargo-deny, Windows-compat, package-check) catches mechanical regressions inside this repo. This
checklist covers what CI structurally can't:

- Behavioral drift against the live X (Twitter) API. CI runs unit and integration tests against mocked HTTP and stubbed
  transports, not the real endpoints, so API-contract regressions land silently until a user hits them.
- Embedded-transport reachability against a real X API on a fresh machine: env-var credentials, token-store credentials
  via `xr auth app`, and the missing-credentials fall-through. None of these are exercised end-to-end by the in-repo
  tests; bird mocks the bird/xurl boundary, and xurl-rs's own tests cover the HTTP layer.
- Distribution paths that only exercise on real artifacts (cross-compile binaries, `cargo install` from a clean machine,
  `cargo-binstall` against a published GitHub Release, `brew install` once the Homebrew dispatch finishes).
- Local state correctness on a fresh machine: bird does NOT store auth tokens itself (xurl owns the token store), but it
  does write `~/.config/bird/config.toml` (0644) for watchlist and preferences, and `~/.config/bird/bird.db` (0600) for
  the SQLite entity cache. Permissions and round-trip behavior need confirmation on a fresh `XDG_CONFIG_HOME`.

Post-tag verification (`release.yml` → homebrew-tap → `finalize-release.yml` → crates.io publish) lives in
[`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md). The tag push happens AFTER the release-branch cut and the
PR-to-main merge, so verification of the tag-triggered pipeline is post-flight, not pre-flight.

## Quick start: run the automated gates

The generic gates run from one script. Build the release binary first, then:

```bash
cargo build --release
scripts/release/preflight.sh all          # drift + surface + smoke + mechanics
```

The script (`scripts/release/preflight.sh`) is **project-authored** on the github-repo-setup skill's skeleton: the
shared scaffolding (gate helpers, 1Password reads, `shred -u` tempdir cleanup, subcommand dispatch, drift + surface +
mechanics gates) is the skeleton's. The `smoke` gate and the `seed_smoke_store` recipe are placeholders that SKIP with a
pointer to this file; the live-API sections below are the manual recipe until those bodies are filled in with bird's
checks. `all` runs the drift gate first, since nothing else matters while `main` holds changes `dev` never received. It
exits non-zero if any gate fails. Sub-commands let you re-run one gate group in isolation:

| Sub-command | What it runs                                                                                                                                                                 | Live API? |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `drift`     | Commits on `main` since the last release whose changes `dev` lacks, `.github/` parity, `Cargo.lock` packages `main` resolves newer (delegated to `scripts/release/drift.sh`) | no        |
| `surface`   | LAST_TAG resolution, commit/file/breaking-marker counts                                                                                                                      | no        |
| `smoke`     | Placeholder; SKIPs until `gate_smoke` carries bird's live-API checks (§ Real-world smoke)                                                                                    | yes       |
| `mechanics` | Cargo.toml version, lockfile present, `bird --version` match, CHANGELOG match, toolchain quarantine, advisories, leak check, unguarded docs added to `main`, diff-B          | no        |
| `all`       | every above                                                                                                                                                                  | yes       |

Flags:

- `--smoke-home PATH`: reuse an existing seeded `$SMOKE_HOME` (skip the seed)
- `--no-cleanup`: keep the temp home after exit (useful for follow-up `bird` probes)
- `--tag TAG`: override LAST_TAG auto-detection

The script shreds every tempdir that held credentials on exit (`shred -u`, three passes before unlinking; falls back to
`dd if=/dev/urandom + rm` if `shred` isn't on `PATH`; refuses to operate outside `/tmp` or `$HOME` as a path-typo
guardrail).

## Establish the surface

Everything below assumes you know what's changing. Run this first.

Driven by `scripts/release/preflight.sh surface`.

```bash
LAST_TAG=$(git tag --sort=-version:refname | head -n 1)
git log "$LAST_TAG..dev" --oneline                              # commits going out
git diff "$LAST_TAG..dev" --stat                                # file-level scope
git diff "$LAST_TAG..dev" -- src/ schema/                       # surface area: code + output schemas
git log "$LAST_TAG..dev" --grep '^[a-z]\+\(([^)]*)\)\?!:' --oneline   # Conventional-Commits breaking markers, scoped or not
```

On a repo with no tags yet, or whose lineage is squash-only so no tag is an ancestor of `dev`, the surface is
`origin/main..origin/dev` instead of `$LAST_TAG..dev`; `preflight.sh surface` SKIPs the tag counts in that case. bird's
`dev` and `main` share no history, so `git diff origin/main..origin/dev` is the surface here.

Every `!:` commit drives the major-version decision and gets a row in the release's `### Breaking changes` section.

## Checklist

### Branch drift (main ahead of dev)

Driven by `scripts/release/preflight.sh drift` (delegates to `scripts/release/drift.sh`).

Security PRs, hotfixes, and config edits land on `main` first. The release branch is cut from `main` and then takes
`dev`'s changes, so anything `main` holds that `dev` never received is reverted by the release or collides with it, and
Dependabot raises the same fix again.

- [ ] The previous release's bookkeeping (`Cargo.toml` and `CHANGELOG.md`) reached `dev`; gate 0 fails when it never
      did, and `scripts/sync-dev-after-release.sh v<version>` is the fix.
- [ ] Every commit on `main` since the last release has its changes on `dev` (gate 1 lists the ones that do not, as
      `differs` or `missing`). Backport them by PR into `dev` first, merge, and rerun.
- [ ] `.github/` is identical on both branches (gate 2). A difference either way is a config change that only reached
      one branch.
- [ ] No `Cargo.lock` package resolves newer on `main` than on `dev` (gate 3). The one benign case is a version still
      inside the local package manager's release-age window when the advisory is already patched at `dev`'s version.
- [ ] `dev`-newer packages are the routine updates this release ships; the gate counts them and does not list them.

### Dependabot preflight

Run before any other checklist work, before `Cargo.toml` is bumped, before any release branch is cut. Surfaces pending
dependency updates so they can be merged on dev (or rejected) instead of arriving as Dependabot PRs the moment the
release commit lands on the target branch: Cargo.lock churn triggers Dependabot's out-of-cycle re-evaluation, and at
that point the release is already tagged and they miss the cut.

- [ ] Trigger the workflow: GitHub → Actions → "Dependabot Preflight" → "Run workflow" (head = `dev`). The caller is
  `.github/workflows/dependabot-preflight.yml`.
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
`delete`), the `completions` generator, and the embedded xurl-rs client surface that owns auth and HTTP.

- [ ] `bird help` lists the same shortcut commands as the previous release plus any net additions or removals. Diff
  `$LAST_TAG`'s `bird help` against `dev`'s and confirm every removed or renamed command has a `!:` commit and a `###
  Changed` (or `### Breaking changes`) bullet in the release changelog.
- [ ] Per-command `--help` shape unchanged for stable commands. Spot-check `bird me --help`, `bird bookmarks --help`,
  `bird search --help`, `bird raw --help`; flag changes (renames, defaults, types) are user-facing and must show up in
  the changelog.

### Real-world smoke (live X API, via the embedded xurl-rs client)

Driven by `scripts/release/preflight.sh smoke` once `gate_smoke` carries these checks; until then this section is the
manual recipe.

The in-repo tests mock the bird/xurl boundary. The following exercises only fire end-to-end against credentials
configured in the embedded xurl-rs token store and the live X API. Pick fresh targets each release.

- [ ] `bird --version` returns the new `vX.Y.Z` value (post-bump).
- [ ] `bird doctor` exits 0 on a healthy machine, 78 on bad config, and reports the linked xurl-rs crate version under
  the `xurl` section. Spot-check `bird doctor <cmd>` for at least one shortcut command (e.g. `bird doctor me`) and
  confirm it reports the correct accepted / credentialed schemes.
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
- [ ] Write commands round-trip via the embedded `execute_embedded_write` dispatcher: `bird tweet "..."`, `bird reply
  <id> "..."`, `bird like <id>`, `bird unlike <id>`, `bird repost <id>`, `bird unrepost <id>`, `bird follow <user>`,
  `bird unfollow <user>`, `bird dm <user> "..."`, `bird block`, `bird unblock`, `bird mute`, `bird unmute`. Each must
  reject `--cache-only` with a Command error (exit code 1).
- [ ] Raw passthrough: `bird get /2/users/me -p id=123 -q expansions=author_id --pretty`, plus `bird post`, `bird put`,
  `bird delete` against a safe endpoint (e.g. a test workspace). The `-H/--header` flag flows headers into
  `RequestOptions.headers`.
- [ ] Completions: `bird completions bash`, `zsh`, `fish`, `powershell`, `elvish` each produce a parseable script (`bash
  -n`, `zsh -n`, `fish --no-execute`).
- [ ] Error paths: drive at least one auth failure (`XurlError::Auth`) and confirm bird exits 77 with the stderr error
  envelope; one config failure (`XurlError::Validation`, bad TOML) and confirm exit 78; one rate-limit failure
  (`XurlError::Api { status: 429 }`) and confirm exit 3; one command failure (e.g. `--cache-only` on a write command)
  and confirm exit 1.

### Embedded transport contract

bird embeds xurl-rs as a library. The transport contract has a small set of observable behaviors that only verify on a
real shell.

- [ ] App credentials via env: with `CLIENT_ID` + `CLIENT_SECRET` set in the shell, `bird login` runs through the
  embedded OAuth2 flow.
- [ ] App credentials via token store: with no `CLIENT_ID` in env but an app persisted via `xr auth app`, bird reads the
  same store and `bird login` succeeds.
- [ ] Missing credentials: with no `CLIENT_ID` and no stored app, an API-hitting command exits with the
  `ConstructionStub` "CLIENT_ID not set" message surfaced on stderr.
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
- [ ] `bird search "rust" --cache-only` serves from store only and never hits the X API. Write commands reject
  `--cache-only` with exit code 1.

### Token & file permissions

bird does not store tokens itself (xurl-rs's token store does), but it owns the local config and cache files.

- [ ] `~/.config/bird/config.toml` is created at `0644` on first watchlist or preference write.
- [ ] `~/.config/bird/bird.db` (SQLite entity cache) is created at `0600`.
- [ ] No bird-managed files end up world-readable that shouldn't be (audit with `find ~/.config/bird -ls`).

### Exit-code contract

Six exit codes, stable across releases. The 3 / 4 / 5 codes are inherited verbatim from xurl-rs's `exit_code_for_error`
via `XurlError`-to-`BirdError` translation; 77 / 78 are bird-side overrides.

- [ ] `0` on success.
- [ ] `77` on auth failures (`XurlError::Auth`, `XurlError::TokenStore`, `XurlError::Api { status: 401 }`,
  `XurlError::AuthMethodMismatch`).
- [ ] `78` on config failures (`XurlError::Validation`, invalid `config.toml`, broken `XDG_CONFIG_HOME`).
- [ ] `3` on rate-limit (`XurlError::Api { status: 429 }` or `XurlError::Http("429 …")`).
- [ ] `4` on not-found (`XurlError::Api { status: 404 }` or `XurlError::Http("404 …")`).
- [ ] `5` on network errors (`XurlError::Io`).
- [ ] `1` on everything else (other API errors, write-command-with-`--cache-only`, etc.).
- [ ] Stderr JSON envelope on `--output json`: `{"error":..., "kind":"config"|"auth"|"command", "exit_code":<N>,
  "command":..., "status":...}`. Verify shape on at least one error from each kind.

### Distribution and install paths

The release builds cross-compiled binaries and the homebrew tap dispatches downstream. None of this runs in `cargo
test`.

- [ ] Last green run of `release.yml` (on this branch or a sibling) cross-compiled all five targets listed in
  `RELEASES.md` § Tagging and publishing. If the workflow has changed since, dry-run with `cargo build --release
  --target <target>` for each.
- [ ] In a clean container or fresh machine: download a **prior** release archive (`bird-<target>.tar.gz` or `.zip` for
  Windows), run `bird --version` and one read-only shortcut. Confirms the archive layout (binary + completions +
  licenses) still works without the project's toolchain. Install of the **newly** published artifact happens post-tag
  in [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md).

### Release mechanics sanity

Driven by `scripts/release/preflight.sh mechanics`.

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
- [ ] Triple-diff verification before tag: `git diff origin/main..HEAD`, `git diff HEAD..origin/dev` filtered by the
  guarded set (not all of `docs/`, since `docs/CLI_DESIGN.md`, `docs/DEVELOPER.md`, and `docs/SECRETS.md` ship to
  `main` and a wholesale exclusion would hide a missed change there), `git diff origin/dev..origin/main` (sanity): all
  three agree on intended scope.
- [ ] **Leak check before pushing the release branch.** No guarded path may surface in the diff vs `origin/main`. The
  set resolves from `.github/workflows/guard-main-docs.yml` via `scripts/release/guarded-paths.sh`; never restate the
  pattern inline. If cherry-picks pulled in guarded paths via rename detection, resolve per `RELEASES.md` § Cherry-pick
  conflicts on guarded paths.

  ```bash
  GUARDED="$(scripts/release/guarded-paths.sh)"
  git diff origin/main..HEAD --name-only | grep -E "$GUARDED" && echo "LEAKED: reset and redo" || echo "(clean)"
  ```

- [ ] **Every doc this release adds to `main` is meant to ship.** The leak check screens against the registered set, so
  it cannot flag a category nobody registered yet. Enumerate the additions under `docs/` and every added markdown file
  anywhere, and read them; an entry that should not ship gets registered in the workflow's `extra_paths` and removed
  from the branch.

  ```bash
  git diff origin/main..HEAD --diff-filter=A --name-only | grep -E '(^docs/|\.md$)' | grep -Ev "$GUARDED" || echo "(none unguarded)"
  ```

- [ ] `CHANGELOG.md` versioned section has no `[Unreleased]` placeholder and matches the bumped `Cargo.toml` version.

### Post-tag verification

Moved to [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md) because tagging happens **after** the release-branch cut
and PR-to-main merge, so verification of the tag-triggered pipeline (release.yml → homebrew-tap → finalize-release →
crates.io publish → fresh-machine install smokes) is post-flight, not pre-flight. Run `scripts/release/postflight.sh
all` immediately after `git push origin vX.Y.Z`.

## Related docs

- [`RELEASES-POSTFLIGHT.md`](./RELEASES-POSTFLIGHT.md): runs AFTER the tag push to verify the downstream pipeline.
- [`RELEASES.md`](./RELEASES.md): operational runbook this checklist gates.
- [`RELEASES-RATIONALE.md`](./RELEASES-RATIONALE.md): release-flow rationale.
- [`AGENTS.md`](./AGENTS.md): project structure, transport contract, output formats.
