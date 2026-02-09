# bird

**bird** is a CLI for the X (Twitter) API: zero-config OAuth2 login, curated commands (`me`, `bookmarks`), and schema-driven raw endpoint access (`get`, `post`, `put`, `delete`). Most users can run `bird login` with no setup and start using the API immediately.

- **Priority (everywhere):** command args → config file → environment → built-in default.
- **Docs:** [CLI design](docs/CLI_DESIGN.md) · [Secrets & app credentials](docs/SECRETS.md) · [Developer guide](docs/DEVELOPER.md)

---

## Install

### From a release (recommended)

Download the latest binary for your platform from [Releases](https://github.com/brettdavies/bird/releases). Extract and put `bird` (or `bird.exe` on Windows) on your `PATH`.

### From source (Rust required)

```bash
git clone https://github.com/brettdavies/bird
cd bird
cargo build --release
# Binary: target/release/bird (or target/release/bird.exe on Windows)
```

Install into `~/.cargo/bin`:

```bash
cargo install --path .
```

---

## Quick start

1. Run **`bird login`**. A browser opens; sign in to X and authorize the app. Tokens are stored by your username.
2. Run **`bird me`** or **`bird bookmarks`** (use `--pretty` for readable JSON).

No config or app creation needed for the default app.

---

## Config and credentials

- **Config file (optional):** `~/.config/bird/config.toml` (XDG). See [config.example.toml](config.example.toml).
- **Tokens (after login):** `~/.config/bird/tokens.json` — do not commit.

Credential priority: **CLI args > config file > env > default.**

### Zero-config login (default app)

The app ships with a default OAuth2 **client_id** (public client; no secret). Run **`bird login`** with no config: the browser opens, you sign in and authorize, and tokens are stored.

- **Callback URI** for the default app: `http://127.0.0.1:8765/callback`
- **Dev builds** (e.g. `cargo build`): use the baked-in dev app.
- **Release binaries**: use the production app (via GitHub Actions secret).

Maintainers: canonical 1Password paths are in [docs/SECRETS.md](docs/SECRETS.md).

### Your own app (optional)

To use your own X Developer app (quota isolation, compliance, or custom branding):

- **Public client:** set `X_API_CLIENT_ID` only (or `client_id` in config). Login and refresh use PKCE with no secret.
- **Confidential client:** set `X_API_CLIENT_ID` and `X_API_CLIENT_SECRET` (or in config). Same callback URI: `http://127.0.0.1:8765/callback`.

Optional: `X_API_REDIRECT_URI` (default `http://127.0.0.1:8765/callback`).

See [docs/DEVELOPER.md](docs/DEVELOPER.md) for overriding the client ID or rebuilding with your own app.

---

## Commands

| Command | Description |
|--------|-------------|
| `bird login` | OAuth2 PKCE login; opens browser, stores tokens by username |
| `bird me` | Current user (GET /2/users/me); `--pretty` for readable JSON |
| `bird bookmarks` | List bookmarks (paginated, max 100); `--pretty` |
| `bird get <path>` | Raw GET; path can be literal or template e.g. `/2/users/{id}/bookmarks` with `-p id=123` |
| `bird post <path>` | Raw POST; optional `--body '{"text":"..."}'` |
| `bird put <path>` | Raw PUT |
| `bird delete <path>` | Raw DELETE |
| `bird doctor` | Auth state, config, and which commands can run; `bird doctor <cmd>` for one command; `--pretty` for summary |

Path parameters for raw commands: **CLI `-p key=value`** or env **`X_API_<KEY>`** (e.g. `X_API_ID` for `{id}`).

Output is JSON to stdout by default; use **`--pretty`** for human-readable.

---

## Output: color and hyperlinks

- **`--plain`** — No color, no hyperlinks; use in scripts or pipelines.
- **`--no-color`** — Disable ANSI colors only (or set **`NO_COLOR`**).
- **`TERM=dumb`** or non-TTY — Colors and hyperlinks are disabled automatically.
- When color is on, `bird doctor --pretty` and error messages use color; **`bird login`** prints the authorize URL as a clickable terminal hyperlink (OSC 8) where supported.

---

## Agent / non-interactive usage

Use environment variables only; no browser.

- **OAuth2 user:** `X_API_ACCESS_TOKEN` (and optionally `X_API_REFRESH_TOKEN` for refresh).
- **App-only (bearer):** `X_API_BEARER_TOKEN`.
- **OAuth 1.0a:** `X_API_CONSUMER_KEY`, `X_API_CONSUMER_SECRET`, `X_API_OAUTH1_ACCESS_TOKEN`, `X_API_OAUTH1_ACCESS_TOKEN_SECRET`.
- **Multi-account:** `X_API_USERNAME` to select which stored account to use.

Example:

```bash
export X_API_ACCESS_TOKEN="your_user_access_token"
bird me
bird get /2/users/me --pretty
bird bookmarks
```

---

## Releases

**Local build (no push):**

```bash
cargo build --release
# Run: ./target/release/bird ...  or  cargo run --release -- me --pretty
```

**Publishing:** Release binaries are built automatically when you push a version tag (e.g. `v0.1.0`). See [Releases](https://github.com/brettdavies/bird/releases). Set the repository secret **`BIRD_DEFAULT_CLIENT_ID`** to the prod app’s OAuth2 client ID (source: [docs/SECRETS.md](docs/SECRETS.md)). To cut a new release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The [release workflow](.github/workflows/release.yml) builds for Linux (x86_64), macOS (aarch64), and Windows (x86_64) and attaches binaries to the GitHub Release.

---

## Updating the API schema

The bundled OpenAPI spec is `openapi/x-api-openapi.json`. To support new endpoints without code changes:

1. **Download latest schema** (optional): `./scripts/download-openapi.sh`
2. Rebuild: `cargo build --release`

New paths and path params from the spec are then available to the raw `get` / `post` / `put` / `delete` commands.

---

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/CLI_DESIGN.md](docs/CLI_DESIGN.md) | Auth requirements, doctor, and error design |
| [docs/SECRETS.md](docs/SECRETS.md) | 1Password paths for dev/prod app credentials (maintainers) |
| [docs/DEVELOPER.md](docs/DEVELOPER.md) | Override client ID, rebuild with your own app, build from source |

---

## License

MIT.
