# bird

**bird** is a Rust CLI for the X (Twitter) API, built on `xdk-rs` from [xurl-rs](https://github.com/brettdavies/xurl-rs)
for authentication and transport. It adds a local entity store, watchlist monitoring, usage tracking, thread
reconstruction, and structured error output for agents.

## Why bird?

`xr`, the xurl-rs CLI, already provides authentication, curated commands, and raw API access. bird is built on the same
X API library and adds capabilities `xr` doesn't have:

- **Entity store** — Local SQLite cache with per-endpoint TTL. Reduces API calls, enables offline access, and supports
  `--refresh`, `--no-cache`, and `--cache-only` modes.
- **Thread reconstruction** — Rebuilds full conversation threads from a single tweet ID.
- **Watchlist** — Monitor a list of users for recent activity without manual searches.
- **Usage tracking** — Local API usage history with cost estimation; `--sync` to refresh from the X API.
- **Structured errors** — Machine-readable JSON errors on stderr for agent and CI integration.
- **Self-diagnostics** — `bird doctor` reports the embedded X API client, auth state, command availability, and store
  health in one place.

bird delegates all authentication and HTTP transport to the embedded `xdk-rs` library; it starts no `xr` or `xurl`
process.

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
cargo install --locked bird

# Pre-built binary (fast, no compiler needed)
cargo binstall bird
```

### From a release

Download the latest binary for your platform from [Releases](https://github.com/brettdavies/bird/releases). Extract and
place `bird` on your `PATH`.

### From source

```bash
git clone https://github.com/brettdavies/bird
cd bird
cargo build --release
# Binary: target/release/bird
```

### App credentials

bird embeds `xdk-rs` as a library, so it ships as a single binary. You still need X API app credentials. Either set them
as env vars:

```bash
export CLIENT_ID=<your_client_id>
export CLIENT_SECRET=<your_client_secret>
```

Or store an app once via the optional [xurl-rs CLI](https://github.com/brettdavies/xurl-rs) (`xr auth app`); bird reads
the same token store. After credentials are in place, run `bird login` for OAuth2.

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

| Command                  | Description              |
| ------------------------ | ------------------------ |
| `bird tweet <text>`      | Post a tweet             |
| `bird reply <id> <text>` | Reply to a tweet         |
| `bird like <id>`         | Like a tweet             |
| `bird unlike <id>`       | Unlike a tweet           |
| `bird repost <id>`       | Repost (retweet) a tweet |
| `bird unrepost <id>`     | Undo a repost            |
| `bird follow <user>`     | Follow a user            |
| `bird unfollow <user>`   | Unfollow a user          |
| `bird dm <user> <text>`  | Send a direct message    |
| `bird block <user>`      | Block a user             |
| `bird unblock <user>`    | Unblock a user           |
| `bird mute <user>`       | Mute a user              |
| `bird unmute <user>`     | Unmute a user            |

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

| Command                    | Description                                                        |
| -------------------------- | ------------------------------------------------------------------ |
| `bird login`               | Sign in via browser; `--no-browser` for agents                     |
| `bird doctor`              | Diagnostics: X API client, auth, commands, store health            |
| `bird doctor <cmd>`        | Scoped diagnostics for a single command                            |
| `bird cache stats`         | Entity store status                                                |
| `bird cache clear`         | Delete all cached entities                                         |
| `bird completions <shell>` | Generate shell completions (bash, zsh, fish, powershell, elvish)   |

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

Authentication is handled by `xdk-rs`. For headless/CI environments where a browser is not available, use `bird
login --no-browser` (alias `--headless`): bird prints the authorization URL on stdout, then reads the redirect URL back
from stdin. Under `--output json` the prompt and result are wrapped in the standard envelope so agents can parse them
programmatically. Tokens live in the `~/.xurl` token store that `xr` also uses; `XURL_BEARER_TOKEN` supplies an app-only
bearer token without storing it. See [xurl-rs](https://github.com/brettdavies/xurl-rs) for the full set of variables.

bird reads one environment variable of its own: `X_API_USERNAME` (or `--username`) to select which stored account to
use.

### Structured error output

Use `--output json` (or `BIRD_OUTPUT=json`) for machine-readable errors on stderr. When stderr is not a TTY, JSON is the
default.

The envelope is the one `xr` prints, so one parser reads both tools:

```json
{"status":"error","reason":"config-error","exit_code":78,"message":"..."}
{"status":"error","reason":"auth-required","exit_code":77,"message":"...","next_step":{"action":"sign-in","command":"bird login --no-browser"}}
{"status":"error","reason":"forbidden","exit_code":1,"message":"..."}
{"status":"error","reason":"rate-limited","exit_code":3,"message":"...","next_step":{"action":"wait-and-retry","docs":"https://docs.x.com/resources/fundamentals/rate-limits"},"retry_after_secs":42,"retry_at":"2026-10-22T21:00:00Z"}
```

| Field       | Type   | Description                                                                              |
| ----------- | ------ | ---------------------------------------------------------------------------------------- |
| `status`    | string | `error`                                                                                  |
| `reason`    | string | Stable kebab-case reason; for an X API error, the reason `xr` reports                    |
| `exit_code` | number | 1, 2 (usage), 3 (rate limit), 4 (404), 5 (no response), 77, 78, 130/143 (cancelled)      |
| `message`   | string | Human-readable message                                                                   |
| `next_step` | object | The recovery step, when one exists: `action`, then `command` or `template`, and `docs`   |
| `command`   | string | The offending value: an unknown command word, or the command that needs `--force`        |

A rate limit with a known reset adds `retry_after_secs` and `retry_at`; an auth-scheme mismatch adds the endpoint,
the schemes it accepts, and the active app. `--output json` is indented; `--raw` prints it on one line.

`bird schema envelope` prints the JSON Schema for this envelope and the success and dry-run ones.

---

## Config

bird's own config is minimal — just `username` and `watchlist`:

- **Config file (optional):** `~/.config/bird/config.toml`
- **Entity store:** `~/.config/bird/bird.db` (SQLite)

Username priority: `--username` flag > config file > `X_API_USERNAME` env var.

Authentication and token storage are handled by `xdk-rs`, in the `~/.xurl` token store `xr` also uses. To use your own X
Developer app, export `CLIENT_ID` / `CLIENT_SECRET` or store it with `xr auth app` (see
[xurl-rs](https://github.com/brettdavies/xurl-rs)).

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

| Doc                                      | Purpose                                                           |
| ---------------------------------------- | ----------------------------------------------------------------- |
| [docs/CLI_DESIGN.md](docs/CLI_DESIGN.md) | Auth requirements, doctor, and error design                       |
| [docs/DEVELOPER.md](docs/DEVELOPER.md)   | Build from source, architecture, project layout                   |
| [RELEASES.md](RELEASES.md)               | Release process and distribution channels                         |
| [CHANGELOG.md](CHANGELOG.md)             | Version history (generated by [git-cliff](https://git-cliff.org)) |

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
