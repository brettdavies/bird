# Developer guide

For advanced users who want to override the default OAuth2 app, rebuild bird with their own client ID, or understand how credentials and builds work.

---

## Overriding the client ID at runtime

You do **not** need to rebuild. Override via environment or config:

- **Environment:** set `X_API_CLIENT_ID` (and optionally `X_API_CLIENT_SECRET` for a confidential client).
- **Config file:** in `~/.config/bird/config.toml` set `client_id` and optionally `client_secret`.

Priority is always: CLI args > config file > env > built-in default. So as soon as you set `X_API_CLIENT_ID` or `client_id` in config, that value is used instead of the shipped default.

Same callback URI applies: `http://127.0.0.1:8765/callback` must be registered for your app in the [X Developer Portal](https://developer.x.com).

---

## Rebuilding with your own app (build-time client ID)

If you want the **default** (when no env or config is set) to be your app’s client ID, build with:

```bash
BIRD_DEFAULT_CLIENT_ID="your_oauth2_client_id" cargo build --release
```

- The binary will use that client ID when the user has not set `X_API_CLIENT_ID` or config.
- Use this for private builds or forks where you want your app to be the default.
- **Do not** ship a client secret in the binary; use the public client flow (no secret) for distributable builds.

---

## Dev vs prod default

- **Dev builds** (e.g. `cargo build` with no env): use the baked-in **dev** app client ID in `src/config.rs` (`OAUTH2_CLIENT_ID_DEV`). Intended for local development.
- **Release builds** (CI): the release workflow sets `BIRD_DEFAULT_CLIENT_ID` from the GitHub secret, so release binaries use the **prod** app client ID.

To update the baked-in dev client ID, edit `OAUTH2_CLIENT_ID_DEV` in `src/config.rs`. The value should match the dev app’s OAuth2 client ID (see [SECRETS.md](SECRETS.md) for the 1Password path).

---

## 1Password paths (maintainers)

Canonical op paths for app credentials:

| Purpose | op path |
|--------|---------|
| **Dev app** (baked-in default for `cargo build`) | `op://secrets-dev/x_twitter_app_meum_dev/environment` |
| **Prod app** (releases; GitHub secret `BIRD_DEFAULT_CLIENT_ID`) | `op://secrets-dev/x_twitter_app_bird_prod/environment` |

Details: [SECRETS.md](SECRETS.md).

---

## Config file reference

Location: `~/.config/bird/config.toml` (XDG). Example:

```toml
# Optional: use your own app instead of the default
# client_id = "your_oauth2_client_id"
# client_secret = "your_oauth2_client_secret"   # only for confidential client
# redirect_uri = "http://127.0.0.1:8765/callback"
# username = "your_handle"   # which stored account to use
```

Redirect URI must be `http://127.0.0.1:8765/callback` in the X Developer Portal for local login.

---

## Building from source

**Requirements:** Rust (stable), e.g. via [rustup](https://rustup.rs).

```bash
git clone https://github.com/brettdavies/bird
cd bird
git config core.hooksPath .githooks
cargo build --release
```

The `git config` command activates the local git hooks (see below).

Run tests:

```bash
cargo test
```

Run a command:

```bash
cargo run --release -- me --pretty
# or
./target/release/bird me --pretty
```

---

## Git hooks

The repo includes a `.githooks/` directory with local git hooks. After cloning, activate them:

```bash
git config core.hooksPath .githooks
```

**`pre-push`** — Prevents direct pushes to `main`. All changes to main should go through a PR.

---

## Branching workflow

```
main              ← releases tagged here, no engineering docs
  │
development       ← integration branch, all feature PRs target here
  ├── feat/...       (short-lived, PR to development)
  ├── fix/...        (short-lived, PR to development)
  └── chore/...      (short-lived, PR to development)
```

1. Create a feature branch from `development`
2. Open a PR targeting `development`
3. CI runs on the PR (fmt, clippy, test)
4. Merge to `development`

When ready to release, merge `development` into `main` via a release branch that strips `docs/plans/`, `docs/solutions/`, and `docs/brainstorms/` (the guard workflow enforces this). Tag on `main`.

---

## Project layout (relevant to credentials)

- **`src/config.rs`** — Config load order; `OAUTH2_CLIENT_ID_DEV` constant; `option_env!("BIRD_DEFAULT_CLIENT_ID")` fallback at runtime.
- **`src/login.rs`** — OAuth2 PKCE login flow; uses resolved `client_id` and optional `client_secret`.
- **`src/auth.rs`** — Token exchange and refresh; supports both public client (no secret) and confidential client (Basic auth).

---

## Summary

| Goal | Approach |
|------|----------|
| Use your app without rebuilding | Set `X_API_CLIENT_ID` (and optionally `X_API_CLIENT_SECRET`) or config file. |
| Default in your build is your app | Build with `BIRD_DEFAULT_CLIENT_ID=your_id cargo build --release`. |
| Update the dev app in source | Edit `OAUTH2_CLIENT_ID_DEV` in `src/config.rs`; value from op dev path. |
| Release binaries use prod app | Set GitHub repo secret `BIRD_DEFAULT_CLIENT_ID` to prod app’s client ID (from op prod path). |
