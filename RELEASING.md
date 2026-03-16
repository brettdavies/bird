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

- Verifies the tag matches `Cargo.toml` version
- Runs `cargo deny` (license + advisory + ban checking)
- Builds binaries for 5 targets (linux x86_64/aarch64, macos x86_64/aarch64, windows x86_64)
- Ad-hoc codesigns macOS binaries
- Creates `.tar.gz` archives with binary + LICENSE + README
- Publishes to crates.io
- Generates changelog via git-cliff
- Creates a GitHub Release with archives attached
- Dispatches a `repository_dispatch` event to `brettdavies/homebrew-tap`, which automatically updates the formula's version and SHA256

### Pipeline order

```text
check-version + audit -> build (5 targets) -> publish-crate -> release -> homebrew
```

`cargo publish` runs BEFORE GitHub Release creation. If publish fails, no release
is advertised and no Homebrew update is triggered.

## Required GitHub Secrets

| Secret | Purpose | Rotation |
|--------|---------|----------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token | Remove after Trusted Publishing is configured |
| `HOMEBREW_TAP_TOKEN` | Fine-grained PAT with `contents:write` on `brettdavies/homebrew-tap` | Max 1 year; renew before expiry |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

Both secrets are stored in 1Password (`secrets-dev` vault).

## Trusted Publishing (after first release)

After the manual `cargo publish` for v0.1.0:

1. Go to `https://crates.io/settings/tokens/trusted-publishing`
2. Add trusted publisher: owner=`brettdavies`, repo=`bird`, workflow=`release.yml`
3. Enable "Enforce Trusted Publishing" to disable token-based publishing
4. Remove the `CARGO_REGISTRY_TOKEN` secret from the repo
5. Update `release.yml` `publish-crate` job to use `rust-lang/crates-io-auth-action@v1` instead of `CARGO_REGISTRY_TOKEN`

## Distribution Channels

| Channel | How |
|---------|-----|
| Homebrew | `brew install brettdavies/tap/bird` |
| Pre-built binary | Download from [GitHub Releases](https://github.com/brettdavies/bird/releases) |
| Rust crate | `cargo install bird` |
| Fast binary | `cargo binstall bird` |
| From source | `git clone && cargo build --release` |
