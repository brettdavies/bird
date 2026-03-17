# bird

**bird** is a CLI for the X (Twitter) API: zero-config OAuth2 login, curated commands (`me`, `bookmarks`, `search`, `thread`, `watchlist`), and schema-driven raw endpoint access (`get`, `post`, `put`, `delete`). Most users can run `bird login` with no setup and start using the API immediately.

- **Priority (everywhere):** command args > config file > environment > built-in default.
- **Docs:** [CLI design](docs/CLI_DESIGN.md) · [Developer guide](docs/DEVELOPER.md)

---

## Install

### Homebrew (macOS / Linux)

```bash
brew install brettdavies/tap/bird
```

### cargo install (Rust required)

```bash
cargo install bird
```

### cargo binstall (fast, pre-built binary)

```bash
cargo binstall bird
```

### From a release

Download the latest binary for your platform from [Releases](https://github.com/brettdavies/bird/releases). Extract and put `bird` on your `PATH`.

### From source

```bash
git clone https://github.com/brettdavies/bird
cd bird
cargo build --release
# Binary: target/release/bird
```

### Prerequisite: xurl

bird requires [xurl-rs](https://github.com/brettdavies/xurl-rs) (or the Go [xurl](https://github.com/xdevplatform/xurl)) for X API authentication:

```bash
# Recommended: xurl-rs (Rust)
brew install brettdavies/tap/xurl-rs

# Alternative: xurl (Go original)
brew install xdevplatform/tap/xurl
```

bird checks for `xr` (xurl-rs) first, then `xurl` (Go). Override with `BIRD_XURL_PATH`.

Verify your setup: `bird doctor`

---

## Quick start

1. Run **`bird login`**. A browser opens; sign in to X and authorize the app. Tokens are stored by your username.
2. Run **`bird me`** or **`bird bookmarks`** (use `--pretty` for readable JSON).

No config or app creation needed.

---

## Config

- **Config file (optional):** `~/.config/bird/config.toml` (XDG). See [config.example.toml](config.example.toml).
- **Tokens:** managed by xurl in `~/.xurl/`.

Credential priority: **CLI args > config file > env > default.**

### Your own app (optional)

To use your own X Developer app (quota isolation, compliance, or custom branding), configure xurl directly. See [xurl documentation](https://github.com/xdevplatform/xurl).

---

## Commands

| Command | Description |
|---------|-------------|
| `bird login` | OAuth2 PKCE login; opens browser, stores tokens by username |
| `bird me` | Current user (GET /2/users/me); `--pretty` for readable JSON |
| `bird bookmarks` | List bookmarks (paginated, max 100); `--pretty` |
| `bird profile <user>` | Look up a user profile by username |
| `bird search <query>` | Search recent tweets; `--sort likes`, `--min-likes N`, `--pages N` |
| `bird thread <tweet_id>` | Reconstruct a conversation thread |
| `bird watchlist check` | Check recent activity for watched users |
| `bird watchlist add <user>` | Add a user to the watchlist |
| `bird usage` | View API usage and costs; `--sync` to refresh from API |
| `bird get <path>` | Raw GET; path can be template e.g. `/2/users/{id}/bookmarks` with `-p id=123` |
| `bird post <path>` | Raw POST; optional `--body '{"text":"..."}'` |
| `bird put <path>` | Raw PUT |
| `bird delete <path>` | Raw DELETE |
| `bird tweet <text>` | Post a tweet |
| `bird reply <id> <text>` | Reply to a tweet |
| `bird like <id>` | Like a tweet |
| `bird follow <user>` | Follow a user |
| `bird dm <user> <text>` | Send a direct message |
| `bird doctor` | Diagnostic: xurl status, auth, commands, store health; `--pretty` |
| `bird cache stats` | Store status (JSON default, `--pretty` for human-readable) |
| `bird cache clear` | Delete all cache entries |
| `bird completions <shell>` | Generate shell completions (bash, zsh, fish, powershell, elvish) |

Output is JSON to stdout by default; use **`--pretty`** for human-readable.

---

## Shell completions

Generate and install completions for your shell:

```bash
# Bash
bird completions bash > ~/.local/share/bash-completion/completions/bird

# Zsh
bird completions zsh > ~/.zfunc/_bird

# Fish
bird completions fish > ~/.config/fish/completions/bird.fish
```

Homebrew users get completions installed automatically.

---

## Output: color and hyperlinks

- **`--plain`** — No color, no hyperlinks; use in scripts or pipelines.
- **`--no-color`** — Disable ANSI colors only (or set **`NO_COLOR`**).
- **`TERM=dumb`** or non-TTY — Colors and hyperlinks are disabled automatically.

---

## Agent / non-interactive usage

Use environment variables only; no browser.

- **OAuth2 user:** `X_API_ACCESS_TOKEN` (and optionally `X_API_REFRESH_TOKEN` for refresh).
- **App-only (bearer):** `X_API_BEARER_TOKEN`.
- **OAuth 1.0a:** `X_API_CONSUMER_KEY`, `X_API_CONSUMER_SECRET`, `X_API_OAUTH1_ACCESS_TOKEN`, `X_API_OAUTH1_ACCESS_TOKEN_SECRET`.
- **Multi-account:** `X_API_USERNAME` or `--username` to select stored account.

```bash
export X_API_ACCESS_TOKEN="your_user_access_token"
bird me
bird bookmarks
```

---

## Updating the API schema

The bundled OpenAPI spec is `openapi/x-api-openapi.json`. To support new endpoints:

1. Download latest schema: `./scripts/download-openapi.sh`
2. Rebuild: `cargo build --release`

---

## Documentation

| Doc | Purpose |
|-----|---------|
| [docs/CLI_DESIGN.md](docs/CLI_DESIGN.md) | Auth requirements, doctor, and error design |
| [docs/DEVELOPER.md](docs/DEVELOPER.md) | Build from source, architecture, project layout |
| [RELEASING.md](RELEASING.md) | Release process and distribution channels |
| [CHANGELOG.md](CHANGELOG.md) | Version history |

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
