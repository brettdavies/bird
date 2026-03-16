---
title: "feat: Distribution via Homebrew tap and crates.io"
type: feat
status: completed
date: 2026-03-16
deepened: 2026-03-16
origin: docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md
schedule: docs/plans/2026-03-16-000-meta-implementation-schedule.md
schedule_phase: "2 (Stage A) + 4 (Stage B)"
---

# feat: Distribution via Homebrew tap and crates.io

## Enhancement Summary

**Deepened on:** 2026-03-16 (round 2 — 11 parallel agents)
**Research agents used:** rust-tool-release skill, best-practices-researcher (Homebrew taps, crates.io publishing), framework-docs-researcher (GitHub Actions release patterns), architecture-strategist, security-sentinel, code-simplicity-reviewer, pattern-recognition-specialist, spec-flow-analyzer, performance-oracle, learnings-researcher, git-history-analyzer

### Key Improvements (Round 2)

1. **License resolved:** Change to `MIT OR Apache-2.0` (dual license) per rust-tool-release skill standard, alignment notes, and existing formula — add `LICENSE-MIT` + `LICENSE-APACHE`, not a single `LICENSE` file
2. **5-target build matrix:** Add `aarch64-unknown-linux-gnu` (Linux ARM, Tier 1 since Homebrew 5.0.0) and `x86_64-apple-darwin` (macOS Intel, Tier 1 until Sept 2026) to match xurl-rs reference implementation
3. **Phase ordering fixed:** Split into Stage A (crates.io readiness — no dependencies) and Stage B (Homebrew tap — depends on PR 3 completions). Removed circular dependency
4. **Security hardening:** Pin GitHub Actions by SHA, add `docs/notes/` to exclude list (also contains 1Password paths), ad-hoc codesign macOS binaries, add version input validation to tap workflow
5. **Missing infrastructure:** Add RELEASING.md, CHANGELOG.md, cliff.toml, deny.toml, cargo audit + cargo deny to CI, cargo publish --dry-run on every PR
6. **Performance:** Add Swatinem/rust-cache to release workflow, add `codegen-units = 1` + `panic = "abort"` to release profile, add cargo-binstall metadata
7. **Dispatch mechanism aligned:** Use `repository_dispatch` to match the existing `homebrew-tap` automation plan and xurl-rs pattern (not `workflow_dispatch` as previously proposed)
8. **bird doctor unblocked:** Must exempt `doctor` and `completions` from xurl fail-fast check before crates.io publish (currently all commands fail if xurl is missing)

### Critical Decisions Required Before Implementation

1. **License:** Plan now proposes `MIT OR Apache-2.0` — confirm this is the intended direction
2. **Trusted Publishing vs CARGO_REGISTRY_TOKEN:** Agents disagree — security says Trusted Publishing (more secure, no long-lived secrets); simplicity says CARGO_REGISTRY_TOKEN (simpler); skill standard says CARGO_REGISTRY_TOKEN. Recommend Trusted Publishing with skill update
3. **Dispatch mechanism:** homebrew-tap repo already has a `repository_dispatch`-based `update-formula.yml` plan. Bird plan previously proposed `workflow_dispatch` + `bump-formula.yml`. Recommend aligning with the existing tap plan
4. **macOS Intel target:** Simplicity reviewer says YAGNI; skill requires 5 targets; Homebrew research says Tier 1 until Sept 2026. Recommend including per skill standard

### New Considerations Discovered (Round 2)

- **CRITICAL: `bird doctor` is blocked by xurl fail-fast** — `main.rs:795-800` exits with code 78 for ALL commands when xurl is missing, including `doctor`. `cargo install` users without xurl cannot run the diagnostic tool. Must fix before crates.io publish
- **RESOLVED: Formula type aligned with homebrew-tap SOT** — all brettdavies tools use source-build formulas, compatible with the generic `update-formula.yml` (two unconditional `sed` commands). See `~/.claude/skills/homebrew-tap-publish/references/alternative-approaches.md` for rationale
- **SECURITY: `docs/notes/bird-alignment.md` also contains 1Password vault paths** — line 179 reveals vault name and item name. Must add `docs/notes/` to exclude list
- **SECURITY: All GitHub Actions pinned by mutable tag, not SHA** — supply-chain risk (cf. tj-actions/changed-files March 2025 incident)
- Linux ARM64 (`aarch64-unknown-linux-gnu`) is now Homebrew Tier 1 (Homebrew 5.0.0, Nov 2025) — add to formula
- macOS Intel drops to Tier 3 in September 2026 — include now, plan to drop later
- GitHub Releases API now exposes SHA256 digests natively (June 2025) — future optimization
- `cargo-binstall` support via `[package.metadata.binstall]` gives users fast pre-built binary download through cargo ecosystem
- `clap_complete` should NOT be added in this plan (belongs to PR 3) — YAGNI violation
- PR 4 (quiet flag) dependency is unnecessary — README can be updated in a follow-up release
- `documentation` field should change from GitHub URL to `https://docs.rs/bird`
- `rust-toolchain.toml` should NOT be excluded from crate (helps cargo install users)
- `rustfmt.toml` SHOULD be excluded from crate (developer-only)
- `Cargo.lock` is intentionally included in crate (binary crate convention for reproducible builds)
- Ad-hoc codesigning (`codesign -s -`) should be added to macOS build steps in CI

## Overview

Publish bird to crates.io, automate Homebrew formula updates on version tag pushes, and add shell completions to the existing source-build formula. This covers PR 1 (publication foundation) and PR 2 (Homebrew tap automation) of 4 from the distribution/DX brainstorm (see brainstorm: `docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md`).

## Problem Statement / Motivation

Bird currently has one distribution channel: GitHub Releases with raw binaries. Users must manually download, extract, and place the binary on PATH. There is no `brew install`, no `cargo install`, and no automatic completion installation. This limits discoverability and adoption.

## Existing State

**The `brettdavies/homebrew-tap` repo already exists** at `~/dev/homebrew-tap/` with two formulas:

- **`Formula/bird.rb`** — builds from source (`depends_on "rust" => :build`, `cargo install`), uses source archive URL, dual license `any_of: ["MIT", "Apache-2.0"]`
- **`Formula/xurl-rs.rb`** — the Rust port of xurl, installs as `xr` binary, also builds from source, already generates completions via `generate_completions_from_executable`

**Important distinction**: Bird depends on the `xurl` binary (from `xdevplatform/tap/xurl` cask), NOT on `xr` (from `xurl-rs`). These are separate projects.

**This PR should update the existing `bird.rb` formula** to add sha256, shell completions, and caveats while keeping the source-build approach.

**The `brettdavies/homebrew-tap` repo also has an existing automation plan** at `~/dev/homebrew-tap/docs/plans/2026-03-16-automated-formula-updates-plan.md`. That plan defines a generic `update-formula.yml` workflow using `repository_dispatch` with a `client_payload` containing `formula`, `version`, and `repo` fields. Bird's dispatch step must align with this canonical pattern.

**Files confirmed missing from bird repo:** `LICENSE-MIT`, `LICENSE-APACHE`, `RELEASING.md`, `CHANGELOG.md`, `AGENTS.md`, `cliff.toml`, `deny.toml`. All required by the rust-tool-release skill.

**Verified: `BIRD_DEFAULT_CLIENT_ID` is dead code.** Zero references to `option_env!`, `env!`, or `BIRD_DEFAULT_CLIENT_ID` in any `src/` file. Auth is fully delegated to xurl via `transport.rs`.

**Verified: `docs/SECRETS.md` and `docs/DEVELOPER.md` are severely stale.** Both reference deleted files (`auth.rs`, `login.rs`) and removed constants (`OAUTH2_CLIENT_ID_DEV`). `DEVELOPER.md` needs a substantial rewrite, not just cleanup.

## Proposed Solution

### Four distribution channels (one existing, three new)

| Channel | Command | Audience | Completions | Status |
|---------|---------|----------|-------------|--------|
| Homebrew tap | `brew install brettdavies/tap/bird` | macOS/Linux users | Via `generate_completions_from_executable` | Exists (source-build, keep) |
| crates.io | `cargo install bird` | Rust developers | Manual via `bird completions <shell>` | New |
| cargo-binstall | `cargo binstall bird` | Rust developers (fast) | Manual | New (requires `[package.metadata.binstall]`) |
| GitHub Releases | Download `.tar.gz` from releases page | Everyone else | Included in archive | Exists (raw binary) -> upgrade to archives |

### License alignment

**Decision:** Change to `MIT OR Apache-2.0` (dual license) to match:

- The rust-tool-release skill standard (`license = "MIT OR Apache-2.0"`)
- The alignment notes (`docs/notes/bird-alignment.md` lines 24-33)
- The existing `bird.rb` formula (`any_of: ["MIT", "Apache-2.0"]`)
- The xurl-rs reference implementation

**Actions:**

1. Change `Cargo.toml` `license` field from `"MIT"` to `"MIT OR Apache-2.0"`
2. Add `LICENSE-MIT` and `LICENSE-APACHE` files (copy from `~/dev/xurl-rs/`)
3. Keep existing formula's `license any_of: ["MIT", "Apache-2.0"]`
4. Change `documentation` field to `"https://docs.rs/bird"`

### Homebrew formula design

Keep the existing **source-build formula** pattern (aligned with the homebrew-tap
SOT decision — all brettdavies tools use source-build formulas). Add shell
completions and caveats. For why source-build was chosen over pre-built binaries,
see `~/.claude/skills/homebrew-tap-publish/references/alternative-approaches.md`.

```ruby
class Bird < Formula
  desc "X API CLI with entity caching, search, threads, and watchlists"
  homepage "https://github.com/brettdavies/bird"
  url "https://github.com/brettdavies/bird/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "<computed-hash>"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/brettdavies/bird.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    generate_completions_from_executable(bin/"bird", "completions")
  end

  def caveats
    <<~EOS
      bird requires xurl for X API authentication.
      Install it with:
        brew install xdevplatform/tap/xurl

      Verify your setup with:
        bird doctor
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/bird --version")
  end
end
```

**Key design decisions:**

- Source-build formula with single URL + SHA256 (compatible with generic `update-formula.yml`)
- No explicit `version` field — Homebrew auto-detects from the URL pattern
- `head` block for `brew install --HEAD` support
- Shell completions via `generate_completions_from_executable` (requires PR 3 completions subcommand)
- `caveats` block guides users to install xurl dependency
- Automated updates work identically to xurl-rs (two unconditional `sed` commands)

### crates.io publishing

- Crate name: `bird` (confirmed available — 404 on crates.io API as of 2026-03-16)
- `cargo install bird` compiles from source; user must install xurl separately
- `bird doctor` validates xurl installation and provides guidance — **but must be unblocked from xurl fail-fast first**
- Trusted Publishing (OIDC) for automated publishes after manual v0.1.0

### Release automation

**Pipeline:** `check-version` + `audit` -> `build` (5-target matrix) -> `publish-crate` -> `release` (GitHub Release) -> `trigger-tap-update`

- `cargo publish` runs BEFORE GitHub Release creation (if it fails, no release is advertised)
- Cross-repository trigger via `repository_dispatch` to align with existing homebrew-tap automation plan
- Tap-side workflow downloads source tarball, computes SHA256, updates formula via two unconditional `sed` commands (url + sha256)

### 5-target build matrix (per rust-tool-release skill)

| Target | Runner | Method | Homebrew? |
|--------|--------|--------|-----------|
| `x86_64-unknown-linux-gnu` | ubuntu-22.04 | native | Yes |
| `aarch64-unknown-linux-gnu` | ubuntu-22.04 | cross | Yes |
| `aarch64-apple-darwin` | macos-14 | native | Yes |
| `x86_64-apple-darwin` | macos-13 | native | Yes |
| `x86_64-pc-windows-msvc` | windows-latest | native | No (GitHub Releases only) |

**Performance notes:**

- Add `Swatinem/rust-cache@v2` with target-specific key to release workflow (currently missing — saves 1-3 min/target)
- Use `--locked` flag for reproducible builds
- Ad-hoc codesign macOS binaries: `codesign --force --sign - bird` (eliminates quarantine without Apple Developer ID)
- Cross-compiled `aarch64-unknown-linux-gnu` via `cross` is the bottleneck (~8-15 min). Consider `ubuntu-22.04-arm` native runners if available

## Technical Considerations

### Cargo.toml changes

**Changes to `[package]`:**

```toml
license = "MIT OR Apache-2.0"   # was "MIT"
documentation = "https://docs.rs/bird"  # was GitHub URL
```

**Updated `exclude` list** (replace granular `docs/` subdirs with `docs/` wildcard per xurl-rs pattern):

```toml
exclude = [
    ".claude/",
    ".github/",
    ".githooks/",
    "cliff.toml",
    "docs/",
    "openapi/",
    "rustfmt.toml",
    "scripts/",
    "tests/",
    "todos/",
]
```

Notes:

- `docs/` excludes ALL docs including `SECRETS.md`, `DEVELOPER.md`, and `docs/notes/bird-alignment.md` (which also contains 1Password vault paths at line 179)
- `rustfmt.toml` excluded (developer-only, no effect on cargo install)
- `rust-toolchain.toml` intentionally NOT excluded (helps cargo install users with correct toolchain)
- `Cargo.lock` intentionally included (binary crate convention for reproducible builds)
- `cliff.toml` excluded (developer-only changelog config)

**Add `[package.metadata.binstall]`:**

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/bird-{ target }.tar.gz"
pkg-fmt = "tgz"
```

This lets users with `cargo-binstall` run `cargo binstall bird` and get the pre-compiled binary in seconds instead of compiling for 2-4 minutes.

**Add to `[profile.release]`:**

```toml
[profile.release]
strip = true
lto = true
codegen-units = 1  # better optimization, ~10-17% smaller binary
panic = "abort"    # removes unwinding machinery, appropriate for CLI
```

Estimated binary size reduction: 4.6 MB -> ~3.8-4.2 MB. Archive size: ~1.5-1.9 MB compressed.

### Required files to create

| File | Source | Purpose |
|------|--------|---------|
| `LICENSE-MIT` | Copy from `~/dev/xurl-rs/` | MIT license text |
| `LICENSE-APACHE` | Copy from `~/dev/xurl-rs/` | Apache 2.0 license text |
| `RELEASING.md` | Follow xurl-rs pattern | Release process documentation |
| `CHANGELOG.md` | Generate with `git cliff --init` | Auto-generated changelog |
| `cliff.toml` | Copy from `~/dev/xurl-rs/`, adapt | git-cliff configuration |
| `deny.toml` | Generate with `cargo deny init` | License/advisory/ban checking |

### bird doctor: unblock from xurl fail-fast

**Blocker for crates.io publish.** Currently `main.rs:795-800` has an unconditional xurl fail-fast check that blocks ALL commands, including `doctor`. A user who runs `cargo install bird` without xurl finds that `bird doctor` exits with code 78 and cannot help them diagnose the issue.

**Fix:** Exempt `doctor` (and `completions` for PR 3) from the xurl path resolution check. Move the xurl check to after command parsing, skip it for subcommands that don't need xurl.

This fix is small enough to include in Stage A of this plan rather than deferring to PR 3.

### Trusted Publishing for crates.io

**Setup steps (after manual v0.1.0 publish):**

1. Go to `https://crates.io/settings/tokens/trusted-publishing`
2. Add trusted publisher: owner=`brettdavies`, repo=`bird`, workflow=`release.yml`
3. Workflow must declare `permissions: id-token: write`
4. Use `rust-lang/crates-io-auth-action@v1` to exchange OIDC token for 30-minute access token
5. Token is automatically revoked when job completes
6. Enable "Enforce Trusted Publishing" to disable API token publishing entirely

**First publish must be manual** (chicken-and-egg: Trusted Publishing requires the crate to exist first).

### Security hardening

**Pin all GitHub Actions by SHA** (supply-chain risk mitigation):

```yaml
- uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
- uses: dtolnay/rust-toolchain@stable  # exception: uses branch, not tag
- uses: Swatinem/rust-cache@9d47c6ad4b02e050fd481d890b2ea34778fd09d6 # v2.7.8
- uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2
- uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4.3.0
- uses: softprops/action-gh-release@da05d552573ad5aba039eaac05058a918a7bf631 # v2.2.2
```

Use Dependabot or Renovate to propose SHA updates automatically.

**Add version input validation to tap workflow:**

```yaml
- name: Validate version input
  run: |
    if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "::error::Invalid version format"
      exit 1
    fi
```

**Ad-hoc codesign macOS binaries in CI:**

```yaml
- name: Ad-hoc codesign (macOS only)
  if: runner.os == 'macOS'
  run: |
    codesign --force --sign - "target/${{ matrix.target }}/release/bird"
    codesign --verify --verbose "target/${{ matrix.target }}/release/bird"
```

### CI additions

**Add to `ci.yml`:**

1. **`cargo-deny`** (license + advisory + ban checking):

   ```yaml
   audit:
     runs-on: ubuntu-22.04
     strategy:
       matrix:
         checks: [advisories, "bans licenses sources"]
     continue-on-error: ${{ matrix.checks == 'advisories' }}
     steps:
       - uses: actions/checkout@v4
       - uses: EmbarkStudios/cargo-deny-action@v2
         with:
           command: check ${{ matrix.checks }}
   ```

2. **`cargo publish --dry-run`** (catch packaging issues on every PR):

   ```yaml
   package-check:
     runs-on: ubuntu-22.04
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
       - run: cargo package --list
       - run: cargo publish --dry-run
   ```

3. **Changelog generation on main push** (via `orhun/git-cliff-action@v4`):

   ```yaml
   changelog:
     if: github.ref == 'refs/heads/main'
     runs-on: ubuntu-22.04
     steps:
       - uses: actions/checkout@v4
         with: { fetch-depth: 0 }
       - uses: orhun/git-cliff-action@v4
         with: { config: cliff.toml }
   ```

### Cross-repository automation

**Align with existing homebrew-tap plan.** The `brettdavies/homebrew-tap` repo already defines a `repository_dispatch`-based `update-formula.yml` workflow (at `~/dev/homebrew-tap/docs/plans/2026-03-16-automated-formula-updates-plan.md`). The bird dispatch step uses `gh api` (pre-installed on all GitHub-hosted runners):

```yaml
  homebrew:
    needs: release
    runs-on: ubuntu-latest
    steps:
      - name: Dispatch Homebrew formula update
        env:
          GH_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
          VERSION: ${{ github.ref_name }}
        run: |
          gh api repos/brettdavies/homebrew-tap/dispatches \
            --method POST \
            -f event_type=update-formula \
            -f 'client_payload[formula]=bird' \
            -f "client_payload[version]=${VERSION#v}" \
            -f 'client_payload[repo]=brettdavies/bird'
```

**Why `gh api` over raw `curl`:** Auth handled via `GH_TOKEN` env var, bracket notation creates nested JSON, returns exit code 1 on HTTP errors with error body on stdout. `${VERSION#v}` strips the `v` prefix.

**No tap-side changes needed.** The source-build formula uses a single URL + SHA256, which the generic `update-formula.yml` handles with two unconditional `sed` commands — identical to xurl-rs.

### Version synchronization

Add as a gating job in `release.yml` (runs BEFORE build matrix):

```yaml
check-version:
  runs-on: ubuntu-22.04
  steps:
    - uses: actions/checkout@v4
    - name: Verify tag matches Cargo.toml version
      run: |
        TAG_VERSION="${GITHUB_REF_NAME#v}"
        CARGO_VERSION=$(cargo pkgid | sed 's/.*@//')
        if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
          echo "::error::Tag ${GITHUB_REF_NAME} does not match Cargo.toml version ${CARGO_VERSION}"
          exit 1
        fi
```

Uses `cargo pkgid` (zero extra dependencies) instead of `cargo metadata` + `jaq`.

### Archive creation in release workflow

Currently `release.yml` uploads raw binaries. Switch to `.tar.gz` archives containing binary + LICENSE for the GitHub Releases and cargo-binstall channels:

```text
bird-aarch64-apple-darwin/
  bird
  LICENSE-MIT
  LICENSE-APACHE
  README.md
```

**Note:** Shell completions are NOT bundled in archives. The Homebrew formula uses `generate_completions_from_executable` (source-build), and `cargo install` users generate completions via `bird completions <shell>`. This eliminates the cross-compilation completions problem entirely — no `build.rs` completion generation needed for this plan.

### OAuth2 client_id and crates.io — RESOLVED

**Finding: `BIRD_DEFAULT_CLIENT_ID` is dead code.** Zero references in `src/` to `BIRD_DEFAULT_CLIENT_ID`, `option_env!`, or `env!`. Confirmed by all agents.

**Actions for this PR:**

1. Remove `BIRD_DEFAULT_CLIENT_ID` env var from `release.yml` line 39-41
2. Delete the `BIRD_DEFAULT_CLIENT_ID` GitHub Actions secret
3. Rewrite `docs/SECRETS.md` and `docs/DEVELOPER.md` to reflect xurl transport architecture (these are severely stale — nearly every section references deleted code)
4. Remove `BIRD_DEFAULT_CLIENT_ID` secret instruction from `README.md` line 129

### DRY xurl install instructions

xurl install instructions appear in 4 source locations plus the Homebrew formula caveats. Centralize into a single constant:

- `transport.rs:67` — resolve_xurl_path error
- `transport.rs:159` — xurl_call NotFound error
- `doctor.rs:105` — build_commands_section
- `doctor.rs:215` — format_pretty guidance

Include a generic URL fallback: `"Install xurl: brew install xdevplatform/tap/xurl (or download from https://github.com/xdevplatform/xurl/releases)"`. This survives xurl tap removal.

## Acceptance Criteria

### Stage A: crates.io readiness (no external dependencies)

- [ ] `Cargo.toml` license changed to `"MIT OR Apache-2.0"`
- [ ] `LICENSE-MIT` and `LICENSE-APACHE` added to repo root
- [ ] `documentation` field changed to `"https://docs.rs/bird"`
- [ ] `docs/` excluded from crate (replaces granular subdirs), `rustfmt.toml` + `cliff.toml` excluded
- [ ] `[package.metadata.binstall]` added to Cargo.toml
- [ ] `[profile.release]` updated: `codegen-units = 1`, `panic = "abort"`
- [ ] `BIRD_DEFAULT_CLIENT_ID` removed from `release.yml`
- [ ] `docs/SECRETS.md` and `docs/DEVELOPER.md` rewritten for xurl transport architecture
- [ ] `README.md` updated: all 4 install methods documented, stale secret instructions removed
- [ ] `bird doctor` exempted from xurl fail-fast (can run without xurl installed)
- [ ] xurl install instructions centralized into single constant with generic URL fallback
- [ ] `RELEASING.md`, `CHANGELOG.md`, `cliff.toml`, `deny.toml` created
- [ ] GitHub Actions pinned by SHA in `release.yml` and `ci.yml`
- [ ] `cargo audit` / `cargo deny` added to CI
- [ ] `cargo publish --dry-run` + `cargo package --list` added to CI
- [ ] Zero `unwrap()` in production code (verify before publish)
- [ ] `cargo publish --dry-run` succeeds locally
- [ ] `cargo package --list` shows no sensitive files

### Stage B: Homebrew tap + release automation (depends on PR 3 completions)

- [x] `release.yml` expanded to 5-target matrix with `cross` for ARM Linux
- [x] Release archives created as `.tar.gz` with binary + LICENSE + README
- [x] macOS binaries ad-hoc codesigned in CI
- [x] Version tag / Cargo.toml gating check added to release workflow
- [x] `cargo publish` job added to release workflow (Trusted Publishing after manual v0.1.0)
- [x] Changelog generated via git-cliff in release workflow
- [x] `repository_dispatch` trigger added to release workflow via `gh api` (aligned with tap plan)
- [x] `Formula/bird.rb` updated with sha256, `generate_completions_from_executable`, and caveats
- [ ] `brew install brettdavies/tap/bird` compiles from source and installs bird binary to PATH
- [ ] `brew install` displays caveats (xurl dependency)
- [ ] Bash, zsh, fish completions installed to standard Homebrew paths via `generate_completions_from_executable`
- [ ] `bird --version` output matches formula version
- [ ] `bird doctor` reports healthy status after Homebrew install
- [ ] SHA256 hash verified for source tarball
- [x] HOMEBREW_TAP_TOKEN PAT created and stored; rotation documented in RELEASING.md

### crates.io

- [ ] Manual `cargo publish` v0.1.0 succeeds
- [ ] Trusted Publishing configured on crates.io (after v0.1.0)
- [ ] `cargo install bird` compiles and installs successfully on clean system
- [ ] `cargo binstall bird` downloads pre-built binary from GitHub Releases
- [ ] `bird doctor` provides clear guidance when xurl is missing

## Implementation Plan

### Stage A: crates.io readiness (in bird repo — ships independently)

No dependencies on PR 3 or PR 4. Can ship immediately.

1. Change `Cargo.toml` license to `"MIT OR Apache-2.0"`, documentation to `"https://docs.rs/bird"`.
2. Add `LICENSE-MIT` and `LICENSE-APACHE` (copy from xurl-rs).
3. Replace granular `exclude` entries with `"docs/"`, add `"rustfmt.toml"`, `"cliff.toml"`.
4. Add `[package.metadata.binstall]` section.
5. Update `[profile.release]`: add `codegen-units = 1`, `panic = "abort"`.
6. Remove `BIRD_DEFAULT_CLIENT_ID` from `release.yml` (dead code).
7. Exempt `doctor` (and `completions`) from xurl fail-fast in `main.rs`.
8. Centralize xurl install instructions into single constant with fallback URL.
9. Rewrite `docs/SECRETS.md` and `docs/DEVELOPER.md` for xurl transport architecture.
10. Update `README.md`: add all 4 install methods, remove stale secret instructions.
11. Create `RELEASING.md`, `CHANGELOG.md`, `cliff.toml`, `deny.toml`.
12. Pin all GitHub Actions by SHA in `ci.yml` and `release.yml`.
13. Add `cargo-deny`, `cargo publish --dry-run`, and changelog generation to `ci.yml`.
14. Verify: `cargo publish --dry-run`, `cargo package --list`, zero `unwrap()` in production code.
15. Manual `cargo publish` for v0.1.0. Configure Trusted Publishing immediately after.

### Stage B: Homebrew tap + release automation (ships after PR 3 completions)

Depends on PR 3 for: `bird completions <shell>` subcommand (used by `generate_completions_from_executable` in the formula).

1. Expand `release.yml` to 5-target matrix (add `aarch64-unknown-linux-gnu` via `cross`, `x86_64-apple-darwin` via `macos-13`).
2. Add `Swatinem/rust-cache@v2` to release workflow with target-specific keys.
3. Add archive creation step (tar.gz with binary + LICENSE + README).
4. Add ad-hoc codesigning for macOS targets.
5. Add `check-version` gating job.
6. Add `publish-crate` job with Trusted Publishing.
7. Add changelog generation via git-cliff in release job.
8. Add `homebrew` job with `gh api` dispatch for homebrew-tap (aligned with existing tap plan and xurl-rs pattern).
9. Create HOMEBREW_TAP_TOKEN PAT, store as secret, document rotation in RELEASING.md.
10. Update `Formula/bird.rb` with sha256, `generate_completions_from_executable`, and caveats block.
11. Compute SHA256 for current source tarball, verify `brew install` works.
12. Test full pipeline: tag push -> build -> publish -> release -> tap update.

## Dependencies & Risks

### Resolved dependencies

- ~~**Depends on PR 4 (quiet flag)**~~: Removed. README can be updated in a follow-up release.
- ~~**OAuth2 client_id**~~: `BIRD_DEFAULT_CLIENT_ID` is dead code. Safe to remove.

### Active dependencies

- **Stage B depends on PR 3 (completions)**: The source-build formula uses `generate_completions_from_executable(bin/"bird", "completions")`, which requires the `bird completions <shell>` subcommand from PR 3. Stage A (crates.io) is fully independent.
- **Crate name `bird`**: Confirmed available (404 on crates.io API, 2026-03-16). Publish promptly in Stage A to claim.
- **`HOMEBREW_TAP_TOKEN`**: Fine-grained PAT. Max expiration 1 year. Set max expiration, document rotation in RELEASING.md. If it expires, the release workflow succeeds but the tap update fails — GitHub sends a failure notification email.
- **xurl cask dependency**: Relies on `caveats` + `bird doctor` for user guidance. Fallback URL in error messages survives tap removal.
- **Windows xurl support**: Unverified. If xurl does not support Windows, `cargo install bird` on Windows is unusable. Verify before publishing. Document limitation if confirmed.
- **~~Pre-built formula incompatible with generic `update-formula.yml`~~**: RESOLVED. All tools now use source-build formulas, compatible with the generic `update-formula.yml` out of the box.

### Risk mitigations

- **PAT expiration**: The tap update step should NOT use `continue-on-error`. If it fails, the release job fails, triggering GitHub's notification email. This is the monitoring mechanism — no separate cron job needed.
- **Failed cargo publish**: `publish-crate` runs before `release`. If it fails, no GitHub Release is created and no tap update is triggered. Recovery: fix the issue, delete/re-create the tag.
- **Package size**: `openapi/x-api-openapi.json` is excluded via `docs/` wildcard. Verify with `cargo package --list`.
- **Binary reproducibility**: Pin `rust-toolchain.toml`, use `--locked` flag, `codegen-units = 1` for deterministic builds.

## Release workflow structure

```text
check-version ──┐
                 ├──> build (5 targets) ──> publish-crate ──> release ──> trigger-tap-update
audit ───────────┘
```

Key ordering: `cargo publish` BEFORE GitHub Release creation. If publish fails, no release is advertised.

## Sources & References

- **Origin brainstorm:** [docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md](docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md)
- **Alignment notes:** [docs/notes/bird-alignment.md](docs/notes/bird-alignment.md) — dual license decision, 5 targets, dispatch pattern
- **Homebrew-tap automation plan:** [homebrew-tap/docs/plans/2026-03-16-automated-formula-updates-plan.md](https://github.com/brettdavies/homebrew-tap/blob/main/docs/plans/2026-03-16-automated-formula-updates-plan.md) — canonical dispatch pattern
- **Homebrew-tap automation solution:** [homebrew-tap/docs/solutions/integration-issues/homebrew-tap-automated-formula-updates-via-dispatch.md](https://github.com/brettdavies/homebrew-tap/blob/main/docs/solutions/integration-issues/homebrew-tap-automated-formula-updates-via-dispatch.md) — compounded learnings: expression injection, brew CI pitfalls, troubleshooting
- **rust-tool-release skill:** `~/.claude/skills/rust-tool-release/SKILL.md` — release standard (source-build formula only, aligned with homebrew-tap SOT)
- **xurl-rs reference:** `~/dev/xurl-rs/` — Cargo.toml, release.yml, RELEASING.md, cliff.toml patterns
- Homebrew Formula Cookbook: https://docs.brew.sh/Formula-Cookbook
- Homebrew Support Tiers: https://docs.brew.sh/Support-Tiers (macOS Intel -> Tier 3 Sept 2026)
- Homebrew 5.0.0: https://brew.sh/2025/11/12/homebrew-5.0.0/ (Linux ARM64 Tier 1)
- crates.io Trusted Publishing: https://crates.io/docs/trusted-publishing
- crates.io development update (Jan 2026): https://blog.rust-lang.org/2026/01/21/crates-io-development-update/
- rust-lang/crates-io-auth-action: https://github.com/rust-lang/crates-io-auth-action
- RFC 3691 Trusted Publishing: https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html
- GitHub Releases SHA256 digests (June 2025): https://github.blog/changelog/2025-06-03-releases-now-expose-digests-for-release-assets/
- Fine-grained PATs GA (March 2025): https://github.blog/changelog/2025-03-18-fine-grained-pats-are-now-generally-available/
- Orhun's automated Rust releases: https://blog.orhun.dev/automated-rust-releases/
- EmbarkStudios/cargo-deny: https://github.com/EmbarkStudios/cargo-deny
- cargo-binstall: https://github.com/cargo-bins/cargo-binstall
- cross-rs/cross: https://github.com/cross-rs/cross
- Cargo publishing docs: https://doc.rust-lang.org/cargo/reference/publishing.html
