# CLI design

This document describes how auth requirements, errors, and the doctor command are designed and how they share a single
source of truth.

## Doctor: full and scoped reports

- **`bird doctor`** — Full report: auth state, effective config (with source), and availability plus reasons for all
  commands (login, me, bookmarks, profile, search, thread, watchlist, spend).
- **`bird doctor <command>`** — Scoped report: same auth and config, but the commands section lists only the given
  command (e.g. `bird doctor me`). Lets humans and agents ask “what do I need to run `bird me`?” without parsing the
  full JSON.

Use **`--pretty`** for a human-readable summary in either case.

We use a single entry point (`bird doctor` and optionally a command name) rather than `bird me --doctor` or
`bird me doctor` so that “what’s wrong / what do I need?” is discoverable in one place and we avoid duplicating a flag
or nested subcommands on every command.

## Requirements and errors

Each command has defined **auth requirements** derived from the X API OpenAPI spec:

- **me**: OAuth 2.0 user token or OAuth 1.0a (not app-only bearer).
- **bookmarks**: OAuth 2.0 user token only.
- **profile**: any of bearer, OAuth 1.0a, or OAuth 2.0 user. Looks up a user by username via
  `GET /2/users/by/username/{username}`.
- **search**: any of bearer, OAuth 1.0a, or OAuth 2.0 user.
- **thread**: any of bearer, OAuth 1.0a, or OAuth 2.0 user. Reconstructs a conversation thread via
  `GET /2/tweets/{id}` + `GET /2/tweets/search/recent?query=conversation_id:{id}`.
- **watchlist fetch**: any of bearer, OAuth 1.0a, or OAuth 2.0 user (same auth as search — runs `from:<username>` search
  queries).
- **watchlist add/remove/list**: no auth required (local config.toml operations).
- **spend**: no auth required (reads the local SQLite usage table).
- **spend --sync**: Bearer token only (fetches `GET /2/usage/tweets` from X API).
- **login**: N/A (runs the OAuth2 PKCE browser flow through the embedded `xdk-rs` client; `--no-browser` runs the
  headless two-step flow).

When a command fails, the error envelope (`xr`'s: `status`, `reason`, `exit_code`, `message`) carries a `next_step`
whenever a recovery step exists, and text mode prints the same step as a hint line under the error. A missing credential
picks its step from the token store the run loaded:

1. The store could not be read: `inspect-store`, run `bird doctor`.
2. Another app is signed in: `select-app`, the same invocation with `--app <name>`.
3. The active app has client credentials and no token: `sign-in`, run `bird login` (`bird login --no-browser` under
   structured output).
4. Another app has client credentials: `select-app`, sign in against it.
5. No app has any: `register-app`, a template for `xr auth apps add`.

Every other step comes from xdk-rs (`xdk::Error::next_action` and `docs_url`): `wait-and-retry` on a rate limit that
named its reset, `retry` on a 5xx or a dropped connection, `fix-input` on a refused value, `enroll-app` when X refuses
the app, `report-issue` for a fault in the library. A usage error points at the command's `--help`, and a write refused
for want of `--force` carries the forced invocation as a `confirm` template.

The same requirement definitions drive **doctor** availability and reason strings (e.g. “run bird login or set … Or set
OAuth 1.0a env”), so execution, error messages, and doctor stay consistent.

## Source of truth

Auth requirements for each command are defined in a **central module** (`src/requirements.rs`). That module is used by:

- **Execution** — “resolve token for command X” tries accepted auth types in a defined order and returns a structured
  “auth required” error with hints when none work.
- **Errors** — Subcommand failure formatting uses the same hint strings.
- **Doctor** — Availability and reasons are computed from the same requirements.

The mapping is hand-maintained. Raw commands are treated as “any auth” with a generic hint.
