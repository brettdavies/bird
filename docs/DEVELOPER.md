# Developer guide

## Architecture

bird is a CLI for the X (Twitter) API. All HTTP transport and authentication is delegated to the
[xurl-rs](https://github.com/brettdavies/xurl-rs) crate, embedded as a library at the bird/xurl boundary
(`src/xurl_client/`). bird owns the intelligence layer: entity store, caching, cost tracking, and UX.

```text
bird (CLI + intelligence) --> xurl-rs (embedded library: auth + HTTP) --> X API
```

## Building from source

**Requirements:** Rust stable (1.85+). xurl-rs is pulled in as a Cargo dep; the bird binary ships standalone.

```bash
git clone https://github.com/brettdavies/bird
cd bird
git config core.hooksPath scripts/hooks
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

| Path               | Purpose                                                                                  |
| ------------------ | ---------------------------------------------------------------------------------------- |
| `src/main.rs`      | Thin binary entrypoint; delegates to `bird::cli::run_argv()`                             |
| `src/cli/`         | clap definitions, layered runner entrypoints, dispatcher, per-command handlers           |
| `src/xurl_client/` | `XurlClient` trait, `MockXurlClient` test double, `ConstructionStub` no-credentials stub |
| `src/db/`          | SQLite entity store: caching, usage tracking, migrations; `client/` wraps `XurlClient`   |
| `src/config.rs`    | Config load with priority: args > file > env > default                                   |
| `src/doctor.rs`    | Diagnostic report: linked xurl version, per-app auth state, per-command scheme matrix    |
| `src/cost.rs`      | API cost estimation                                                                      |
| `src/output.rs`    | Color, formatting, ANSI sanitization, JSON envelope rendering                            |
| `src/schema.rs`    | Username validation (`validate_username`)                                                |
| `src/error/`       | `BirdError` enum, exit-code mapping, exhaustive `XurlError → BirdError` translation      |

## Authentication

bird does not handle authentication directly. OAuth2 PKCE, token refresh, OAuth1 signing, and the token store all live
in xurl-rs. The `-u <username>` flag flows into `RequestOptions.username` for multi-user token selection on the embedded
client.

To authenticate: `bird login` calls `xurl::auth::Auth::oauth2_flow` directly (interactive) or
`remote_oauth2_step1`/`step2` (headless via `--no-browser`).

For environment-based auth (agents, CI), set `CLIENT_ID` and `CLIENT_SECRET`. xurl-rs's CLI (`xr auth app`) is an
optional convenience for persisting an app to the shared token store; the embedded client reads the same store.

## Config file

Location: `~/.config/bird/config.toml` (XDG). Example:

```toml
# Which xurl username for multi-user token selection
# username = "your_handle"

# Watchlist of usernames to monitor
# watchlist = ["alice", "bob"]
```

## Git hooks

After cloning, activate local hooks:

```bash
git config core.hooksPath scripts/hooks
```

**`pre-push`** mirrors CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`,
shellcheck, MSRV verification, rustdoc-as-warnings, and Windows cross-clippy. Direct pushes to `main` are blocked
server-side by the `protect-main` ruleset; the local hook focuses on catching CI failures before push.

## Branching workflow

```text
main              <-- releases tagged here
  |
dev               <-- integration branch, all feature PRs target here
  |-- feat/...       (short-lived, PR to dev)
  |-- fix/...
  |-- chore/...
```

## Releasing

See [RELEASING.md](../RELEASING.md).
