# Developer guide

## Architecture

bird is a CLI for the X (Twitter) API. All HTTP transport and authentication is delegated to [xurl](https://github.com/xdevplatform/xurl) via a subprocess transport layer (`src/transport.rs`). bird owns the intelligence layer: entity store, caching, cost tracking, and UX.

```text
bird (CLI + intelligence) --> xurl (subprocess: auth + HTTP) --> X API
```

## Building from source

**Requirements:** Rust stable (1.85+), xurl installed.

```bash
git clone https://github.com/brettdavies/bird
cd bird
git config core.hooksPath .githooks
cargo build --release
```

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

## Project layout

| Path | Purpose |
|------|---------|
| `src/main.rs` | CLI definition, `main()`, command dispatch |
| `src/transport.rs` | xurl subprocess transport layer (all API calls) |
| `src/config.rs` | Config load with priority: args > file > env > default |
| `src/doctor.rs` | Diagnostic report: xurl status, auth, commands, store health |
| `src/db/` | SQLite entity store: caching, usage tracking, migrations |
| `src/cost.rs` | API cost estimation |
| `src/output.rs` | Color, formatting, ANSI sanitization |
| `src/requirements.rs` | Per-command auth requirements (single source of truth) |
| `src/schema.rs` | OpenAPI schema parsing for path templates |

## Authentication

bird does not handle authentication directly. All auth flows (OAuth2 PKCE, token refresh, bearer tokens) are handled by xurl. bird passes the `-u <username>` flag to xurl for multi-user token selection.

To authenticate: `bird login` delegates to `xurl auth oauth2`.

To use environment-based auth (agents, CI): set `X_API_ACCESS_TOKEN` or `X_API_BEARER_TOKEN` — xurl reads these automatically.

## Config file

Location: `~/.config/bird/config.toml` (XDG). Example:

```toml
# Which xurl username for multi-user token selection
# username = "your_handle"

# Watchlist of usernames to monitor
# watchlist = ["alice", "bob"]
```

Legacy fields (`client_id`, `client_secret`, `redirect_uri`) are tolerated by serde but unused since auth was delegated to xurl.

## Git hooks

After cloning, activate local hooks:

```bash
git config core.hooksPath .githooks
```

**`pre-push`** — Prevents direct pushes to `main`. All changes go through PRs.

## Branching workflow

```text
main              <-- releases tagged here
  |
development       <-- integration branch, all feature PRs target here
  |-- feat/...       (short-lived, PR to development)
  |-- fix/...
  |-- chore/...
```

## Releasing

See [RELEASING.md](../RELEASING.md).
