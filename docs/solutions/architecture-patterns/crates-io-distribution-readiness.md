---
title: "Crates.io distribution readiness: metadata, CI hardening, licensing, and docs rewrite"
category: architecture-patterns
date: 2026-03-16
tags:
  - distribution
  - crates-io
  - cargo-publish
  - cargo-binstall
  - dual-license
  - ci-hardening
  - supply-chain-security
  - cargo-deny
  - release-infrastructure
  - dead-code-removal
components:
  - Cargo.toml
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
  - LICENSE-MIT
  - LICENSE-APACHE
  - RELEASING.md
  - CHANGELOG.md
  - cliff.toml
  - deny.toml
  - README.md
  - docs/DEVELOPER.md
  - docs/SECRETS.md
  - src/transport.rs
  - src/doctor.rs
severity: high
resolution_time: "2-3 hours"
pr: 10
branch: feat/distribution-crates-io
---

# Crates.io distribution readiness: metadata, CI hardening, licensing, and docs rewrite

## Problem

Bird had a single distribution channel (GitHub Releases with raw binaries) and lacked the metadata, licensing, CI checks, and documentation needed for `cargo install bird` or `cargo binstall bird` to work correctly. Specifically:

1. **Cargo.toml was incomplete** -- missing `homepage`, `documentation`, `keywords`, `categories`, `authors`, `rust-version`, `exclude`, and `[package.metadata.binstall]`. The description was stale. The license was `"MIT"` but the existing Homebrew formula already declared `any_of: ["MIT", "Apache-2.0"]`.
2. **No CI safety net for publishing** -- no `cargo-deny` (license/advisory/ban audit), no `cargo publish --dry-run` (packaging validation), and GitHub Actions were pinned by mutable tags (supply-chain risk). The release workflow injected a now-dead `BIRD_DEFAULT_CLIENT_ID` secret.
3. **Documentation was severely stale** -- `docs/DEVELOPER.md` and `docs/SECRETS.md` referenced deleted files, removed constants, and a pre-xurl architecture. `README.md` described only 2 of 25+ commands.
4. **xurl install instructions were duplicated** across 5 locations with slightly different wording.
5. **No release infrastructure** -- no `RELEASING.md`, `CHANGELOG.md`, `cliff.toml`, or `deny.toml`.

## Solution

Five commits implementing Stage A of the distribution plan:

1. `build: update Cargo.toml metadata and add dual license`
2. `refactor: centralize xurl install instructions`
3. `docs: rewrite stale docs and update README for xurl architecture`
4. `ci: pin actions by SHA, add cargo-deny and package check`
5. `chore: add release infrastructure files`

## Key Implementation Details

### Cargo.toml metadata for crates.io

Complete `[package]` section with all fields for a quality crates.io listing:

```toml
license = "MIT OR Apache-2.0"
documentation = "https://docs.rs/bird"
keywords = ["twitter", "x", "api", "cli", "oauth"]
categories = ["command-line-utilities", "web-programming::http-client"]
rust-version = "1.87"
exclude = [
    ".claude/", ".github/", ".githooks/", "cliff.toml",
    "docs/", "openapi/", "rustfmt.toml", "scripts/", "tests/", "todos/",
]
```

Key exclude decisions:

- `docs/` wildcard excludes everything including files with 1Password vault paths
- `rust-toolchain.toml` intentionally NOT excluded (helps `cargo install` users)
- `Cargo.lock` intentionally included (binary crate convention)

### cargo-binstall metadata

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/bird-{ target }.tar.gz"
pkg-fmt = "tgz"
```

Enables `cargo binstall bird` to download pre-built binaries in seconds instead of compiling.

### Release profile optimization

```toml
[profile.release]
strip = true
lto = true
codegen-units = 1   # ~10-17% smaller binary
panic = "abort"      # removes unwinding machinery
```

### Dual license (MIT OR Apache-2.0)

Added `LICENSE-MIT` and `LICENSE-APACHE` files. Aligns with the Rust ecosystem convention, the existing Homebrew formula, and the xurl-rs reference implementation.

### CI hardening

**SHA-pinned actions** across all workflows:

```yaml
- uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
- uses: Swatinem/rust-cache@9d47c6ad4b02e050fd481d890b2ea34778fd09d6 # v2.7.8
```

Exception: `dtolnay/rust-toolchain@stable` uses a branch ref (pinning would freeze the toolchain version).

**New `audit` job** with `cargo-deny`:

```yaml
audit:
  strategy:
    matrix:
      checks: [advisories, "bans licenses sources"]
  continue-on-error: ${{ matrix.checks == 'advisories' }}
```

`advisories` uses `continue-on-error: true` because RustSec can have false positives. `bans licenses sources` is strict.

**New `package-check` job**: `cargo package --list` + `cargo publish --dry-run` on every PR.

### Centralized xurl install instructions

Single `pub const` in `transport.rs` replaces 5 duplicated strings:

```rust
pub const XURL_INSTALL_HINT: &str = "Install xurl: brew install xdevplatform/tap/xurl \
    (or download from https://github.com/xdevplatform/xurl/releases)";
```

Consumed by `resolve_xurl_path()`, `xurl_call()`, `xurl_passthrough()`, and `doctor.rs` (2 locations). Includes both Homebrew and GitHub Releases for `cargo install` users who may not have Homebrew.

### deny.toml configuration

License allowlist covers the actual dependency tree: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Unicode-DFS-2016. `multiple-versions = "warn"` avoids false build failures from transitive conflicts.

### Documentation rewrite

- **`docs/DEVELOPER.md`**: Rewritten from OAuth2 override guide to architecture-and-build guide. Documents xurl subprocess architecture, project layout, config reference, branching workflow.
- **`docs/SECRETS.md`**: Rewritten to document that bird has no API keys in its codebase (auth delegated to xurl). Lists the single GitHub Actions secret (`HOMEBREW_TAP_TOKEN`).
- **`README.md`**: Updated with 5 install methods, xurl prerequisite section, full 25+ command table, shell completions, dual license footer.

### Release workflow cleanup

Removed dead `BIRD_DEFAULT_CLIENT_ID` secret injection. Added `--locked` flag for reproducible builds. Added `Swatinem/rust-cache` with target-specific keys.

## Gotchas

1. **`dtolnay/rust-toolchain@stable` is NOT pinned by SHA** -- intentional. It's a thin wrapper that downloads from rustup; pinning would freeze the toolchain version.

2. **`continue-on-error` on advisories is essential** -- the RustSec DB can flag dependencies with no fix yet. Making it strict would block PRs on unactionable advisories.

3. **`Cargo.lock` must be committed for binary crates** -- Rust convention. Without it, `cargo install bird` resolves dependencies at install time with potentially different versions than CI tested.

4. **The `exclude` list must be maintained** when adding new top-level directories. The `cargo package --list` CI step makes omissions visible.

5. **First `cargo publish` must be manual** -- Trusted Publishing requires the crate to already exist on crates.io. Manual publish for v0.1.0, then configure Trusted Publishing.

6. **`BIRD_DEFAULT_CLIENT_ID` removal is safe** -- zero references to `option_env!` or `env!` for this constant anywhere in `src/`. Auth is fully delegated to xurl since PR #8.

7. **`docs/` excluded as a whole directory** rather than specific subdirectories. Simpler (matches xurl-rs pattern) and has the security benefit of excluding files with 1Password vault paths.

## Prevention

1. **`cargo publish --dry-run` runs on every PR** via `package-check` CI job -- catches missing files, invalid metadata, exclude mistakes.

2. **`cargo-deny` runs on every PR** via `audit` CI job -- catches incompatible licenses, security advisories, banned dependencies.

3. **SHA-pinned actions** with comment-annotated versions. Use Dependabot or Renovate to automate SHA update proposals.

4. **`RELEASING.md`** documents the complete release flow so any maintainer can cut a release.

5. **`XURL_INSTALL_HINT` constant** -- any new error path suggesting xurl installation should reference this constant, not duplicate the string.

6. **When adding new top-level directories**, check the `exclude` list and run `cargo package --list` locally.

## Related Solutions

- [xurl subprocess transport layer](xurl-subprocess-transport-layer.md) -- documents the transport layer that made `BIRD_DEFAULT_CLIENT_ID` removal possible; `XURL_INSTALL_HINT` constant lives in `transport.rs`
- [Security audit](../security-issues/rust-cli-security-code-quality-audit.md) -- earlier audit that established credential handling patterns
- [CI formatting drift](../build-errors/ci-formatting-drift-rust-edition-2024.md) -- CI patterns that the SHA-pinned actions build upon
- [Quiet flag diagnostic suppression](quiet-flag-diagnostic-suppression-pattern.md) -- the `diag!` macro references `XURL_INSTALL_HINT` in transport.rs
