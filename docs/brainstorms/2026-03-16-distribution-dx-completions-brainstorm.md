# Distribution, DX, and Publication Alignment

**Date:** 2026-03-16
**Status:** Completed
**Scope:** Publication readiness (license, release workflow, changelog, crates.io, Homebrew), shell completions, agentic CLI flags
**Alignment reference:** `docs/notes/bird-alignment.md` and `~/dev/xurl-rs/` patterns

## What We're Building

Publication-ready infrastructure for Bird CLI, aligned with the xurl-rs release standard. Two tiers:

### Tier 1: Publication Foundation (blockers for crates.io and Homebrew)

1. **Dual license** — Add `LICENSE-MIT` and `LICENSE-APACHE` files; update `Cargo.toml` to `license = "MIT OR Apache-2.0"` (blocker for crates.io)
2. **Release workflow expansion** — Add 2 missing build targets (5 total), add crates.io publish job, switch to `.tar.gz`/`.zip` archives
3. **RELEASING.md** — Document the release process (tag, push, CI does the rest)
4. **Changelog infrastructure** — Add `cliff.toml`, changelog CI job, generate initial `CHANGELOG.md`
5. **README install section** — Document all 4 install channels (Homebrew, cargo, binary, source)
6. **Homebrew tap update** — Evolve existing `brettdavies/homebrew-tap` `Formula/bird.rb` from source-build to pre-built binary; add bump-formula automation
7. **Cargo.toml cleanup** — Add security-sensitive docs to `exclude`, remove dead `BIRD_DEFAULT_CLIENT_ID` from release workflow

### Tier 2: DX Improvements (ship after foundation)

1. **Shell completions** — `bird completions <shell>` subcommand via `clap_complete`; pre-generated scripts in release archives; Homebrew formula installs them
2. **Agentic polish** — `--quiet` / `-q` global flag; document machine-readable contract in README

## Why This Approach

The previous brainstorm session split work into 3 PRs (completions, quiet, distribution) but the priority order was inverted. Shell completions and `--quiet` are valuable DX but not blockers for publication. The alignment notes (`docs/notes/bird-alignment.md`) from the dev team correctly identify the foundation gaps:

- **No LICENSE files** — blocker for crates.io
- **Only 3 release targets** — missing `aarch64-unknown-linux-gnu` and `x86_64-apple-darwin`
- **No crates.io publish job** — manual only
- **No changelog automation** — no `cliff.toml` or `CHANGELOG.md`
- **No RELEASING.md** — undocumented release process
- **Homebrew formula builds from source** — slow, should use pre-built binaries
- **`BIRD_DEFAULT_CLIENT_ID` is dead code** — set in release.yml but never referenced after xurl refactor removed `auth.rs`/`login.rs`

Bird already has a strong agentic foundation (JSON stdout, structured exit codes, `--plain`, `bird doctor`). The foundation tier unblocks publication; the DX tier polishes the experience.

## Key Decisions

### License: dual MIT OR Apache-2.0

- Matches xurl-rs exactly
- Copy `LICENSE-MIT` and `LICENSE-APACHE` from xurl-rs (same copyright holder)
- Update `Cargo.toml`: `license = "MIT OR Apache-2.0"`
- The existing `brettdavies/homebrew-tap` `Formula/bird.rb` already uses `any_of: ["MIT", "Apache-2.0"]` — will be in sync after this change

### Release targets: expand to 5

| Target | Runner | Notes |
|--------|--------|-------|
| `x86_64-unknown-linux-gnu` | ubuntu-22.04 | Already exists |
| `aarch64-unknown-linux-gnu` | ubuntu-22.04 | New — needs `cross` tool |
| `aarch64-apple-darwin` | macos-14 | Already exists |
| `x86_64-apple-darwin` | macos-13 | New — Intel Mac |
| `x86_64-pc-windows-msvc` | windows-latest | Already exists |

### Distribution: four channels

| Channel | Command | Audience |
|---------|---------|----------|
| Homebrew tap | `brew install brettdavies/tap/bird` | macOS/Linux users |
| crates.io | `cargo install bird` | Rust developers |
| GitHub Releases | Download `.tar.gz`/`.zip` from releases page | Everyone else |
| Clone + build | `cargo build --release` | Contributors |

### Homebrew tap: evolve existing repo

- The `brettdavies/homebrew-tap` repo already exists with `Formula/bird.rb` (source build) and `Formula/xurl-rs.rb`
- Evolve `bird.rb` from source-build to pre-built binary formula (faster install, no Rust toolchain needed)
- Add `bump-formula.yml` workflow for automated updates on tag push
- xurl is a **cask** (`xdevplatform/tap/xurl`), not a formula — use `caveats` block, not `depends_on`

### crates.io: publish with dependency guidance

- `cargo install bird` works but requires xurl installed separately
- `bird doctor` already checks for xurl and reports its status
- README documents the xurl dependency prominently
- Consider crates.io Trusted Publishing (GitHub Actions OIDC) as alternative to API token

### Cargo.toml security cleanup

- Add `docs/SECRETS.md` and `docs/DEVELOPER.md` to `exclude` (contain 1Password vault paths)
- Remove `BIRD_DEFAULT_CLIENT_ID` from `release.yml` (dead code after xurl refactor)
- Consider also excluding `rust-toolchain.toml` and `rustfmt.toml` (developer tooling)

### Shell completions (Tier 2): runtime subcommand + build-time assets

- `bird completions bash|zsh|fish|powershell|elvish` subcommand using `clap_complete`
- Pre-generated scripts included in GitHub Release archives
- Homebrew formula installs completions automatically
- Bypass xurl fail-fast check for completions (zero dependencies on config/auth/db)

### Agentic flags (Tier 2): minimal additions

- Add `--quiet` / `-q` global flag — suppresses informational stderr, keeps fatal errors
- `--dry-run` deferred to future (write commands not yet implemented)
- Document exit code contract and machine-readable patterns in README

## Implementation Order

### PR 1: Publication foundation (license + Cargo.toml + release workflow)

- Add `LICENSE-MIT` and `LICENSE-APACHE`
- Update `Cargo.toml`: `license = "MIT OR Apache-2.0"`, expand `exclude`
- Remove dead `BIRD_DEFAULT_CLIENT_ID` from `release.yml`
- Expand release matrix to 5 targets (add `aarch64-unknown-linux-gnu` via `cross`, `x86_64-apple-darwin`)
- Add crates.io publish job to `release.yml`
- Add version tag / Cargo.toml consistency check
- Add `cliff.toml` and changelog CI job
- Generate initial `CHANGELOG.md`
- Create `RELEASING.md`
- Update README with install section
- Verify with `cargo publish --dry-run` and `cargo package --list`

### PR 2: Homebrew tap evolution

- Update `brettdavies/homebrew-tap` `Formula/bird.rb` from source-build to pre-built binary
- Add `bump-formula.yml` automation workflow
- Add trigger step in bird's `release.yml` to kick off formula bump
- Add `HOMEBREW_TAP_TOKEN` secret (fine-grained PAT)

### PR 3: Shell completions

- Add `clap_complete` dependency
- Add `Completions` variant to `Command` enum
- Bypass xurl fail-fast check for completions and doctor
- Switch release archives to `.tar.gz`/`.zip` with completions included
- Update Homebrew formula to install completions

### PR 4: Agentic polish

- Add `--quiet` / `-q` global flag with `BIRD_QUIET` env var
- Thread quiet through all 43 `eprintln!` call sites
- Document machine-readable contract in README

## Resolved Questions

- **License type?** — Dual MIT OR Apache-2.0 (matching xurl-rs)
- **Homebrew tap repo?** — Already exists at `brettdavies/homebrew-tap`, just needs evolution
- **`BIRD_DEFAULT_CLIENT_ID`?** — Dead code after xurl refactor, safe to remove
- **crates.io name availability?** — `bird` confirmed available (404 on API, 2026-03-16)
- **xurl dependency in formula?** — Use `caveats` block (xurl is a cask, not a formula)

## Open Questions

None — all key decisions resolved.

## Future Work

- `--dry-run` flag for write commands (tweet, like, follow, dm, etc.)
- `--output-format json|text` if the `--pretty` pattern becomes insufficient
- Submit to homebrew-core once Bird has traction
- Man page generation (clap_mangen)
- crates.io Trusted Publishing (evaluate after initial manual publish)

## Existing Plans to Retain

The following plan documents from the previous session are still valid and should be updated to reflect the new priority ordering:

- `docs/plans/2026-03-16-001-feat-shell-completions-plan.md` — now PR 3 (was PR 1)
- `docs/plans/2026-03-16-002-feat-quiet-flag-agentic-polish-plan.md` — now PR 4 (was PR 2)
- `docs/plans/2026-03-16-003-feat-distribution-homebrew-crates-plan.md` — partially superseded; the distribution foundation is now PR 1, Homebrew is PR 2
