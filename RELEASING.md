# Releasing bird

## Merging development to main

Engineering docs (`docs/plans/`, `docs/solutions/`, `docs/brainstorms/`) live on
`development` only. `guard-main-docs.yml` blocks them from `main`. You MUST use
the release branch pattern:

```bash
# 1. Branch from main, NOT development
git checkout -b release/v0.2.0 origin/main

# 2. Cherry-pick only non-docs commits from development
git cherry-pick <commit1> <commit2> ...

# 3. Verify no docs paths leaked through
git diff origin/main --stat

# 4. Push and open a PR to main
git push -u origin release/v0.2.0
gh pr create --base main
```

**CRITICAL:** Always branch from `origin/main`. Branching from `development`
causes `add/add` merge conflicts when dev and main have divergent histories
(e.g., after squash merges).

## Tagging and releasing

After the PR merges to main, tag and push:

```bash
git checkout main && git pull
git tag v0.2.0
git push origin main --tags
```

This triggers `.github/workflows/release.yml` which:

- Verifies the tag matches `Cargo.toml` version
- Runs `cargo deny` (license + advisory + ban checking)
- Builds binaries for 5 targets (linux x86_64/aarch64, macos x86_64/aarch64, windows x86_64)
- Ad-hoc codesigns macOS binaries
- Creates `.tar.gz` archives with binary + LICENSE + README + shell completions
- Publishes to crates.io via Trusted Publishing (OIDC, no static token)
- Generates changelog via git-cliff
- Creates a GitHub Release with archives attached
- Dispatches `repository_dispatch` to `brettdavies/homebrew-tap`, which auto-updates the formula version and SHA256

### Pipeline order

```text
check-version + audit -> build (5 targets) -> publish-crate -> release -> homebrew
```

`cargo publish` runs BEFORE GitHub Release creation. If publish fails, no release
is advertised and no Homebrew update is triggered.

## Required GitHub Secrets

| Secret               | Purpose                                                              | Rotation                        |
| -------------------- | -------------------------------------------------------------------- | ------------------------------- |
| `CI_RELEASE_TOKEN` | Fine-grained PAT with `contents:write` for CI release automation (Homebrew dispatch, changelog, rulesets) | Max 1 year; renew before expiry |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

Secrets are stored in 1Password (`secrets-dev` vault).

## crates.io Publishing

Publishing uses [Trusted Publishing](https://doc.rust-lang.org/cargo/reference/registry-authentication.html#trusted-publishing)
via `rust-lang/crates-io-auth-action`. No static API token is needed — OIDC
exchanges a short-lived GitHub Actions token for a ~30-minute crates.io token.

Trusted Publishing was configured after the v0.1.0 manual publish. If it ever
needs reconfiguration:

1. Go to `https://crates.io/settings/tokens/trusted-publishing`
2. Add trusted publisher: owner=`brettdavies`, repo=`bird`, workflow=`release.yml`
3. Enable "Enforce Trusted Publishing" to disable token-based publishing

## Distribution Channels

| Channel          | How                                                                             |
| ---------------- | ------------------------------------------------------------------------------- |
| Homebrew         | `brew install brettdavies/tap/bird`                                             |
| Pre-built binary | Download from [GitHub Releases](https://github.com/brettdavies/bird/releases)   |
| Rust crate       | `cargo install bird`                                                            |
| Fast binary      | `cargo binstall bird`                                                           |
| From source      | `git clone && cargo build --release`                                            |
