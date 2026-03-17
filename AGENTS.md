# bird

A CLI for the X (Twitter) API. Wraps [xurl-rs](https://github.com/brettdavies/xurl-rs) (or Go [xurl](https://github.com/xdevplatform/xurl)) for transport; bird owns entity store, caching, cost tracking, and UX.

## Binary & Package

- Binary: `bird`
- Package: `bird` (crates.io)

## Architecture

```text
bird (CLI + intelligence) --> xr/xurl (subprocess: auth + HTTP) --> X API
```

- `src/cli.rs` -- clap derive definitions (Cli, Command, CacheAction, WatchlistCommand)
- `src/main.rs` -- BirdError, command dispatch, three-tier main() gating
- `src/transport.rs` -- xurl subprocess transport, XurlError, MockTransport
- `src/db/` -- SQLite entity store (db.rs), entity-aware transport client (client.rs), usage tracking (usage.rs)
- `src/doctor.rs` -- diagnostic report (xurl status, auth, commands, cache health)
- `src/requirements.rs` -- per-command auth requirements (single source of truth)
- `src/output.rs` -- color helpers, `diag!` macro, ANSI sanitization

## Transport Dependency

Requires `xr` (xurl-rs) or `xurl` (Go) on PATH. Override: `BIRD_XURL_PATH`. Minimum version: 1.0.3.

## Quality Bar

- Clippy clean (`cargo clippy -- -D warnings`)
- Formatted with rustfmt, edition 2024 (`cargo fmt --check`)
- No `unwrap()` in production code
- `cargo-deny` for advisories and licenses
- MSRV: 1.87

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Command error (API, network, I/O) |
| 77 | Auth error (HTTP 401/403) |
| 78 | Config error (EX_CONFIG) |

Note: differs from xurl-rs's sequential 0-5 scheme.

## Testing

- `MockTransport` for unit tests; `BIRD_XURL_PATH` for integration tests
- `#[ignore]` live integration test against X API (`cargo test --test live_integration -- --ignored`)
- 201 tests across unit, CLI smoke, and transport integration suites

## Known Debt

- main.rs ~710 lines post-extraction; `run()` alone is ~358 lines
- db/db.rs (1,289 lines) and db/client.rs (1,141 lines) exceed 200-line trigger
- `--pretty` duplicated across 11 subcommand variants (extract CommonFlags post-GA)
- TODO at db/client.rs:38 (body re-serialization from JSON)

## References

- [docs/DEVELOPER.md](docs/DEVELOPER.md) -- build, architecture, project layout
- [docs/CLI_DESIGN.md](docs/CLI_DESIGN.md) -- auth requirements, error design
- [docs/solutions/](docs/solutions/) -- documented patterns and lessons learned
