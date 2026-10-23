# bird

**bird** is a Rust CLI for the X (Twitter) API, built on `xdk-rs` from [xurl-rs](https://github.com/brettdavies/xurl-rs)
for authentication and transport. It adds a Markdown archive, same-day response files, watchlist monitoring, usage
tracking, thread reconstruction, and structured error output for agents.

## Why bird?

`xr`, the xurl-rs CLI, already provides authentication, curated commands, and raw API access. bird is built on the same
X API library and adds capabilities `xr` doesn't have:

- **File store** — A Markdown note for every post, user, bookmark, and like bird reads, in a folder you can point at a
  notes vault, plus same-day response files that answer a repeated read without a request. Supports `--refresh`,
  `--no-cache`, and `--cache-only` modes.
- **Thread reconstruction** — Rebuilds full conversation threads from a single tweet ID.
- **Watchlist** — Monitor a list of users for recent activity without manual searches.
- **Spend ledger** — Local history of API calls with cost estimates (`bird spend`); `--sync` adds X's actuals.
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
| `bird watchlist fetch`         | Fetch recent activity for watched users |
| `bird watchlist add <user>`    | Add a user to the watchlist             |
| `bird watchlist remove <user>` | Remove a user from the watchlist        |
| `bird watchlist list`          | Show the current watchlist              |
| `bird spend`                   | View the local spend ledger             |
| `bird spend --sync`            | Also fetch X's actual usage             |

### System

| Command                     | Description                                                      |
| --------------------------- | ---------------------------------------------------------------- |
| `bird login`                | Sign in via browser; `--no-browser` for agents                   |
| `bird doctor`               | Diagnostics: X API client, auth, commands, store health          |
| `bird doctor <cmd>`         | Scoped diagnostics for a single command                          |
| `bird cache stats`          | Archive, response files, and ledger status                       |
| `bird cache clear`          | Delete stored responses; the archive stays                       |
| `bird completions <shell>`  | Generate shell completions (bash, zsh, fish, powershell, elvish) |
| `bird skill install <host>` | Install bird's agent skill into a host's skills directory        |

---

## File store

bird keeps no database. Each post, user, bookmark, and like it reads becomes a Markdown note with YAML frontmatter under
`~/.local/share/bird/archive/` (set `archive_dir` in `config.toml` or `BIRD_ARCHIVE_DIR` to put it inside a notes
vault). A later fetch updates bird's fields and keeps everything you added to a note. Each read's response is also kept
for the rest of the UTC day, per account, so the same read again prints the same output and sends nothing. A 0.2.0 store
is removed on the first run.

```bash
bird search "rust" --refresh     # Send the read, store the fresh response
bird search "rust" --no-cache    # Read and write nothing
bird search "rust" --cache-only  # Answer from today's stored response only, no API requests
bird cache stats                 # Archive, response files, and ledger
bird cache clear                 # Delete stored responses; the archive stays
```

---

## Output and formatting

Read commands print X's JSON response to stdout; use `--pretty` for formatted output. bird's own reports (`bird doctor`,
`bird cache stats`, `bird spend`) print as human-readable text in text mode, the default when stderr is a terminal, and
as one JSON document under `--output json` or when stderr is not a terminal.

```bash
bird me --pretty           # Formatted, colored output
bird doctor                # Human-readable report on a terminal
bird doctor --output json  # The same report as JSON
bird me --color never      # No color (or set NO_COLOR)
bird me --output json      # Force JSON error output on stderr
bird me -q                 # Suppress informational stderr messages
```

Colors and hyperlinks are disabled automatically when stdout is not a TTY or `TERM=dumb`.

---

## Agent and non-interactive usage

Authentication is handled by `xdk-rs`. For headless/CI environments where a browser is not available, use
`bird login --no-browser` (alias `--headless`): bird prints the authorization URL on stdout, then reads the redirect URL
back from stdin. Under `--output json` the prompt and result are wrapped in the standard envelope so agents can parse
them programmatically. Tokens live in the `~/.xurl` token store that `xr` also uses; `XURL_BEARER_TOKEN` supplies an
app-only bearer token without storing it. See [xurl-rs](https://github.com/brettdavies/xurl-rs) for the full set of
variables.

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

| Field       | Type   | Description                                                                            |
| ----------- | ------ | -------------------------------------------------------------------------------------- |
| `status`    | string | `error`                                                                                |
| `reason`    | string | Stable kebab-case reason; for an X API error, the reason `xr` reports                  |
| `exit_code` | number | 1, 2 (usage), 3 (rate limit), 4 (404), 5 (no response), 77, 78, 130/143 (cancelled)    |
| `message`   | string | Human-readable message                                                                 |
| `next_step` | object | The recovery step, when one exists: `action`, then `command` or `template`, and `docs` |
| `command`   | string | The offending value: an unknown command word, or the command that needs `--force`      |

A rate limit with a known reset adds `retry_after_secs` and `retry_at`; an auth-scheme mismatch adds the endpoint, the
schemes it accepts, and the active app. `--output json` is indented; `--raw` prints it on one line.

`bird schema envelope` prints the JSON Schema for this envelope and the success and dry-run ones.

---

## Config

bird's own config is minimal: `username`, `watchlist`, and `archive_dir`.

- **Config file (optional):** `~/.config/bird/config.toml`
- **Archive:** `~/.local/share/bird/archive/`; `archive_dir` or `BIRD_ARCHIVE_DIR` moves it
- **Response files:** `~/.cache/bird/responses/`
- **Spend ledger:** `~/.local/share/bird/spend.jsonl`

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

## Agent skill

`bird skill install <host>` clones the [bird skill bundle](https://github.com/brettdavies/bird-skill) into the directory
each agent host reads skills from; `bird skill update <host>` replaces it. `bird skill install --all` installs into
every host; `bird skill update --all` refreshes every host that already has a copy and reports the rest as skipped.
`--dry-run` prints the planned clone without running it.

| Host          | Destination                      | Moved by                                 |
| ------------- | -------------------------------- | ---------------------------------------- |
| `claude_code` | `~/.claude/skills/bird`          | `CLAUDE_CONFIG_DIR`                      |
| `codex`       | `~/.agents/skills/bird`          |                                          |
| `cursor`      | `~/.cursor/skills/bird`          |                                          |
| `factory`     | `~/.factory/skills/bird`         |                                          |
| `kiro`        | `~/.kiro/skills/bird`            | `KIRO_HOME`                              |
| `opencode`    | `~/.config/opencode/skills/bird` | `OPENCODE_CONFIG_DIR`, `XDG_CONFIG_HOME` |

Destinations resolve as `xr skill install` resolves them. A host's own config-directory variable comes first, then
`BIRD_SKILL_HOME` (which stands in for `HOME` for every host), then `XDG_CONFIG_HOME` for opencode, then `HOME`. Codex
still reads the older `~/.codex/skills`, so a copy found there is reported; `bird skill update codex` removes it and
installs at `~/.agents/skills/bird`.

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
