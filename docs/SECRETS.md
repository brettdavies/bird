# Secrets and default app credentials

Canonical 1Password paths for X (Twitter) API app credentials used by bird. These paths are fixed; use them when updating the baked-in dev client_id or the prod secret for releases.

## 1Password paths (op)

| Purpose | op path |
|--------|---------|
| **Dev app** (baked-in default for `cargo build` / local) | `op://secrets-dev/x_twitter_app_meum_dev/environment` |
| **Prod app** (releases; set as `BIRD_DEFAULT_CLIENT_ID` in GitHub Actions) | `op://secrets-dev/x_twitter_app_bird_prod/environment` |

## Usage

- **Dev:** The OAuth2 client_id from the dev item is baked into `src/config.rs` as `OAUTH2_CLIENT_ID_DEV`. To update it, read the value (e.g. `op read "op://secrets-dev/x_twitter_app_meum_dev/environment"` or the item’s `oauth2_client_id` field as needed) and update the constant.
- **Prod:** For release builds, set the **GitHub repository secret** `BIRD_DEFAULT_CLIENT_ID` (Settings → Secrets and variables → Actions) to the prod app’s OAuth2 client_id. Source: `op://secrets-dev/x_twitter_app_bird_prod/environment` (or the item’s `oauth2_client_id` field). The release workflow reads this secret; no 1Password integration in CI is required.
