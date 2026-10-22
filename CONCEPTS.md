# Concepts

Shared domain vocabulary for this project: entities, named processes, and status concepts with project-specific meaning.
Seeded with core domain vocabulary; it accretes as learnings are processed, and direct edits are fine. Glossary only,
not a spec or catch-all.

## Store and cache

### Entity store

The SQLite database at `~/.config/bird/bird.db` that sits in front of every read command. It keeps the posts, users, and
bookmarks the API returned as individual rows, so a later read that asks for an entity the store already holds is
answered locally instead of billed again.

### Raw response cache

The content-addressed part of the entity store for endpoints whose responses are not split into entities. A request's
canonical form is its key, so an identical request reads the stored body.

### Cache mode

The per-invocation rule for how a read uses the entity store: the default reads and writes it, `--refresh` skips the
read but writes the fresh response, `--no-cache` neither reads nor writes, and `--cache-only` reads and never sends a
request. Write commands accept no cache mode that would skip the API.

### Cost ledger

The usage table that records an estimated cost for every API call bird sends. `bird usage --local` reports from it
alone; `bird usage` also syncs the actual figures X reports, so estimate and bill sit side by side.

## Transport

### Embedded client

The one `xdk::api::Client` bird builds per invocation, with the current-thread runtime that blocks on its async calls.
It is the only path to the X API; bird starts no `xr` or `xurl` process.

### Token store

The credential file xdk-rs owns (`~/.xurl` by default, or `XURL_TOKEN_STORE`), shared with the `xr` CLI. bird reads it
through the embedded client and never writes tokens itself.

## Output

### Envelope

The JSON wrapper every structured output takes: a success envelope on stdout (`data`, `meta`) and an error envelope on
stderr (`error`, `kind`, `exit_code`, `message`). Each has a JSON Schema under `schema/` that `bird schema` prints.

### Writer injection

The rule, enforced as R20 by `guard-r20.yml` and the pre-push hook, that every user-visible line goes through an
injected `&mut dyn Write` or an `OutputConfig` method. Only the binary shim and the fatal-error chokepoint write to the
process streams directly, which is what lets tests capture every byte bird prints.

## Commands

### Watchlist

The list of usernames in `config.toml` that `bird watchlist fetch` checks for recent activity, one search per user.

### Thread reconstruction

Rebuilding a conversation from one post: the root post read plus a `conversation_id` search, ordered into a tree.

### Skill bundle

The agent instructions in the `bird-skill` repository that `bird skill install <host>` clones into an agent host's
skills directory.
