---
title: "Automated Release Pipeline for Rust CLI Tools"
category: architecture-patterns
date: 2026-03-16
tags: [ci-cd, github-actions, release-automation, cross-compilation, homebrew, crates-io, codesigning, git-cliff, trusted-publishing]
applies_to: [release.yml, ci.yml, guard-main-docs.yml, homebrew-tap, RELEASING.md]
confidence: high
---

# Automated Release Pipeline for Rust CLI Tools

## Problem

The bird CLI had a minimal release workflow with only 3 targets (linux x86_64, macOS aarch64, windows x86_64) that uploaded raw binaries to GitHub Releases. There was no crates.io publishing, no Homebrew tap automation, no version verification gate, no security auditing in the release pipeline, and no changelog generation. The CI workflows used outdated action SHAs and lacked the `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` env for the upcoming Node.js 20 deprecation (June 2026).

## Solution

Expanded `release.yml` from a 3-target binary upload into a 6-job pipeline with 5 build targets, crates.io publishing, changelog generation, and cross-repository Homebrew tap dispatch. Updated all 3 workflow files with latest SHA-pinned actions and Node.js 24 opt-in.

### Pipeline structure

```text
check-version + audit -> build (5 targets) -> publish-crate -> release -> homebrew
```

Ordering is intentional: `cargo publish` runs BEFORE GitHub Release creation. If publish fails, no release is advertised and no Homebrew update triggers.

## Key Implementation Details

### Version check gate

Uses `cargo pkgid` (zero extra dependencies) instead of `cargo metadata` + jq:

```yaml
- name: Verify tag matches Cargo.toml version
  run: |
    TAG_VERSION="${GITHUB_REF_NAME#v}"
    CARGO_VERSION=$(cargo pkgid | sed 's/.*#//')
    if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
      echo "::error::Tag ${GITHUB_REF_NAME} does not match Cargo.toml version ${CARGO_VERSION}"
      exit 1
    fi
```

### 5-target build matrix with cross-compilation toggle

The `matrix.cross` boolean controls tool selection via a single expression:

```yaml
matrix:
  include:
    - { os: ubuntu-22.04, target: x86_64-unknown-linux-gnu, artifact: bird }
    - { os: ubuntu-22.04, target: aarch64-unknown-linux-gnu, artifact: bird, cross: true }
    - { os: macos-14, target: aarch64-apple-darwin, artifact: bird }
    - { os: macos-latest, target: x86_64-apple-darwin, artifact: bird }
    - { os: windows-latest, target: x86_64-pc-windows-msvc, artifact: bird.exe }
```

Build command swaps `cross` for `cargo` based on the boolean:

```yaml
run: ${{ matrix.cross && 'cross' || 'cargo' }} build --release --locked --target ${{ matrix.target }}
```

`fail-fast: false` is critical -- without it, one failed target cancels all builds and downstream jobs get skipped.

### Cross-platform archive creation

Uses `shell: bash` to work on all platforms including Windows. Windows uses `7z` for `.zip`; others use `tar` for `.tar.gz`:

```yaml
- name: Create archive
  shell: bash
  run: |
    STAGING="bird-${{ matrix.target }}"
    mkdir -p "$STAGING"
    cp "target/${{ matrix.target }}/release/${{ matrix.artifact }}" "$STAGING/"
    cp LICENSE-MIT LICENSE-APACHE README.md "$STAGING/"
    cp -r completions "$STAGING/"
    if [[ "${{ matrix.target }}" == *windows* ]]; then
      7z a "${STAGING}.zip" "$STAGING"
    else
      tar czf "${STAGING}.tar.gz" "$STAGING"
    fi
```

### macOS ad-hoc codesigning

Eliminates Gatekeeper quarantine without requiring an Apple Developer ID:

```yaml
- name: Ad-hoc codesign (macOS)
  if: runner.os == 'macOS'
  run: |
    codesign --force --sign - "target/${{ matrix.target }}/release/bird"
    codesign --verify --verbose "target/${{ matrix.target }}/release/bird"
```

### Homebrew tap dispatch via `gh api`

Uses bracket notation for nested JSON payload, `${VERSION#v}` strips the `v` prefix:

```yaml
run: |
  gh api repos/brettdavies/homebrew-tap/dispatches \
    --method POST \
    -f event_type=update-formula \
    -f 'client_payload[formula]=bird' \
    -f "client_payload[version]=${VERSION#v}" \
    -f 'client_payload[repo]=brettdavies/bird'
```

### Trusted Publishing (OIDC)

The `publish-crate` job uses `rust-lang/crates-io-auth-action` to
exchange a GitHub OIDC token for a crates.io publish credential — no
long-lived `CARGO_REGISTRY_TOKEN` secret needed. Trusted Publishing is
configured and enforced on crates.io for this crate.

**Bootstrap note:** The first publish of any new crate requires a
manual `cargo publish` with `CARGO_REGISTRY_TOKEN` because Trusted
Publishing can only be configured on a crate that already exists.

## Prevention Strategies

### Version drift

The `check-version` job catches tag-vs-Cargo.toml mismatches, but a stale `Cargo.lock` will cause the build to fail *after* the version check passes (because `--locked` is used), wasting CI minutes across all 5 matrix targets. Consider adding `cargo check --locked` to the `check-version` job to fail fast in one job instead of five.

### Crate publish is not idempotent

`cargo publish` will fail if the version already exists on crates.io. If a release pipeline partially succeeds (publish-crate passes, release job fails), rerunning the workflow fails at `publish-crate`. Recovery: the crate is already published; manually trigger from the `release` job onward.

### Cross-compilation fragility

The `aarch64-unknown-linux-gnu` target uses `cross` which runs builds inside Docker containers. The `rusqlite` dependency with `bundled` feature compiles SQLite from C source. Cross-compilation of C dependencies is the most fragile part of this matrix. Consider pinning the `cross` version (`cargo install cross --version X.Y.Z --locked`).

### SHA-pinned action staleness

Actions are correctly pinned by SHA, but there is no Dependabot configuration to propose updates. Add `.github/dependabot.yml` with `package-ecosystem: github-actions` to get automated update PRs.

### CI_RELEASE_TOKEN expiration

The current design relies on job failure + GitHub email notification
as the monitoring mechanism. Record the actual PAT expiration date in
RELEASING.md and set a calendar reminder.

## Common Pitfalls

- **Forgetting `--tags`**: `git push origin main` does NOT push tags. Must use `git push origin main --tags`.
- **Pre-release tags won't trigger**: Pattern `v[0-9]+.[0-9]+.[0-9]+` does not match `v0.2.0-rc.1`.
- **`dtolnay/rust-toolchain@stable` is intentionally not SHA-pinned**: This is the community convention (dtolnay does not publish tags). Do not "fix" this.
- **`cargo publish` is irreversible**: A published version cannot be unpublished, only yanked.
- **The `docs/` exclude in Cargo.toml is security-critical**: It
  prevents any docs (which may contain 1Password vault paths or
  internal notes) from being published to crates.io.

## Maintenance Checklist

### When upgrading action SHAs

- Verify the new SHA corresponds to a tagged release, not an arbitrary commit
- Check the action's changelog for breaking changes in input/output names
- Update the inline version comment (e.g., `# v6.0.2`)
- If updating `upload-artifact` or `download-artifact`, verify they remain on the same major version (v3->v4 was breaking)

### When adding a new build target

- Add the target to the build matrix
- If cross-compilation needed, add `cross: true` and verify Docker image support
- Verify `rusqlite` (bundled SQLite) compiles for the new target
- Update `[package.metadata.binstall]` if archive naming differs
- Update RELEASING.md

### Periodic

- **Monthly**: Check `CI_RELEASE_TOKEN` PAT expiration
- **Quarterly**: Run `cargo deny check` locally for new advisories
- **September 2026**: Evaluate dropping `x86_64-apple-darwin` when macOS Intel moves to Homebrew Tier 3

## Related Documents

- [crates-io-distribution-readiness.md](crates-io-distribution-readiness.md) -- Stage A: Cargo.toml metadata, CI hardening, release infrastructure files
- [shell-completions-main-dependency-gating.md](shell-completions-main-dependency-gating.md) -- Homebrew formula requires `bird completions` subcommand
- [quiet-flag-diagnostic-suppression-pattern.md](quiet-flag-diagnostic-suppression-pattern.md) -- `--quiet` flag for clean CI/scripting output
- [ci-formatting-drift-rust-edition-2024.md](../build-errors/ci-formatting-drift-rust-edition-2024.md) -- Foundational CI patterns and rust-toolchain.toml pinning
- [xurl-subprocess-transport-layer.md](xurl-subprocess-transport-layer.md) --
  External binary dependency pattern informing release concerns
- [github-pat-consolidation-across-repos-20260319.md](github-pat-consolidation-across-repos-20260319.md) --
  `CI_RELEASE_TOKEN` consolidation and permissions spec
- [changelog-as-committed-artifact-20260319.md](changelog-as-committed-artifact-20260319.md) --
  CHANGELOG.md managed during release prep, not auto-generated

## Key Files

| File | Purpose |
|------|---------|
| `.github/workflows/release.yml` | Full 6-job release pipeline |
| `.github/workflows/ci.yml` | CI with audit + package-check |
| `.github/workflows/guard-main-docs.yml` | Docs guard with SHA-pinned action |
| `RELEASING.md` | Human-readable release process |
| `cliff.toml` | git-cliff changelog configuration |
| `~/.claude/skills/rust-tool-release/SKILL.md` | Canonical release standard |
