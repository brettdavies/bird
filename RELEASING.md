# Releasing bird

## Automated (preferred)

Tag a version and push — CI handles everything:

```bash
# 1. Bump version in Cargo.toml
# 2. Commit and tag
git add Cargo.toml
git commit -m "chore: bump version to 0.2.0"
git tag v0.2.0
git push origin main --tags
```

This triggers `.github/workflows/release.yml` which:

- Builds binaries for 3 targets (linux x86_64, macos aarch64, windows x86_64)
- Creates a GitHub Release with all binaries attached

Changelog is auto-generated on every push to main via git-cliff.

## Required GitHub Secrets

| Secret | Purpose | Rotation |
|--------|---------|----------|
| `HOMEBREW_TAP_TOKEN` | Fine-grained PAT with `contents:write` on `brettdavies/homebrew-tap` | Max 1 year; renew before expiry |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

## Future: crates.io Publishing

After manual `cargo publish` for v0.1.0:

1. Configure Trusted Publishing on crates.io (owner=`brettdavies`, repo=`bird`, workflow=`release.yml`)
2. Add `publish-crate` job to release workflow
3. Enable "Enforce Trusted Publishing" on crates.io to disable API token publishing

## Distribution Channels

| Channel | How |
|---------|-----|
| Homebrew | `brew install brettdavies/tap/bird` |
| Pre-built binary | Download from [GitHub Releases](https://github.com/brettdavies/bird/releases) |
| Rust crate | `cargo install bird` |
| Fast binary | `cargo binstall bird` |
| From source | `git clone && cargo build --release` |
