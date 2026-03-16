# Entity Store: Cache Redesign

**Date:** 2026-02-17  
**Status:** Draft  
**Scope:** Replace request-level TTL cache with entity-level permanent data store

---

## What We're Building

A fundamental shift from caching HTTP responses to storing API entities (tweets, users) as permanent local data. The X API Pay-Per-Use model charges per-resource on first fetch per UTC day, with subsequent fetches of the same resource free within that day. The current cache optimizes for the wrong thing: it uses short TTLs (15 min) that cause re-fetches within the free window and discards data we've already paid for.

**New model:**

- **Within the same UTC day** as an entity's `last_refreshed_at`: always hit the API (it's free, and we get fresh data). Update local copy.
- **Past the UTC day boundary**: serve from local DB to avoid re-charging. The data persists permanently -- we paid for it.
- **Multi-ID batch requests**: split IDs. Only IDs that are locally stored AND last refreshed in a prior UTC day are served from the DB and excluded from the API request. All other IDs (missing, or refreshed today) go to the API.

## Why This Approach

### Current problems

1. **Request-level caching is wasteful.** Caching `?ids=123,456` as a blob means `?ids=123` is a separate cache miss. No entity reuse.
2. **Auth type in cache key is wrong.** Tweet `123` is the same data regardless of OAuth1/OAuth2/Bearer. Separate cache entries per auth type waste storage and miss dedup opportunities.
3. **Short TTLs discard paid data.** After 15 minutes, cached tweets are evicted. We paid $0.005 for that tweet -- why throw it away?
4. **No request splitting.** Requesting 100 tweet IDs when 80 are locally stored means re-fetching all 100 instead of just the 20 we need.
5. **X API deduplication makes within-day caching unnecessary for cost.** Within a UTC day, re-fetching the same tweet is free. Our 15-minute TTL is solving a non-problem for billing.

### X API billing model (confirmed via research)

- Per-resource billing: $0.005/post, $0.010/user
- Per-resource pricing, NOT per-field: requesting more `tweet.fields` or `expansions` does not increase the per-resource cost
- Daily UTC deduplication: first fetch of a tweet ID per UTC day is charged; subsequent fetches of the same ID within that day are free (cross-endpoint)
- "Soft guarantee" -- works in most cases, not SLA-backed
- Rate limits still apply regardless of billing deduplication
- Failed requests (4xx, 5xx) are not charged

### What local storage still provides

- **Cross-day cost avoidance**: Serving a tweet from local DB on day 2 saves $0.005
- **Offline/degraded access**: `--cache-only` for entity lookups when API is unavailable
- **Data ownership**: We paid for a copy of the data -- the DB is a permanent local store, not a disposable cache
- **Latency reduction**: Local reads are instant vs network round-trips
- **Rate limit conservation**: Deferred to a separate brainstorm, but reducing unnecessary API calls helps

## Key Decisions

### 1. Entity-level storage, not request-level caching

Store individual entities (tweets, users) in normalized tables, keyed by entity ID. Each entity tracks `last_refreshed_at`. This enables:

- Cross-command reuse (a tweet from search is available for direct lookup)
- Multi-ID request splitting (only fetch IDs not in local store or stale)
- Auth-agnostic storage (same tweet regardless of how it was fetched)

### 2. UTC-day freshness boundary

The freshness check is: "Was this entity last refreshed during the current UTC day?"

- **Yes (or missing)** → Fetch from API. Update local copy. (Within-day re-fetch is free.)
- **No (prior UTC day)** → Serve from local DB. Skip API call. (Would cost money.)

This aligns with X's billing deduplication window. We're not caching to avoid free requests -- we're storing to avoid paid ones.

### 3. Auth-agnostic entity storage

Drop `auth_type` from entity storage keys entirely. A tweet is a tweet. User-context data (bookmarks, likes) is stored as relationships (user_id, tweet_id) separate from the tweet entity itself.

### 4. Auto-generated typed Rust structs from OpenAPI spec

X publishes an official OpenAPI 3.0.0 spec at `https://api.x.com/2/openapi.json`. Use **typify** (by Oxide Computer) via `build.rs` to auto-generate all entity types from `components/schemas`. This is the Rust equivalent of Zod/Swagger codegen -- one source of truth (the vendored spec file), auto-generated types, shared across the entire codebase.

**Approach:**

1. Vendor the spec: `spec/openapi.json` (committed to repo)
2. `build.rs` extracts `components/schemas`, feeds to typify
3. Generates Rust structs with `Serialize`/`Deserialize` + custom derives (`Clone`, `Debug`, etc.)
4. Output to `$OUT_DIR/x_api_types.rs`, included via `include!()` macro
5. `cargo:rerun-if-changed=spec/openapi.json` for incremental builds
6. Generate ALL types from the spec (unused types have minimal overhead, zero maintenance for selection)

**Benefits (DRY + SRP):**

- Single source of truth: OpenAPI spec defines all types
- No hand-written entity structs to maintain or drift
- Compile-time field validation
- Consistent serialization shape across DB and API sources
- `Option<T>` for optional fields handled automatically by typify
- Custom derives addable globally or per-type (e.g., `sqlx::FromRow` for DB-backed types)

### 5. Single canonical field set per entity type

Define ONE comprehensive `tweet.fields`, `user.fields`, and `expansions` set used by ALL commands when talking to the API. Since pricing is per-resource (not per-field), there is no cost penalty for requesting more fields. Commands select which fields to *display* from the stored entity.

This replaces the current per-command field constants (search.rs, thread.rs, watchlist.rs each define their own).

### 6. Decompose API expansions into separate entities

When the API returns expanded objects (e.g., `includes.users` alongside tweets), decompose them into separate entity upserts. A tweet lookup with `expansions=author_id` results in:

- Tweet entity upserted (with `author_id` as foreign key)
- User entity upserted (from `includes.users`)

Maximum normalization and reuse. Same author across 50 tweets = 1 user row, not 50 copies.

### 7. Multi-ID request splitting with merge

For batch requests (e.g., `?ids=1,2,3,4,5`):

1. Check each ID's `last_refreshed_at` in the entity store
2. IDs with `last_refreshed_at` in a prior UTC day AND present in DB → serve from DB (avoid charge)
3. All other IDs (missing, or refreshed today) → include in API request
4. Merge DB entities + API entities, preserving original request ID ordering
5. Upsert all API-returned entities into the store

Field shape consistency: canonical field set ensures DB and API entities have the same shape. If differences exist (e.g., null vs populated), serve what we have. Typed structs with `Option<T>` fields handle this naturally.

### 8. Hybrid raw + normalized storage

- **Normalized entity tables**: `tweets`, `users` (and future entity types) with typed, decomposed data
- **Raw response table**: Request-keyed (normalized URL, minus auth_type). Maps API request → exact JSON response for `bird raw` compatibility
- Entities are decomposed from the same response that gets stored raw
- Entity-linking of raw responses deferred to future if needed

### 9. Relationship separation

User-context data (bookmarks, likes, etc.) is stored as junction tables linking account username to entity IDs. The underlying entity data is shared across all accounts. The `--account` flag determines which relationship data to use.

### 10. Search always hits API

Search queries are inherently time-sensitive and not entity-cacheable. Always fetch from API. But store all returned entities (tweets, users) in normalized tables for future direct lookups.

### 11. Permanent storage with configurable limit

Default to a generous size limit (e.g., 1 GB). Prune least-recently-accessed entities when exceeded. User-configurable via config or CLI flag. No auto-expiry -- data persists until space pressure forces pruning.

### 12. CLI flags

- `--no-cache`: Disable local store entirely (no reads, no writes). Direct API passthrough.
- `--refresh`: Skip local store reads, still write responses. Forces fresh data.
- `--cache-only`: Serve from local store only. No API calls. Works for entity lookups; errors for search.
- `--max-db-size`: Configure storage limit.

### 13. Store pagination cursor state

For paginated endpoints (bookmarks, timeline, search), store the pagination cursor state per (endpoint, account). This enables resuming from where a previous fetch stopped -- useful for large bookmark imports or timeline backfills.

### 14. New DB file, clean break

Create a new `bird.db` (or similar) for entity storage. Drop/ignore the old `cache.db`. The old cache held request-level blobs that cannot be meaningfully migrated to entity-level storage. Clean break avoids migration complexity.

### 15. Big bang replacement, search-first validation

Replace the entire `CachedClient` with the new entity store in one pass. Build and validate against `search` (the most complex command) first -- if it works there, all other commands are simpler. No gradual migration; remove old cache module entirely.

### 16. API types ≠ DB types: translation layer required

Auto-generated types from the OpenAPI spec match the API response shape (nested, denormalized). The DB schema needs flat rows with foreign keys (e.g., tweet row with `author_id` column, not a nested user object). A translation layer is needed between API types (deserialization targets) and DB types (storage models). This is a design detail for the planning phase, not a brainstorm decision -- but it must not be overlooked.

### 17. Cost estimation must reflect request splitting

With request splitting, some entities come from the DB (no cost) and some from the API (billable). `cost.rs` currently counts all objects in a response. It must be updated to count only API-fetched entities. DB-served entities are free.

### 18. Usage tracking survives the migration

The current cache module handles usage logging (`usage`, `usage_actual` tables). The new entity store replaces the cache, but usage tracking must be preserved. The `bird usage` command depends on it.

### 19. CLI commands adapt to new schema

`bird cache stats` and `bird cache clear` are user-facing commands. With the new entity store, these become `bird db stats` / `bird db clear` (or similar). The planning phase should address the CLI surface rename.

### 20. Rate limiting is a separate concern

Rate limit management (429 retries with backoff based on `x-rate-limit-reset` headers, request debouncing, per-endpoint tracking) is explicitly out of scope for this brainstorm. It should be addressed in a dedicated brainstorm because it has its own design decisions and interacts with but is independent of the storage layer.

## Resolved Questions

1. **Should we cache within the 24h window?** No. Within the UTC day, always hit the API for fresh data. It's free.
2. **Should auth type affect cache keys?** No. Entity data is the same regardless of auth method. User-context relationships are separated.
3. **What about search?** Always hit API. Store returned entities for future lookups.
4. **Offline mode scope?** Entity lookups only (`--cache-only`). Search errors in offline mode.
5. **Storage limits?** Configurable with generous default (1 GB). No auto-expiry.
6. **What about `bird raw`?** Store raw responses in a request-keyed table (normalized URL, minus auth_type).
7. **Staleness UX?** Entity records include `last_refreshed_at`. No visual indicators for now. `--refresh` for manual override.
8. **Schema migration path?** New DB file (`bird.db`), drop old `cache.db`. Clean break.
9. **Partial entity data?** Always request canonical (maximum useful) field set. Typed Rust structs with `Option<T>` fields. Upsert/merge on each fetch.
10. **Pagination state?** Store cursor state per (endpoint, account) to enable resume-from-where-we-left-off.
11. **Rate limits vs fresh data?** Always prefer fresh data within UTC day. Rate limit management is a separate brainstorm.
12. **How to define entity types?** Auto-generate ALL types from vendored OpenAPI spec (`spec/openapi.json`) using typify via build.rs. Single source of truth, DRY/SRP. Replaces `serde_json::Value` dynamic parsing. Generate all types (unused types add minimal overhead).
13. **One field set or per-command?** Single canonical field set for all commands. Commands choose what to display.
14. **Decompose expansions?** Yes. Expanded objects (users, media, etc.) are upserted into their own entity tables.
15. **Request splitting merge behavior?** Canonical field set ensures consistent shape. `Option<T>` fields handle gaps. Preserve original ID ordering in merged output.
16. **Transition strategy?** Big bang replacement. Build against search (most complex command) first.
17. **Raw response keying?** Request-keyed (normalized URL). Entity-linking deferred to future.

## Future Considerations (Out of Scope)

- Rate limiting / 429 retry / request debouncing (separate brainstorm)
- Full-text search over local entities (SQLite FTS5)
- UI/TUI for browsing local data
- Data export (local DB → JSON/CSV)
- Multi-account entity sharing vs isolation
- Entity-linking of raw responses (junction table mapping responses → entity IDs)
