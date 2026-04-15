# bird

**bird** is a Rust CLI for the X (Twitter) API, built on [xurl](https://github.com/xdevplatform/xurl) for authentication
and transport. It adds a local entity store, watchlist monitoring, usage tracking, thread reconstruction, and structured
error output for agents.

## Why bird?

xurl already provides authentication, curated commands, and raw API access. bird extends xurl with capabilities it
doesn't have:

- **Entity store** — Local SQLite cache with per-endpoint TTL. Reduces API calls, enables offline access, and supports
  `--refresh`, `--no-cache`, and `--cache-only` modes.
- **Thread reconstruction** — Rebuilds full conversation threads from a single tweet ID.
- **Watchlist** — Monitor a list of users for recent activity without manual searches.
- **Usage tracking** — Local API usage history with cost estimation; `--sync` to refresh from the X API.
- **Structured errors** — Machine-readable JSON errors on stderr for agent and CI integration.
- **Self-diagnostics** — `bird doctor` reports xurl status, auth state, command availability, and store health in one
  place.

bird delegates all authentication and HTTP transport to xurl at runtime.

---

## Install

### Homebrew (macOS / Linux)

```bash
brew tap brettdavies/tap
brew install bird
```

### crates.io

```bash
# From source (requires Rust toolchain)
cargo install bird

# Pre-built binary (fast, no compiler needed)
cargo binstall bird
```

### From a release

Download the latest binary for your platform from [Releases](https://github.com/brettdavies/bird/releases). Extract
and place `bird` on your `PATH`.

### From source

```bash
git clone https://github.com/brettdavies/bird
cd bird
cargo build --release
# Binary: target/release/bird
```

### Prerequisite: xurl

bird requires [xurl-rs](https://github.com/brettdavies/xurl-rs) (or the Go [xurl](https://github.com/xdevplatform/xurl))
for X API authentication:

```bash
# Recommended: xurl-rs (Rust)
brew tap brettdavies/tap
brew install xurl-rs

# Alternative: xurl (Go original)
brew tap xdevplatform/tap
brew install xurl
```

bird checks for `xr` (xurl-rs) first, then `xurl` (Go). Override with `BIRD_XURL_PATH`.

Verify your setup: `bird doctor`

---

## Quick start

```bash
bird login           # Opens browser, sign in, done
bird me --pretty     # Current user profile
bird bookmarks       # List bookmarks (paginated)
bird search "rust lang" --sort likes --min-likes 100
bird tweet "Hello from bird"
```

No config or app creation needed.

---

## Commands

### Read

| Command                  | Description                                                        |
| ------------------------ | ------------------------------------------------------------------ |
| `bird me`                | Current user (`GET /2/users/me`)                                   |
| `bird bookmarks`         | List bookmarks (paginated, max 100 per page)                       |
| `bird profile <user>`    | Look up a user by username                                         |
| `bird search <query>`    | Search recent tweets; `--sort likes`, `--min-likes N`, `--pages N` |
| `bird thread <tweet_id>` | Reconstruct a conversation thread                                  |

### Write

| Command                   | Description                |
| ------------------------- | -------------------------- |
| `bird tweet <text>`       | Post a tweet               |
| `bird reply <id> <text>`  | Reply to a tweet           |
| `bird like <id>`          | Like a tweet               |
| `bird unlike <id>`        | Unlike a tweet             |
| `bird repost <id>`        | Repost (retweet) a tweet   |
| `bird unrepost <id>`      | Undo a repost              |
| `bird follow <user>`      | Follow a user              |
| `bird unfollow <user>`    | Unfollow a user            |
| `bird dm <user> <text>`   | Send a direct message      |
| `bird block <user>`       | Block a user               |
| `bird unblock <user>`     | Unblock a user             |
| `bird mute <user>`        | Mute a user                |
| `bird unmute <user>`      | Unmute a user              |

### Monitoring

| Command                        | Description                             |
| ------------------------------ | --------------------------------------- |
| `bird watchlist check`         | Check recent activity for watched users |
| `bird watchlist add <user>`    | Add a user to the watchlist             |
| `bird watchlist remove <user>` | Remove a user from the watchlist        |
| `bird watchlist list`          | Show the current watchlist              |
| `bird usage`                   | View local API usage and cost estimates |
| `bird usage --sync`            | Refresh usage data from the X API       |

### Raw API access

| Command              | Description                                             |
| -------------------- | ------------------------------------------------------- |
| `bird get <path>`    | `GET` request; supports path templates with `-p id=123` |
| `bird post <path>`   | `POST` request; optional `--body '{"text":"..."}'`      |
| `bird put <path>`    | `PUT` request                                           |
| `bird delete <path>` | `DELETE` request                                        |

### System

| Command                       | Description                                                        |
| ----------------------------- | ------------------------------------------------------------------ |
| `bird login`                  | Sign in via browser (delegates to xurl)                            |
| `bird doctor`                 | Diagnostics: xurl status, auth, commands, store health             |
| `bird doctor <cmd>`           | Scoped diagnostics for a single command                            |
| `bird cache stats`            | Entity store status                                                |
| `bird cache clear`            | Delete all cached entities                                         |
| `bird completions <shell>`    | Generate shell completions (bash, zsh, fish, powershell, elvish)   |

---

## Entity store

bird maintains a local SQLite entity store that caches API responses with per-endpoint TTL. This reduces redundant API
calls and provides offline access to previously fetched data.

```bash
bird search "rust" --refresh     # Bypass store, fetch fresh, update store
bird search "rust" --no-cache    # Skip store entirely (no read, no write)
bird search "rust" --cache-only  # Serve from store only, no API requests
bird cache stats --pretty        # View store status
bird cache clear                 # Wipe the store
```

---

## Output and formatting

All commands emit JSON to stdout by default. Use `--pretty` for human-readable output.

```bash
bird me --pretty         # Formatted, colored output
bird me --plain          # No color, no hyperlinks (script-friendly)
bird me --no-color       # Disable ANSI colors only (or set NO_COLOR)
bird me --output json    # Force JSON error output on stderr
bird me -q               # Suppress informational stderr messages
```

Colors and hyperlinks are disabled automatically when stdout is not a TTY or `TERM=dumb`.

---

## Agent and non-interactive usage

Authentication is handled entirely by xurl. For headless/CI environments where a browser is not available, configure
auth tokens through xurl's environment variables — see [xurl documentation](https://github.com/xdevplatform/xurl).

bird reads one environment variable: `X_API_USERNAME` (or `--username`) to select which stored account xurl should use.

### Structured error output

Use `--output json` (or `BIRD_OUTPUT=json`) for machine-readable errors on stderr. When stderr is not a TTY, JSON is the
default.

```json
{"error":"message","kind":"config","code":78}
{"error":"message","kind":"auth","code":77}
{"error":"message","kind":"command","command":"me","code":1}
{"error":"message","kind":"command","command":"get","status":429,"code":1}
```

| Field     | Type   | Description                               |
| --------- | ------ | ----------------------------------------- |
| `error`   | string | Error message                             |
| `kind`    | string | `config`, `auth`, or `command`            |
| `code`    | number | Exit code (78, 77, or 1)                  |
| `command` | string | Command name (only for `kind: "command"`) |
| `status`  | number | HTTP status (only for API errors)         |

---

## Config

bird's own config is minimal — just `username` and `watchlist`:

- **Config file (optional):** `~/.config/bird/config.toml`
- **Entity store:** `~/.config/bird/bird.db` (SQLite)

Username priority: `--username` flag > config file > `X_API_USERNAME` env var.

Authentication and token storage are handled entirely by xurl. To use your own X Developer app, configure it through
xurl — see [xurl documentation](https://github.com/xdevplatform/xurl).

---

## Shell completions

Generate and install completions for your shell:

```bash
# Bash
bird completions bash > ~/.local/share/bash-completion/completions/bird

# Zsh (writes to the first directory on your fpath)
bird completions zsh > "${fpath[1]}/_bird"

# Fish
bird completions fish > ~/.config/fish/completions/bird.fish

# PowerShell
bird completions powershell > bird.ps1

# Elvish
bird completions elvish > bird.elv
```

Pre-generated scripts are also available in `completions/`.

Homebrew users get completions installed automatically.

---

## Documentation

| Doc                                           | Purpose                                                              |
| --------------------------------------------- | -------------------------------------------------------------------- |
| [docs/CLI_DESIGN.md](docs/CLI_DESIGN.md)      | Auth requirements, doctor, and error design                          |
| [docs/DEVELOPER.md](docs/DEVELOPER.md)        | Build from source, architecture, project layout                      |
| [RELEASING.md](RELEASING.md)                  | Release process and distribution channels                            |
| [CHANGELOG.md](CHANGELOG.md)                  | Version history (generated by [git-cliff](https://git-cliff.org))    |

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
