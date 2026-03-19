# Secrets

bird delegates all authentication to [xurl](https://github.com/xdevplatform/xurl). There are no API keys, tokens, or OAuth2 client IDs in the bird codebase.

## GitHub Actions Secrets

| Secret | Purpose |
|--------|---------|
| `CI_RELEASE_TOKEN` | Fine-grained PAT with `contents:write` for CI release automation (Homebrew dispatch, changelog, rulesets) |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

## 1Password paths (maintainers)

| Purpose | op path |
|---------|---------|
| CI release PAT | `op://secrets-dev/CI_RELEASE_TOKEN/credential` |

## Removed

The `BIRD_DEFAULT_CLIENT_ID` secret was removed — it referenced dead code from a pre-xurl architecture where bird handled OAuth2 directly. Auth is now fully delegated to xurl via the subprocess transport layer (see `src/transport.rs`).
