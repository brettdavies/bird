# Brainstorm: Wrap xurl vs Maintain Native API Client

**Date**: 2026-03-12
**Status**: Completed
**Decision**: Architecture A — Full Wrap (xurl as sole transport layer)
**Trigger**: X team released `xurl` (v1.0.3) — official CLI for X API with OAuth2 PAYG support

---

## Context

Bird is a Rust CLI that talks directly to the X API via `reqwest`. It implements OAuth2 PKCE,
OAuth1, Bearer auth, entity-level caching, cost tracking, and convenience commands. The X team
now ships `xurl`, an official Go CLI that covers auth + raw HTTP + a growing set of shortcut
commands. The question: should bird wrap `xurl` for transport/auth and focus on its value-add
layer?

---

## Coverage Comparison

### xurl Has (that bird doesn't)

| Category | Commands |
|----------|----------|
| **Write ops** | `post`, `reply`, `quote`, `repost`, `unrepost`, `delete` |
| **Social graph** | `follow`, `unfollow`, `block`, `unblock`, `mute`, `unmute` |
| **Engagement** | `like`, `unlike`, `bookmark`, `unbookmark` |
| **DMs** | `dm`, `dms` |
| **Feeds** | `timeline`, `mentions`, `likes`, `followers`, `following` |
| **Media** | `media upload` (chunked), `media status` |
| **Webhooks** | `webhook start` (ngrok integration) |
| **Multi-app** | `auth apps add/list/remove/update`, `auth default`, `--app` flag |
| **Shell** | `completion` (bash/zsh/fish/powershell) |

### bird Has (that xurl doesn't)

| Category | Feature |
|----------|---------|
| **Entity store** | SQLite entity-level cache with dedup, freshness, pruning |
| **Batch splitting** | Only fetch stale entity IDs from API, serve rest from store |
| **Cost tracking** | Per-request cost estimation, `usage` command with daily breakdown |
| **Cost sync** | `usage --sync` fetches actual API usage from `/2/usage/tweets` |
| **Thread** | Reconstructs full conversation trees from tweet ID |
| **Watchlist** | Monitor accounts, check recent activity |
| **Search enhancements** | Noise reduction, client-side RT filter, cross-page dedup |
| **Doctor** | Full diagnostic: auth state, per-command availability, store health |
| **Cache management** | `cache stats`, `cache clear`, configurable max size, auto-pruning |
| **Offline mode** | `--cache-only` serves from store without API calls |
| **Streaming bookmarks** | Page-by-page NDJSON (not collected in memory) |

### Overlap (both have)

| Feature | bird | xurl |
|---------|------|------|
| OAuth2 PKCE | Yes | Yes |
| OAuth1 | Yes | Yes |
| Bearer token | Yes | Yes |
| Multi-account | Yes (`--account`) | Yes (`--username`, `--app`) |
| Token storage | `~/.config/bird/tokens.json` | `~/.xurl` |
| Raw HTTP | `get/post/put/delete <path>` | `xurl -X METHOD /path` |
| Search | `search` (paginated, filtered) | `search` (basic, max 100) |
| Bookmarks | `bookmarks` (streaming) | `bookmarks` (single page) |
| Profile | `profile @user` | `user @user` |
| Me | `me` | `whoami` |
| Pretty JSON | `--pretty` | Default (colored) |

---

## Research Findings

### Output Parsing: VIABLE

`fatih/color` (used by xurl) auto-disables ANSI escape codes when stdout is not a TTY.
When bird captures xurl's stdout via subprocess, the output is clean `json.MarshalIndent` —
directly deserializable by serde. No `--no-color` flag needed.

Critically, xurl outputs the **raw X API JSON response** — it does not transform or reshape
the data. Bird already parses X API JSON. The coupling is to the X API response format
(which bird must handle regardless), not to xurl-specific formatting.

### Token Store Format: YAML (v1.0+)

The GitHub repo (pre-v1.0.3) used JSON. The installed v1.0.3 uses **YAML**:

```yaml
apps:
    default:
        client_id: "abc123"
        client_secret: "xyz789"
        default_user: BrettDavies
        oauth2_tokens:
            BrettDavies:
                type: oauth2
                oauth2:
                    access_token: "dmNfWW..."
                    refresh_token: "VnNxek..."
                    expiration_time: 1771319325
        oauth1_token:           # optional, per-app
            type: oauth1
            oauth1:
                access_token: "..."
                token_secret: "..."
                consumer_key: "..."
                consumer_secret: "..."
        bearer_token:           # optional, per-app
            type: bearer
            bearer: "AAAA..."
    prod-app:
        client_id: "def456"
        client_secret: "uvw321"
default_app: default
```

Note: The format migrated from JSON (v0.5) to YAML (v1.0) with auto-migration.
xurl reads legacy JSON and rewrites as YAML on first load.

With Architecture A (full wrap), bird never reads `~/.xurl` directly — irrelevant.

### GitHub Repo vs npm Package: Same Repo

The local github-stars clone was 13 commits behind. The Feb 2026 overhaul
(`8371a07`) added shortcut commands, multi-app, YAML config, npm/brew distribution.
Same repo, same program, massive feature jump from v0.5 to v1.0.

### Multi-App Support: NEW in v1.0.3

xurl supports registering multiple X API apps (`auth apps add NAME`),
setting a default (`auth default`), and per-request override (`--app NAME`).
With full wrap, bird inherits this for free via `--app` flag passthrough.

### Pagination: xurl Has None

xurl search caps at `--max-results 100` (single page). No `--pages`, no
`next_token` handling. Bird's multi-page search with dedup remains unique.
Bird constructs URLs with `next_token` query params and passes to xurl's
raw mode (`xurl "/2/tweets/search/recent?query=...&next_token=..."`).

---

## Architectures Considered

### Architecture A: Full Wrap — xurl as Sole Transport (CHOSEN)

Bird delegates ALL X API HTTP requests to xurl. Bird owns the intelligence layer
(entity store, caching, cost tracking, UX). xurl owns the transport layer (auth, HTTP).

```text
bird search "query"
  1. Check entity store for fresh entities (no xurl call if all fresh)
  2. Construct URL with only stale/missing IDs + field params
  3. Call: xurl "/2/tweets/search/recent?query=...&tweet.fields=..." (subprocess)
  4. Parse JSON from stdout (raw X API response, no color when piped)
  5. Decompose entities, store in SQLite, track cost
  6. Apply filters, dedup, output to user

bird post "Hello world!"
  -> xurl post "Hello world!" (subprocess, capture stdout)
  -> Log to usage table for cost tracking
  -> Display response to user
```

### Architecture B: Hybrid — Share Auth, Native Reads, Passthrough Writes

Bird reads `~/.xurl` for tokens, uses reqwest for reads, delegates writes to xurl.

### Architecture C: Stay Native — Cherry-Pick Ideas Only

Bird stays fully native Rust. Implement all write ops natively.

---

## Why Architecture A Wins

### The Interface Boundary Argument

Today:

```text
Bird (auth + transport + intelligence + UX) -> X API
```

With full wrap:

```text
Bird (intelligence + UX) -> xurl (auth + transport) -> X API
```

When the X API changes (new OAuth scopes, endpoint changes, auth flow updates),
only the xurl layer needs updating — `npm update -g @xdevplatform/xurl`. Bird's
entity decomposition, caching, cost tracking, thread reconstruction, search
enhancements — none of that touches the transport.

**The cleaner the boundary, the easier the maintenance.**

### Why the Entity Store Still Works

Bird's entity store checks for fresh entities **before** calling xurl. On cache hit,
xurl is never invoked — zero subprocess overhead on the hot path. The store only
needs the raw X API JSON response to decompose, and that's exactly what xurl outputs
when piped (clean JSON, no ANSI codes).

- **Batch splitting**: Bird constructs URL with only stale IDs, passes to xurl
- **Pagination**: Bird passes `next_token` as query param in URL
- **Fields params**: Bird's `fields.rs` builds query strings, embedded in URL
- **Response parsing**: xurl outputs raw X API JSON — bird already parses this

### Pros

| Pro | Detail |
|-----|--------|
| **Clean interface boundary** | Bird = intelligence. xurl = transport. Single responsibility. |
| **API change resilience** | X changes OAuth/endpoints/auth? Update xurl binary. Bird unchanged. |
| **Delete ~600 lines of auth code** | Remove `auth.rs`, `login.rs`, PKCE, token refresh, OAuth1 signing |
| **Drop reqwest + oauth dependencies** | Smaller binary, fewer transitive deps, faster compile |
| **No token file coupling** | Never parse `~/.xurl`. xurl owns auth completely. |
| **Multi-app for free** | Pass `--app NAME` to xurl. Bird never implements multi-app. |
| **All 30+ commands free** | Write ops, social graph, DMs, media, webhooks — just passthrough |
| **Cache hits skip xurl entirely** | Hot path has zero subprocess overhead |
| **Consistent interface** | Reads and writes use same subprocess mechanism. No bifurcated paths. |
| **Reversible** | If xurl dies, swap back to reqwest. The boundary makes the swap clean. |

### Cons

| Con | Severity | Mitigation |
|-----|----------|------------|
| **Subprocess overhead (~50-100ms/call)** | Low | Only on cache misses. CLI users won't notice. |
| **No TCP connection reuse** | Medium | 10-page search: ~10 extra TLS handshakes. Adds ~1-1.5s to an op that already takes several seconds with rate limiting. Acceptable for CLI. |
| **Error handling is string-based** | Low | xurl exits non-zero on failure. Error JSON in stdout is structured — serde parses it fine. |
| **Can't control HTTP details** | Low | No custom User-Agent, no per-request timeout. Unlikely to matter for CLI. |
| **External binary dependency** | Low | `bird doctor` checks for xurl. Install via npm/brew/curl. |
| **xurl could be abandoned** | Low | Actively developed (4 releases Feb 2026). If it dies, the clean boundary makes swapping back to reqwest straightforward. |

### Why A over B (Hybrid)

Architecture B (native reads, passthrough writes) was the initial recommendation.
On reflection, it creates a worse system:

- **Two transport mechanisms** to maintain (reqwest for reads, subprocess for writes)
- **Token file coupling** — bird must parse `~/.xurl` YAML, adding `serde_yaml` dep
  and coupling to an undocumented format that has already changed once
- **Token refresh race** — if bird reads tokens but xurl refreshes them, they can conflict
- **Still maintaining reqwest** — the auth code stays, just reads from a different file

Architecture A eliminates all of this. One transport mechanism. No token parsing.
No auth code. No reqwest.

### Why A over C (Stay Native)

Architecture C (fully native) means:

- Implementing and maintaining 15+ write operations
- Maintaining OAuth2 PKCE, OAuth1 signing, token refresh
- Tracking X API changes independently
- Implementing chunked media upload (~200 lines)
- Duplicating effort the X team is already doing

The only advantage is "single binary" — but bird already needs xurl for auth
(the X team's OAuth2 is the canonical implementation for PAYG).

---

## Key Questions

### 1. What was the hardest decision?

Accepting subprocess overhead on every cache miss. The entity store means the hot
path (cached data) has zero overhead, and the cold path (API call) adds ~50-100ms
to an operation that already takes ~200-500ms for network round-trip. For a CLI
tool, this is imperceptible.

### 2. What alternatives were rejected?

- **FFI binding to xurl's Go code**: Go runtime embedding in Rust is painful.
- **Rewrite bird in Go using xurl as library**: Throws away Rust work, rusqlite is mature.
- **Fork xurl and add caching**: Maintaining a Go fork defeats the purpose.
- **Architecture B (hybrid)**: Two transport mechanisms is worse than one.

### 3. Where are you least confident?

- Whether the X team will maintain xurl long-term. Mitigated by the clean boundary —
  swapping back to reqwest is a localized change.
- Whether xurl will add caching/cost features that overlap with bird's value-add.
  If so, bird may become a thin UX wrapper or redundant entirely.

---

## Resolved Questions

- [x] **Output parsing**: JSON is clean when piped (fatih/color auto-disables ANSI on non-TTY)
- [x] **Token format**: Irrelevant with full wrap — bird never reads `~/.xurl`
- [x] **No-color mode**: Not needed — color auto-disabled on pipe
- [x] **Write ops**: User wants them, passthrough is acceptable
- [x] **Architecture**: A — full wrap, xurl as sole transport
- [x] **GitHub repo vs npm**: Same repo, local clone was stale (v0.5 vs v1.0.3)
- [x] **Entity store compatibility**: Works — checks store first, only calls xurl on miss
- [x] **Pagination**: Bird constructs URL with next_token, passes to xurl raw mode

## Next Steps (Plan Phase)

- [ ] Create `src/transport.rs` — thin xurl subprocess wrapper
  - `fn xurl_request(args: &[&str]) -> Result<serde_json::Value, BirdError>`
  - Captures stdout (JSON), stderr (errors), exit code
  - Maps xurl errors to `BirdError` variants
- [ ] Migrate read commands to use transport module instead of reqwest
  - `me`, `profile`, `search`, `bookmarks`, `thread`, `watchlist check`
  - Entity store integration unchanged (parse JSON, decompose, store)
- [ ] `bird login` -> `xurl auth oauth2` (subprocess, inherit stdio for browser flow)
- [ ] Add write command passthroughs: `post`, `reply`, `like`, `follow`, `dm`
- [ ] Log all xurl calls to usage table for cost tracking
- [ ] Remove `auth.rs`, `login.rs`, reqwest dependency, OAuth crates
- [ ] Update `bird doctor` to check xurl binary availability and version
- [ ] Deprecation notice for `~/.config/bird/tokens.json` (point to `xurl auth`)
- [ ] Update `bird raw get/post/put/delete` to delegate to xurl
