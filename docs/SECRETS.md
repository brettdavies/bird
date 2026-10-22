# Secrets

bird delegates all authentication to the embedded `xdk-rs` library from
[xurl-rs](https://github.com/brettdavies/xurl-rs). There are no API keys, tokens, or OAuth2 client IDs in the bird
codebase.

## GitHub Actions Secrets

| Secret             | Purpose                                                                                                   |
| ------------------ | --------------------------------------------------------------------------------------------------------- |
| `CI_RELEASE_TOKEN` | Fine-grained PAT with `contents:write` for CI release automation (Homebrew dispatch, changelog, rulesets) |

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

## 1Password paths (maintainers)

| Purpose        | op path                                            |
| -------------- | -------------------------------------------------- |
| CI release PAT | `op://secrets-dev/<release-token-uuid>/credential` |

`<release-token-uuid>` is the UUID of the GitHub token item whose `secret_name` field is `CI_RELEASE_TOKEN`. The item's
title contains parentheses, which `op://` references cannot carry, so the path names the item by UUID.

## Not a bird secret

`BIRD_DEFAULT_CLIENT_ID` is not used: bird holds no OAuth2 client ID of its own. The X app's credentials come from the
`CLIENT_ID` / `CLIENT_SECRET` environment variables or the `~/.xurl` token store.
