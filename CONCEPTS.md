# Concepts

Shared domain vocabulary for this project: entities, named processes, and status concepts with project-specific meaning.
Seeded with core domain vocabulary; it accretes as learnings are processed, and direct edits are fine. Glossary only,
not a spec or catch-all.

## Store and cache

### File store

The files bird keeps in place of a database: the archive, the response files, and the cost ledger. A failure in any of
them prints one `[store] warning` and never changes a command's result.

### Archive

One Markdown note per post, user, bookmark, and like bird has read, named by X id under a directory the user can point
at a notes vault (`archive_dir`, `BIRD_ARCHIVE_DIR`). A later observation rewrites only bird's frontmatter keys and the
region between `<!-- bird:begin -->` and `<!-- bird:end -->`; every other key and line is the user's.

### Response file

A GET's whole 2xx body, stored per account and keyed by the method and normalized URL. It answers the same request for
the same account until the UTC day ends, so a repeat read prints what the first one printed and is not billed again. A
write clears its account's response files; direct-message, usage, and media-upload reads are never stored.

### Account

The app a request goes out under and the user it is sent as (the requested username, else the app's default user; no
user for an app-only request). Response files are kept per account, so one account's answer never serves another.

### Cache mode

The per-invocation rule for how a read uses the response files: the default reads and writes them, `--refresh` skips the
read but stores the fresh response, `--no-cache` neither reads nor writes, and `--cache-only` reads and never sends a
request. Write commands accept no cache mode that would skip the API.

### Cost ledger

`spend.jsonl`, one record per request bird sends or answers, with its estimated cost. `bird spend` reports from it
alone; `bird spend --sync` also appends the actual figures X reports, so estimate and bill sit side by side.

## Transport

### Embedded client

The one `xdk::api::Client` bird builds per invocation, with the current-thread runtime that blocks on its async calls.
It is the only path to the X API; bird starts no `xr` or `xurl` process.

### Token store

The credential file xdk-rs owns (`~/.xurl` by default, or `XURL_TOKEN_STORE`), shared with the `xr` CLI. bird reads it
through the embedded client and never writes tokens itself.

## Output

### Envelope

The JSON document every structured result bird assembles takes, in `xr`'s shape and tagged by `status`: `ok` and
`dry_run` on stdout, `error` on stderr (`reason`, `exit_code`, `message`, `next_step`). `bird schema envelope` prints
its JSON Schema.

### Writer injection

The rule, enforced by `scripts/lint-stdio.sh` in CI's `Output discipline` job and in the pre-push hook, that every
user-visible line goes through an injected `&mut dyn Write` or an `OutputConfig` method. Only the binary shim and the
fatal-error chokepoint write to the process streams directly, which is what lets tests capture every byte bird prints.

## Commands

### Watchlist

The list of usernames in `config.toml` that `bird watchlist fetch` checks for recent activity, one search per user.

### Thread reconstruction

Rebuilding a conversation from one post: the root post read plus a `conversation_id` search, ordered into a tree.

### Skill bundle

The agent instructions in the `bird-skill` repository that `bird skill install <host>` clones into an agent host's
skills directory.
