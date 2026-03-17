# CLI design

This document describes how auth requirements, errors, and the doctor command are designed and how they share a single source of truth.

## Doctor: full and scoped reports

- **`bird doctor`** — Full report: auth state, effective config (with source), and availability plus reasons for all commands (login, me, bookmarks, profile, search, thread, watchlist, usage, get, post, put, delete).
- **`bird doctor <command>`** — Scoped report: same auth and config, but the commands section lists only the given command (e.g. `bird doctor me`). Lets humans and agents ask “what do I need to run `bird me`?” without parsing the full JSON.

Use **`--pretty`** for a human-readable summary in either case.

We use a single entry point (`bird doctor` and optionally a command name) rather than `bird me --doctor` or `bird me doctor` so that “what’s wrong / what do I need?” is discoverable in one place and we avoid duplicating a flag or nested subcommands on every command.

## Requirements and errors

Each command has defined **auth requirements** derived from the X API OpenAPI spec:

- **me**: OAuth 2.0 user token or OAuth 1.0a (not app-only bearer).
- **bookmarks**: OAuth 2.0 user token only.
- **profile**: any of bearer, OAuth 1.0a, or OAuth 2.0 user. Looks up a user by username via `GET /2/users/by/username/{username}`.
- **search**: any of bearer, OAuth 1.0a, or OAuth 2.0 user (same as raw commands).
- **thread**: any of bearer, OAuth 1.0a, or OAuth 2.0 user. Reconstructs a conversation thread via `GET /2/tweets/{id}` + `GET /2/tweets/search/recent?query=conversation_id:{id}`.
- **watchlist check**: any of bearer, OAuth 1.0a, or OAuth 2.0 user (same auth as search — runs `from:<username>` search queries).
- **watchlist add/remove/list**: no auth required (local config.toml operations).
- **usage**: no auth required (reads local SQLite usage table).
- **usage --sync**: Bearer token only (fetches `GET /2/usage/tweets` from X API).
- **login**: N/A (delegates to `xurl auth oauth2` for OAuth2 PKCE browser flow).
- **get / post / put / delete** (raw): any of bearer, OAuth 1.0a, or OAuth 2.0 user.

When a command fails because auth is missing or the wrong type, the CLI prints:

1. A one-line summary, e.g. `me failed: no valid auth for this command.`
2. A **requirements block**: for each auth type that the command supports, what the user must do:
   - **OAuth 2.0 (user):** Run `bird login` or set `X_API_ACCESS_TOKEN` (and optionally `X_API_REFRESH_TOKEN`).
   - **OAuth 1.0a** (when supported): Set `X_API_CONSUMER_KEY`, `X_API_CONSUMER_SECRET`, `X_API_OAUTH1_ACCESS_TOKEN`, `X_API_OAUTH1_ACCESS_TOKEN_SECRET`.
   - **Bearer** (when supported): Set `X_API_BEARER_TOKEN`.

The same requirement definitions drive **doctor** availability and reason strings (e.g. “run bird login or set … Or set OAuth 1.0a env”), so execution, error messages, and doctor stay consistent.

## Source of truth

Auth requirements for each command are defined in a **central module** (`src/requirements.rs`). That module is used by:

- **Execution** — “resolve token for command X” tries accepted auth types in a defined order and returns a structured “auth required” error with hints when none work.
- **Errors** — Subcommand failure formatting uses the same hint strings.
- **Doctor** — Availability and reasons are computed from the same requirements.

The mapping is hand-maintained. Raw commands are treated as “any auth” with a generic hint.
