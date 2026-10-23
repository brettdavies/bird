---
name: bird
binary: bird
description: Rust CLI for the X (Twitter) v2 API with entity caching, watchlist, search, thread reconstruction, OAuth2 login (interactive and headless), structured JSON output envelope, and JSON Schema documents. Triggers on X API, Twitter API, tweet, bookmark, like, follow, watchlist, X search, X bookmarks, thread reconstruction.
homepage: https://github.com/brettdavies/bird
repository: https://github.com/brettdavies/bird
---

# bird

## Running bird

The crate is `bird`. The installed binary is `bird`. All HTTP and authentication go through the embedded `xdk-rs`
library; bird starts no `xr` or `xurl` process.

```bash
# Read commands — current user, bookmarks, profile lookup, search, thread reconstruction
bird me
bird me --pretty
bird bookmarks --output jsonl
bird profile jack
bird search "rust" --output json
bird search "rust" --sort likes --min-likes 100 --pages 2
bird thread 1234567890

# Write commands — tweet, reply, like, follow, DM (each sent through the embedded client)
bird tweet "hello from bird"
bird like 1234567890
bird follow jack
bird dm jack "ping"

# Write-op guards: --dry-run prints the would-be request without sending;
# --force / --yes bypass confirmation; without either, no-TTY callers get exit 1
# (`confirmation-required`) instead of hanging on stdin.
bird like 1234567890 --dry-run
bird like 1234567890 --force

# Pagination — global --limit and --cursor on list-style commands
bird bookmarks --limit 25
bird search "rust" --cursor <token-from-previous-meta.next_cursor>

# Watchlist — track users and check recent activity without manual searches
bird watchlist add x
bird watchlist list
bird watchlist fetch
bird watchlist remove x

# Spend ledger and cache inspection — `bird spend` reads the local cost-estimate
# ledger with no request; `--sync` also fetches X's actual usage beside it
bird spend
bird spend --sync
bird cache stats
bird cache clear

# JSON Schema documents: the envelope and each command's output
bird schema envelope
bird schema thread
bird schema --list           # every name and title
bird schema --all --output json

# Self-diagnostics and shell completions
bird doctor
bird doctor me
bird completions zsh

# Agent-skill bundle: install bird's SKILL.md to a host's skills directory
bird skill install                          # default host: claude-code
bird skill install --dry-run
bird skill update                           # refresh installed bundle
```

Bare `bird` (no arguments) prints help and exits 2. Run `bird --examples` for the curated examples block, or
`bird <sub> --help` for per-subcommand examples.

## Global flags and environment variables

Every subcommand inherits these global flags. Each binds to a `BIRD_*` env var so non-interactive callers can configure
bird without arguments.

| Flag                                | Env var                 | Purpose                                                                      |
| ----------------------------------- | ----------------------- | ---------------------------------------------------------------------------- |
| `--output {text,json,jsonl,ndjson}` | `BIRD_OUTPUT`           | Output format. Defaults to `text` on TTY, `json` when stderr is non-TTY.     |
| `--json`                            | `BIRD_JSON=1`           | Shorthand for `--output json`.                                               |
| `--jsonl`                           | `BIRD_JSONL=1`          | Shorthand for `--output jsonl`.                                              |
| `--color {auto,always,never}`       | `BIRD_COLOR`            | Color mode. `NO_COLOR=1` forces `never`.                                     |
| `-q`, `--quiet`                     | `BIRD_QUIET=1`          | Suppress informational stderr; keep fatal errors.                            |
| `-v`, `--verbose`                   | `BIRD_VERBOSE`          | Repeatable: `-v` info, `-vv` debug, `-vvv` trace.                            |
| `--timeout <secs>`                  | `BIRD_TIMEOUT`          | Network timeout threaded into `xdk::api::Client::with_timeout` (default 30). |
| `--no-interactive`                  | `BIRD_NO_INTERACTIVE=1` | Refuse anything that would block on stdin.                                   |
| `--raw`                             |                         | Emit unstyled, compact output: pipe-safe text, and `json` on one line.       |
| `--examples`                        |                         | Print the curated examples block and exit zero.                              |
| `--refresh`                         |                         | Send every read instead of answering it from a stored response.              |
| `--no-cache`                        | `BIRD_NO_CACHE=1`       | Read and write no stored responses, notes, or ledger records.                |
| `--cache-only`                      |                         | Answer only from stored responses; never hit the API.                        |
| `--limit <N>`                       |                         | Cap for list-style commands (default 100, ceiling 1000).                     |
| `--cursor <TOKEN>`                  |                         | Pagination cursor (alias `--page`); responses surface `meta.next_cursor`.    |
| `-u`, `--username <name>`           | `X_API_USERNAME`        | Multi-user token selection threaded into the embedded request options.       |

Additional env vars without flag pairs:

- `CLIENT_ID` / `CLIENT_SECRET` — X API app credentials. Alternative to an app stored with the xurl-rs CLI
  (`<secret-command> | xr auth apps add <name> --client-id <client-id> --client-secret-file -`).
- `XURL_TOKEN_STORE` — token store path in place of `~/.xurl/auth.yml`, the same variable `xr` reads; the OAuth2 pending
  state sits beside it.
- `BIRD_ARCHIVE_DIR` — where the Markdown archive lives, ahead of `archive_dir` in `config.toml`; absolute or `~/`-led.
  See [File store](#file-store).
- `BIRD_SKILL_HOME` — stands in for `HOME` in every `bird skill` destination, as `XURL_SKILL_HOME` does for `xr`. A
  host's own config-directory variable (`CLAUDE_CONFIG_DIR`, `KIRO_HOME`, `OPENCODE_CONFIG_DIR`) wins over it; see
  [Agent-skill bundle](#agent-skill-bundle).
- `NO_COLOR=1` — strip ANSI color (industry-standard).

## Architecture

```text
bird (CLI + file store + intelligence) --> xdk-rs (embedded library: auth + HTTP) --> X API
```

- `src/main.rs` — thin binary entrypoint (≤30 LOC): SIGPIPE restore, tracing init, delegates to `bird::cli::run_argv()`.
- `src/lib.rs` — library root. `#[doc(hidden)]` on `bird::cli`; the library surface is internal test infrastructure
  only, not a public crate API.
- `src/cli/mod.rs` — clap derive definitions (`Cli`, `Command`, `CacheAction`, `WatchlistCommand`, `SkillAction`,
  `SchemaArgs`).
- `src/cli/runner.rs` — layered entrypoints: `run_argv()`, `run(args, &mut stdout, &mut stderr)`,
  `run_with_paths(args, &mut stdin, &mut stdout, &mut stderr, paths, env)`. Tests call `run_with_paths` directly with
  TempDir-backed `ResolvedPaths`, `std::io::empty()` for stdin, and `Vec<u8>` writers.
- `src/cli/dispatch.rs` — `fn run` top-level match plus shared dispatcher helpers (`command_is_api_hitting`,
  `require_confirmation`, `emit_dry_run`, `build_dry_run_url`, `clamp_limit`, `validate_auth_value`, etc.).
- `src/cli/argv.rs` — argv pre-scan helpers (`output_from_argv`, `explicit_output_from_argv`).
- `src/cli/clap_errors.rs` — clap → `BirdError` mapping for `try_parse_from`.
- `src/cli/commands/` — per-command modules: `login.rs`, `reads.rs` (Me), `bookmarks.rs`, `profile.rs`, `search.rs`,
  `thread.rs`, `watchlist.rs` (Fetch only — Add/Remove/List are pre-dispatched), `spend.rs`, `cache.rs`, plus `writes/`
  (the 13 write verbs share a single `execute` helper).
- `src/xurl_client/` — bird's seam over the embedded `xdk-rs` library. `mod.rs` declares the `XurlClient` trait (one
  generic `send_request`; bird renders every request itself) and `EmbeddedClient` (an `xdk::api::Client` plus the
  current-thread tokio runtime that blocks on its async calls); `mock.rs` is the `MockXurlClient` test double;
  `ConstructionStub` reports a client that failed to build.
- `src/store/` — the file store: `paths.rs` (the three locations and the archive-dir setting), `responses/` (per-account
  response files), `archive/` (Markdown notes and their merge), `ledger.rs` and `spend_view.rs` (the JSONL spend ledger
  and what `bird spend` computes from it), `report.rs` (what `bird cache stats` and doctor report), `status.rs` (the
  run's first store failure, warned once), `atomic.rs` (write-then-rename), `legacy.rs` (removes the 0.2.0 SQLite
  files), and `client/` (`BirdClient` over `XurlClient`, including the `embedded` seam).
- `src/bookmarks.rs`, `src/raw.rs`, `src/profile.rs`, `src/search.rs`, `src/thread.rs`, `src/watchlist/`, `src/spend/` —
  per-command handlers; streaming where the endpoint paginates.
- `src/doctor.rs` — diagnostic report (linked xdk-rs crate version, per-app auth state, per-command accepted vs.
  credentialed schemes, the file store).
- `src/cli/auth_scheme.rs` — `AuthType` enum (`OAuth2User`, `OAuth1`, `Bearer`, `None`); bird's request-time scheme
  identifier rendered to xdk's wire vocabulary by `store::client::embedded::auth_type_to_xurl_wire`.
- `src/output/` — `OutputConfig`, color helpers, ANSI sanitization, and the one emitter per envelope (`print_ok`,
  `print_dry_run`, `print_error`) taking an injected `&mut dyn Write`; `envelope.rs` holds `xr`'s envelope types and
  `hints.rs` the recovery-step chooser.
- `src/error/` — `BirdError`: the envelope reason and exit code of every failure, an `xdk::Error`'s taken from its
  `kind` and `exit_code`.
- `src/config.rs` — `ResolvedConfig`, `ResolvedPaths`, `EnvOverrides`, file permissions (0644 config; Unix-only
  `set_permissions`). `ResolvedConfig::load_with_paths(overrides, paths, env)` is the canonical injectable loader.
  `ResolvedPaths::token_store` and `EnvOverrides::xdk` carry the xdk-rs token store and environment: when a store is
  named (`XURL_TOKEN_STORE`, or a test's temporary one), nothing below the runner reads `$HOME` or the process
  environment for X credentials; with none, xdk opens its default store as `xr` does.
- `src/schema.rs` — input validation (`validate_username` strips `@`, enforces X charset).
- `src/schema_print.rs` — `bird schema` subcommand: prints embedded JSON Schema 2020-12 documents from `schema/`.
- `src/skill_install/` — `bird skill install <host>` / `update <host>` orchestrate a hardened `git clone --depth 1` of
  the [`brettdavies/bird-skill`](https://github.com/brettdavies/bird-skill) bundle repo. `skill.json` is the build-time
  manifest; `build.rs` codegens `SkillHost`, `KNOWN_HOSTS`, `resolve_host`, and each host's directory variables and
  legacy location from it. `destination.rs` resolves a host's directory from the `SkillEnv` the runner builds out of
  `EnvOverrides` (`HOME`, `BIRD_SKILL_HOME`, and the host variables, each captured once). `git.rs` builds and spawns the
  hardened clone, `update.rs` holds the update verb and the `--all` skip rule, and `render.rs` renders and writes the
  envelopes; `mod.rs` keeps the envelope types and the install verb.

## Transport dependency

bird does **not** implement HTTP or OAuth itself. It embeds [`xdk-rs`](https://github.com/brettdavies/xurl-rs)
(`use xdk::`), the X API library published from the xurl-rs repository; every API call goes through one
`xdk::api::Client` constructed per CLI invocation. xdk-rs is async on tokio and bird is synchronous, so `EmbeddedClient`
owns a current-thread runtime and blocks on each call. App credentials come from `CLIENT_ID` / `CLIENT_SECRET` env vars
or an app stored by the optional `xr auth apps add` command (xurl-rs's `xr` CLI shares the `~/.xurl` token store with
the embedded client). With neither, `bird login` refuses with `client-credentials-missing` and `bird doctor` reports the
same reason for login and every command that needs auth; both name the two ways to supply credentials
(`hints::client_credentials_missing` holds the text).

`bird login` calls `xdk::api::Client::oauth2_flow` with its own browser opener; bird never owns tokens.
`bird login --no-browser` (alias `--headless`) drives `remote_oauth2_step1` / `remote_oauth2_step2` so agents and
headless machines get the authorization URL on stdout and feed the redirect URL back on stdin. Both refuse an app with
no client credentials (`client-credentials-missing`, exit 2) before building an authorization URL, as `xr auth oauth2`
does. Token storage, refresh, and OAuth1 signing all live in xdk-rs.

## Output formats

`OutputConfig` (`src/output/mod.rs`) drives three formats, selected by `--output text|json|jsonl|ndjson` (default `text`
on a TTY, `json` when stderr is non-TTY) and overridable via `BIRD_OUTPUT`. bird's own reports (`doctor`, `cache stats`,
`spend`) print their human-readable text in text mode and under `--pretty` (`OutputConfig::report_as_text`), and one ok
document in a JSON mode. On the read commands `--pretty` enables formatted text output with ANSI color and OSC-8
hyperlinks; `--color never` (or `NO_COLOR=1`) strips color; `-q` / `--quiet` suppresses informational stderr
diagnostics.

Stdout success document on `--output json`, the ok envelope `xr` prints for a result it assembles. A list carries `data`
and `meta`; a single record puts its fields beside `status`; a message-shaped result is `message`:

```json
{"status": "ok", "data": [...], "meta": {...}}
{"status": "ok", "path": "...", "size_mb": 1.2, "healthy": true}
{"status": "ok", "message": "Added @jack to watchlist."}
```

Under `--dry-run` a write prints
`{"status": "dry_run", "would_succeed": true, "exit_code": 0, "command", "method", "url"}`, with `body` when the request
has one and `confirmation_required: true` without `--force`. The reads that print X's response body (`me`, `profile`,
`bookmarks`, `search`, `get`) keep that body as their document.

Stderr error envelope under a structured output mode, the one `xr` prints:

```json
{"status": "error", "reason": "rate-limited", "exit_code": 3, "message": "...",
 "next_step": {"action": "wait-and-retry", "docs": "https://docs.x.com/resources/fundamentals/rate-limits"},
 "retry_after_secs": 42, "retry_at": "2026-10-22T21:00:00Z"}
```

`reason` is `xdk::Error::kind` for an X API or library failure and `xr`'s spelling for bird's own refusals, plus bird's
`config-error`, `cache-miss`, and `cancelled`. `next_step` appears when a recovery step exists: its `action` is xdk-rs's
`NextAction`, with a `command` safe to run verbatim, a `template` to finish first, or `docs`. A credential failure's
step comes from the token store the run loaded (`sign-in` with `bird login`, `select-app`, `register-app`,
`inspect-store`). Optional fields follow `xr`'s envelope: `command` (the offending value), `suggestion`, the
auth-mismatch fields, and `retry_after_secs` / `retry_at` on a rate limit with a known reset. Text mode prints the same
step as a hint line. `--output json` is indented; `--raw`, `jsonl`, and `ndjson` print one line. Help and version print
plain text, or an ok envelope carrying `data.help` / `data.version` when JSON is asked for by flag.

## JSON Schema documents

`bird schema` takes `xr schema`'s forms and prints JSON Schema 2020-12 documents: `envelope` (every `ok`, `dry_run`, and
`error` document, generated from the envelope types) and one per command, named by the command: `bookmarks`,
`cache-stats`, `doctor`, `profile`, `search`, `skill-install`, `skill-update`, `spend`, `thread`, `watchlist-fetch`,
`watchlist-list`. A schema prints as itself, never wrapped. The documents carry no `$id`.

```bash
bird schema envelope              # the envelope schema
bird schema thread                # one command's output
bird schema --list                # every name and title; one ok document under --output json
bird schema --all                 # every schema in one object keyed by name
```

Bare `bird schema` is a usage error naming those forms.

## File store

bird keeps no database. What it reads lands in three places:

| Kind           | Default (Linux)                   | Holds                                                |
| -------------- | --------------------------------- | ---------------------------------------------------- |
| Archive        | `~/.local/share/bird/archive/`    | One Markdown note per post, user, bookmark, and like |
| Response files | `~/.cache/bird/responses/`        | Each GET's 2xx body, per account, for the UTC day    |
| Spend ledger   | `~/.local/share/bird/spend.jsonl` | One JSONL record per request sent or answered        |

The archive moves with `BIRD_ARCHIVE_DIR`, then `archive_dir` in `config.toml`; a relative value is a `config-error`.
Notes are named by X id (`posts/<id>.md`, `users/<id>.md`, `bookmarks/<account id>/<post id>.md`,
`likes/<account id>/<post id>.md`); a later fetch rewrites bird's frontmatter keys and the region between
`<!-- bird:begin -->` and `<!-- bird:end -->`, and keeps every other key and line. A GET repeated the same UTC day for
the same account (the app and the user it goes out under) is answered from its response file and prints the same bytes;
direct-message, usage, and media-upload reads are never stored. A write clears its account's response files. A store
failure prints one `[store] warning` line and never changes a command's result. `bird cache stats` and doctor's `cache`
section report all three locations; `bird cache clear` removes response files only.

On its first run bird removes a 0.2.0 `bird.db` and an older `cache.db`, each with its `-wal` and `-shm` files, from
`~/.config/bird/`, and says so once per store.

Three cache modes:

- `--refresh` — send the read, then store the fresh response.
- `--no-cache` — read and write nothing; a write still clears its account's response files.
- `--cache-only` — answer from a same-day response file only; never send an API request. Errors with
  `reason: "cache-miss"` (exit_code 1) when none is stored today (UTC) for the request and account: the message names
  the request's method and path (`CacheMiss` in `src/error/mod.rs` holds the text), and `next_step.template` is the same
  command without `--cache-only`. Write commands (`tweet`, `like`, `follow`, etc.) reject `--cache-only`.

## Exit codes

| Code | Meaning                                                                                     |
| ---- | ------------------------------------------------------------------------------------------- |
| 0    | Success                                                                                     |
| 1    | Command error: an API status with no code of its own (400, 403, 5xx), a refused value, or a |
|      | `confirmation-required` write op without `--force`/`--yes`                                  |
| 2    | Usage: `invalid-args`, `unknown-command`, or an auth-scheme mismatch                        |
| 3    | Rate limited (HTTP 429)                                                                     |
| 4    | Not found (HTTP 404)                                                                        |
| 5    | Network or filesystem failure: the request never got an HTTP response                       |
| 77   | Auth error: missing or refused credential (HTTP 401), or a token store that cannot be read  |
| 78   | Config error (`EX_CONFIG`): bird's own config file or paths                                 |
| 130  | Cancelled: SIGINT arrived while an X API call was in flight (`reason: "cancelled"`)         |
| 143  | Cancelled: SIGTERM arrived while an X API call was in flight (`reason: "cancelled"`)        |

Every code xdk-rs classifies (1, 2 for an auth-scheme mismatch, 3, 4, 5, 77) is `xdk::Error::exit_code`, and the
envelope's `reason` is `xdk::Error::kind`, the same `reason` `xr` reports. The 77/78 split follows the BSD `sysexits.h`
convention and lets agent harnesses distinguish "fix your tokens" from "fix your config" from "API blew up." A write
without `--force` under `--no-interactive` exits 1 with `confirmation-required`, as `xr`'s does, instead of hanging; a
prompt answered no exits 0 having done nothing. SIGTERM or SIGINT during an X API call drops the call and reports
`cancelled` with 128 plus the signal number; with no call in flight the signal takes its default action and the process
stops with no envelope.

## Token and file permissions

bird does **not** store API tokens (xdk-rs does, in the `~/.xurl` token store `xr` shares). bird only owns:

- `~/.config/bird/config.toml` — minimal user config (`username`, `watchlist`, `archive_dir`). Created at mode 0644.
- The [file store](#file-store): every file bird writes there is 0600 and every directory it creates 0700, because the
  archive holds the user's bookmarks.

Permission enforcement lives in `src/config.rs` and `src/store/atomic.rs` behind `#[cfg(unix)]`.

## Agent-skill bundle

The skill bundle lives in a dedicated repo: [`brettdavies/bird-skill`](https://github.com/brettdavies/bird-skill).
`bird skill install <host>` runs a hardened `git clone --depth 1` of that repo into the host's canonical skills
directory; `bird skill update <host>` removes the destination, and any copy at the host's legacy location, and
re-clones. The supported hosts, their target paths, the variables that move them, and their legacy locations are the
single source of truth in `src/skill_install/skill.json`, in `xr`'s shape; `build.rs` codegens the `SkillHost` enum,
`KNOWN_HOSTS`, the `resolve_host` / `host_envelope_str` lookup, `config_dir_env` / `base_dir_env` /
`legacy_destination`, and the `Environment:` block of `bird skill --help` at build time.

| Host          | Destination                      | Host config-dir variable | Base-dir variable | Legacy location        |
| ------------- | -------------------------------- | ------------------------ | ----------------- | ---------------------- |
| `claude_code` | `~/.claude/skills/bird`          | `CLAUDE_CONFIG_DIR`      |                   |                        |
| `codex`       | `~/.agents/skills/bird`          |                          |                   | `~/.codex/skills/bird` |
| `cursor`      | `~/.cursor/skills/bird`          |                          |                   |                        |
| `factory`     | `~/.factory/skills/bird`         |                          |                   |                        |
| `kiro`        | `~/.kiro/skills/bird`            | `KIRO_HOME`              |                   |                        |
| `opencode`    | `~/.config/opencode/skills/bird` | `OPENCODE_CONFIG_DIR`    | `XDG_CONFIG_HOME` |                        |

A destination resolves as `xr`'s does, first match wins:

1. The host's config-directory variable replaces the host directory: `CLAUDE_CONFIG_DIR=/x` installs to
   `/x/skills/bird`.
2. `BIRD_SKILL_HOME` replaces `HOME` for every host: `BIRD_SKILL_HOME=/h` installs Codex to `/h/.agents/skills/bird`.
3. The host's base-directory variable replaces its prefix: `XDG_CONFIG_HOME=/c` installs opencode to
   `/c/opencode/skills/bird`.
4. `HOME`. With none of these set, the verb reports `home-not-set` (exit 1).

An empty value counts as unset. Each variable is read once, by the runner, into `EnvOverrides`; tests inject them there.
Codex still reads `~/.codex/skills`, so a copy found there (under `BIRD_SKILL_HOME`, else `HOME`) is named as
`legacy_install_dir`: `skill install` leaves it in place, and `skill update` removes it along with the destination.

Usage:

```bash
bird skill install claude_code              # clone into ~/.claude/skills/bird
bird skill install claude_code --dry-run    # print the planned `git clone` without spawning git
bird skill install --all                    # install into every supported host
bird skill update claude_code               # remove destination + re-clone
bird skill update --all                     # refresh every host that has a copy; skip the rest
```

Without `<host>` or `--all`, `bird skill install` returns exit 2 (a `missing-host` envelope) and lists the supported
hosts. Output under `--output json` is the install/update envelope (`action`, `host`, `install_dir`, `command_preview`,
`destination_status`, `status`, `exit_code`, optional `reason`, `legacy_install_dir`, and `next_step`). Under `--all`
both verbs print one envelope (`action`, `status`, `exit_code`) whose `installations` array holds one of those per host.

`bird skill update --all` refreshes only the hosts holding a copy, at the destination or the legacy location, as
`xr skill update --all` does. Each other host is reported with `status: "skipped"`, `reason: "not-installed"`,
`exit_code: 0`, and a `next_step` that runs `bird skill install <host>`; installing everywhere is `skill install --all`.

The clone is hardened: `credential.helper=`, `core.askPass=`, `protocol.allow=never`, `protocol.https.allow=always`,
`http.followRedirects=false` are applied via `-c key=value`; `GIT_SSH`, `GIT_SSH_COMMAND`, `GIT_PROXY_COMMAND`,
`GIT_ASKPASS`, and `GIT_EXEC_PATH` are stripped from the spawn environment; and `GIT_CONFIG_GLOBAL=/dev/null`,
`GIT_CONFIG_SYSTEM=/dev/null`, `GIT_TERMINAL_PROMPT=0` block user-config rewriting and credential prompts. Mirrors the
`xurl-rs` / `agentnative-cli` pattern.

## Quality bar

- `cargo fmt --all --check` — clean, edition 2024 (`rustfmt.toml` pins `style_edition = "2024"`).
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test --all` — full unit + CLI smoke + transport integration suites green.
- `cargo deny check` — advisories, licenses, bans, sources all clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` — clean.
- MSRV: **1.94**. The toolchain channel pinned in `rust-toolchain.toml` is the supply-chain anchor; rustup verifies
  component SHA256s from the distribution manifest, making the pin effectively a SHA pin.
- No `unwrap()` in production code paths.
- `bash scripts/lint-stdio.sh` — no output macro (`println!`, `eprintln!`, and kin) in `src/` outside `src/main.rs` and
  `src/error/fatal.rs`; output goes to an injected writer.
- `bash scripts/check-workflow-concurrency.sh` — every workflow concurrency group leads with a literal token.

The hook pair at `scripts/hooks/` shares one library (`_lib.sh`). `pre-commit` checks the staged files: rustfmt,
shellcheck, `shfmt -i 2 -ci -bn`, actionlint, concurrency-group namespacing, and the core-env guard. `pre-push` mirrors
CI over the tracked set: fmt, clippy, test, MSRV, rustdoc with `-D warnings`, deny, the output-discipline check
(`scripts/lint-stdio.sh`), the Windows libc grep and cross-clippy, plus the same file linters. It runs the Rust battery
only when the push reaches a cargo input. `tests/hook_ci_parity.rs` names the CI job behind every pre-push step. Set
`git config core.hooksPath scripts/hooks` once per clone.

## Testing

- Unit tests inline in each module. Exit-code coverage for the `xdk::Error → BirdError` mapping (78 / 77 / 1 / 3 / 4 /
  5) lives in `src/error/mod.rs::tests`.
- CLI smoke tests in `tests/cli_smoke.rs` exercise the clap surface, exit codes, and JSON envelope shape via the
  in-process `run_with_paths` entry point.
- `tests/json_envelope.rs`, `tests/envelope_consistency.rs`, `tests/schema_parity.rs` lock the JSON envelope shape and
  ensure the embedded `schema/` documents stay in sync with the runtime emitters.
- `tests/doctor_schema_runtime.rs` round-trips a fixture `DoctorReport` through `serde_json::to_value` and asserts every
  runtime-emitted key matches `schema/doctor.schema.json`, plus a sentinel-string negative check that no credential
  value can leak into the JSON output.
- `tests/diag_quiet_lazy_eval_test.rs` regression-locks the R24 zero-allocation `if !quiet` diag guard via a
  `PanicOnWrite` writer backed by `ConstructionStub`.

Bird tests only the bird/xdk boundary via `MockXurlClient`; xdk-rs owns the X API HTTP shapes upstream. No bird-side
wiremock; no bird-side real-API integration suite.

## Releasing

See [`RELEASES.md`](RELEASES.md) for the operational runbook, [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) for the
pre-cut go/no-go checklist, [`RELEASES-POSTFLIGHT.md`](RELEASES-POSTFLIGHT.md) for the post-tag verification, and
[`RELEASES-RATIONALE.md`](RELEASES-RATIONALE.md) for the why behind every rule. The short version: feature branch → PR
to `dev` (squash) → `dev`'s tree overlaid onto `release/v<version>` cut from `main` → PR to `main` (squash) → annotated
tag push triggers `release.yml` → `scripts/sync-dev-after-release.sh` backports the bookkeeping to `dev`.

## Documented solutions

`docs/solutions/` is a symlink to `~/dev/solutions-docs/`, a shared, searchable archive of past solutions and best
practices organized by category with YAML frontmatter (`module`, `tags`, `problem_type`). Search with
`qmd query "<topic>" --collection solutions` before implementing or debugging in a documented area; the corpus crosses
repos and already captures known pitfalls.

## References

- [`README.md`](README.md) — install paths, command surface, agent usage.
- [`RELEASES.md`](RELEASES.md) — release runbook (cut-a-release steps only).
- [`RELEASES-RATIONALE.md`](RELEASES-RATIONALE.md) — the WHY behind release rules.
- [`RELEASES-PREFLIGHT.md`](RELEASES-PREFLIGHT.md) — bird-specific pre-cut checklist.
- [`RELEASES-POSTFLIGHT.md`](RELEASES-POSTFLIGHT.md): post-tag verification of the publish chain.
- [`docs/CLI_DESIGN.md`](docs/CLI_DESIGN.md) — auth requirements, doctor, error design.
- [`docs/DEVELOPER.md`](docs/DEVELOPER.md) — build, architecture, project layout.
- [xurl-rs](https://github.com/brettdavies/xurl-rs) — home of `xdk-rs`, the upstream transport dependency, and the `xr`
  CLI.
