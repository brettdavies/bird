# Brainstorm: Research Commands & Transparent Caching Layer

**Date:** 2026-02-11
**Status:** Draft

---

## What We're Building

A full research command suite for bird with an aggressive caching layer. This adds 5 new commands (`search`, `thread`, `profile`, `watchlist`, `usage`) and a transparent SQLite cache that sits between ALL HTTP calls and the X API, reducing costs on a per-endpoint basis.

### New Commands

| Command | Purpose | X API Endpoint |
|---------|---------|---------------|
| `bird search <query>` | Search recent tweets with engagement filtering, sorting, noise reduction | `GET /2/tweets/search/recent` |
| `bird thread <tweet_id>` | Follow a conversation thread from a root tweet | `GET /2/tweets/{id}` + `GET /2/tweets/search/recent` (conversation_id) |
| `bird profile <username>` | User profile with public metrics and description | `GET /2/users/by/username/{username}` |
| `bird watchlist [check\|add\|remove\|list]` | Monitor a list of accounts for recent activity | Batch of search calls per account |
| `bird usage [--since DATE]` | View accumulated API costs over time | No API call (reads from SQLite) |

### Transparent Cache Layer

A SQLite-backed cache (`~/.config/bird/cache.db`) that intercepts ALL outgoing HTTP requests (not just new commands). Even existing `bird get`, `bird bookmarks`, and `bird me` benefit automatically.

**Per-endpoint TTL defaults:**

| Endpoint pattern | Default TTL | Rationale |
|-----------------|-------------|-----------|
| `/2/tweets/search/*` | 15 minutes | Search results change frequently; re-fetch is free within 24hr dedup window |
| `/2/users/*` | 1 hour | Profile data changes infrequently |
| `/2/tweets/{id}` | 15 minutes | Tweet text is immutable but metrics change; re-fetch is free within 24hr dedup window so short TTL costs nothing |
| `/2/users/{id}/bookmarks` | 15 minutes | Bookmarks change with user activity |
| Default (all other) | 15 minutes | Safe fallback |

**Cache controls:**
- `--refresh` — bypass cache for this request
- `--cache-ttl <seconds>` — override TTL for this request
- `--no-cache` — disable cache entirely for this request
- Cache key: hash of (method, URL, auth-type) — not the token itself
- **Exclusions:** Auth endpoints (`/2/oauth2/token`) and all non-GET requests are never cached

---

## Why This Approach

### The X API is expensive

**Billing is per-object returned, not per-request** ([source](https://jesusiniesta.es/blog/x-api-pricing-tiers-what-you-actually-get)):
- $0.005 per post read (returned in any endpoint response)
- $0.010 per user lookup
- $0.010 per post creation
- 24hr UTC dedup: same post pulled twice in a window = charged once
- Only successful responses are billed; failed requests are free

At $0.005/post, a search returning 100 tweets costs $0.50. A 5-query research session across 3 pages each (~1,500 tweets) costs ~$7.50. Caching and leveraging X's 24-hour dedup window can cut this significantly. The transparent cache means even basic `bird get` usage benefits.

### Bird already has the hard parts

Auth (OAuth2 PKCE, OAuth1, Bearer), HTTP client with timeouts, streaming output, structured errors — all done. The gap is purely in post-processing and caching. Adding these features builds on the existing infrastructure rather than duplicating it.

---

## Key Decisions

### 1. SQLite transparent cache (not file-based)

**Decision:** SQLite at `~/.config/bird/cache.db` as a transparent layer on all HTTP calls.

**Why not file-based:**
- Atomic reads/writes (no race conditions with concurrent bird invocations)
- TTL-based pruning is a single SQL DELETE
- Size limits are enforceable
- Usage tracking (cost accumulation) can live in the same database
- Single file vs. hundreds of cache files

**Schema (conceptual):**
- `cache` table: key (hash), url, method, status_code, headers, body, created_at, ttl_seconds
- `usage` table: timestamp, endpoint, method, object_type (tweet/user), object_count, estimated_cost_usd, cache_hit (bool)
- `usage_actual` table: date (UTC), tweet_count (from X API `GET /2/usage/tweets`), synced_at

### 2. Configurable field profiles (not hardcoded)

**Decision:** Define field "profiles" that bundle tweet.fields, user.fields, and expansions.

**Profiles:**
- `minimal` — Just text and author_id. Cheapest, for quick checks.
- `standard` (default) — created_at, public_metrics, author_id, conversation_id, entities + author expansion + username, name, public_metrics. Optimal balance of data and cost.
- `full` — Everything above plus referenced_tweets, context_annotations, geo, attachments, edit_history. Most expensive.

**Override:** `--fields minimal`, `--fields full`, or `--tweet-fields`, `--user-fields`, `--expansions` for manual control.

### 3. Cost tracking: estimated + actual

**Decision:** Three-pronged approach:
- **stderr on every call:** Print estimated cost and cache hit/miss to stderr (e.g., `[cost] 10 tweets read, ~$0.05 (cache miss)` or `[cost] cache hit, $0.00`)
- **SQLite accumulation:** Every API call (cached or not) is logged to the `usage` table with raw object counts (tweets, users), object type, and estimated dollar cost. Storing counts (not just dollars) allows recalculation if pricing changes.
- **Actual usage from X API:** `GET /2/usage/tweets` returns daily project-level tweet consumption for up to 90 days ([source](https://devcommunity.x.com/t/announcing-the-new-usage-endpoint-in-the-x-api-v2/208160)). `bird usage --sync` fetches actual usage from X and stores it alongside estimates. Over time, comparing estimated vs. actual reveals how accurate our tracking is and whether we're missing charges (e.g., from expansions or other billing nuances).

### 4. Watchlist in config.toml

**Decision:** Add a `[[watchlist]]` array to `~/.config/bird/config.toml`:

```toml
[[watchlist]]
username = "anthropic"
note = "Anthropic official"

[[watchlist]]
username = "OpenAI"
note = "OpenAI official"
```

`bird watchlist add <username> [--note "..."]` and `bird watchlist remove <username>` modify this file programmatically.

### 5. Per-endpoint TTL defaults

**Decision:** Cache TTLs vary by endpoint pattern (see table above). Optimized for X API's billing model.

**Critical insight — X's 24hr dedup is billing-only, not data-staleness:** When you re-request a tweet within the 24hr UTC window, X returns **fresh data** (updated metrics) but **doesn't charge you**. This means re-requesting tweets within the dedup window is effectively free — you get updated like/retweet/reply counts at no cost.

**Implication for our cache:** A 24hr cache TTL on tweets would actually prevent us from getting free metric updates. Instead:
- Cache tweet **text/content** for 24hr (immutable, never changes)
- Cache tweet **metrics** for 15min (changes frequently, but re-fetching within 24hr window is free)
- This gives us fresh engagement data at zero additional cost

**Note:** This behavior (fresh data on deduped requests) is inferred from standard API billing patterns. Verify empirically by requesting a tweet, waiting for engagement changes, re-requesting, and comparing the `public_metrics` values.

### 5a. Cache-aware request packing

**Decision:** Since billing is per-object (not per-request), the cache layer should optimize what gets requested:

**How it works:** Before sending a multi-object GET (e.g., `GET /2/tweets?ids=1,2,3,4,5`), check the cache for each ID. If IDs 1 and 3 are cached and valid:
1. Remove them from the outgoing request (`GET /2/tweets?ids=2,4,5`)
2. Backfill the response by merging cached objects with fresh API results
3. Return the combined set to the caller as if all 5 were fetched

**Why this matters:** If 3 of 5 tweet IDs are already cached, we save $0.015 (3 * $0.005) per call. Over a research session with repeated thread-following and cross-referencing, this adds up.

**Applies to:** Any endpoint that accepts comma-separated IDs (`/2/tweets?ids=`, `/2/users?ids=`, `/2/users/by?usernames=`). Does NOT apply to search (which returns unpredictable results).

**Bonus optimization:** When the cache removes enough IDs that the request would be empty (all cached), skip the HTTP call entirely and return from cache only.

### 6. Search output formatting

**Decision:** JSON-first, consistent with all other bird commands:
- Default: Compact JSON to stdout. Agents parse this directly.
- `--pretty`: Pretty-printed JSON with syntax highlighting (colored keys, strings, numbers when stderr is a TTY). Uses `owo-colors` (already a dependency).
- `--format telegram`: Compact one-line-per-tweet with engagement indicators.
- `--format markdown`: Research document format with full text, metrics, links.

### 7. Noise filtering and sorting

**Decision:** Built into `bird search`:
- Auto `-is:retweet` (unless query already contains `is:retweet`)
- `--quality` flag adds `min_likes >= 10`
- `--sort likes|impressions|retweets|replies|recent` (default: likes)
- `--min-likes N`, `--min-impressions N` for manual thresholds
- Sorting and filtering happen post-fetch (X API doesn't support these as search operators)

---

## Open Questions

1. **Cache size limits?** Should we set a max cache DB size (e.g., 100MB) and auto-prune oldest entries when exceeded?

2. **Should `bird search` support full-archive search?** The X API has `GET /2/tweets/search/all` for full archive (not just last 7 days). This is on the same pay-per-use plan. Could be a `--archive` flag.

3. **Rate limit handling:** Should bird use a fixed delay (e.g., 350ms) between paginated requests, or inspect rate limit headers and adapt dynamically?

4. **Scopes:** The current OAuth2 login requests `tweet.read users.read bookmark.read offline.access`. New research commands only need `tweet.read` and `users.read` which are already covered. No scope changes needed.

5. **Dependency budget:** SQLite adds `rusqlite` (with bundled SQLite). This is ~3MB to binary size. Acceptable?

---

## What We're NOT Building

- **LLM integration / synthesis** — Bird provides the data; the agent (Claude) does the synthesis.
- **Streaming/real-time monitoring** — No WebSocket or streaming API. Watchlist is poll-based.
- **Write operations for research** — No tweeting, liking, or bookmarking from research commands.
- **Multi-provider support** — No integration with other social APIs. X only.

---

## Implementation Order (Suggested)

1. **SQLite cache layer** — Foundation. Transparent to all commands.
2. **Cost tracking** — Wired into cache layer. stderr display + usage table.
3. **Field profiles** — Shared config for all research commands.
4. **`bird search`** — Biggest value. Uses cache, fields, cost tracking.
5. **`bird profile`** — Simple, one API call.
6. **`bird thread`** — Depends on search infrastructure (conversation_id queries).
7. **`bird watchlist`** — Config file management + batch search.
8. **`bird usage`** — Read-only, queries SQLite.
