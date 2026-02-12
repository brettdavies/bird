---
title: "feat: Search Command with Filtering and Sorting"
type: feat
date: 2026-02-11
series: "Research Commands & Caching Layer"
plan: 2 of 4
depends_on: "2026-02-11-feat-transparent-cache-layer-plan"
deepened: 2026-02-11
---

# feat: Search Command with Filtering and Sorting

## Enhancement Summary

**Deepened on:** 2026-02-11
**Agents used:** best-practices-researcher (X API), framework-docs-researcher (Rust CLI output), architecture-strategist, performance-oracle, security-sentinel, code-simplicity-reviewer, pattern-recognition-specialist, learnings-researcher

### Scope Changes (Simplicity Review)

The original plan front-loaded v3 complexity for a v1 command. **5 flags removed, ~200 lines saved:**

| Feature | Original | After Review | Rationale |
|---------|----------|-------------|-----------|
| `--quality` flag | Included | **Cut** | Redundant with `--min-likes 10` — saves one flag, one interaction rule, one test |
| `--archive` flag | Included | **Deferred** | Requires Pro tier ($5K/mo); achievable via `bird get /2/tweets/search/all` today |
| `--format telegram/markdown` | 2 formatters | **Deferred** | No consumer exists; 140 lines of formatting code for speculative formats |
| `--fields minimal/standard/full` | Included | **Cut** | Depends on `FieldProfile` which does not exist (Plan 1 deferred it); hardcode sensible defaults |
| `--min-impressions` | Included | **Cut** | No demonstrated need; impressions are unreliable signal; trivial to add later |
| `--sort impressions/retweets/replies` | 5 sort keys | **2 keys** | `likes` and `recent` cover 95% of use cases; one-line addition per key later |
| Default sort | `likes` | **`recent`** | Matches API order; less surprising; explicit `--sort likes` when wanted |
| Enriched `meta` object | Custom JSON | **Passthrough** | Breaks API response contract; use stderr for metadata like other commands |

**Result:** 6 CLI flags (down from 11). ~100 lines in `src/search.rs` (down from ~296). Well under the 200-line refactor trigger.

### Critical Fixes

1. **BUG: `FieldProfile` does not exist** — Plan 1 explicitly deferred this to Plan 2 (Enhancement Summary line 26). The plan references `crate::fields::FieldProfile` but no such module exists. Hardcode field constants inline.
2. **BUG: `RateLimiter` does not exist** — Plan 1 cut this to a "2-line sleep." `CachedClient` has no rate limiting logic. Add `tokio::time::sleep(150ms)` between paginated requests inline.
3. **BUG: Import path wrong** — `crate::cached_client::CachedClient` should be `crate::cache::CachedClient`.
4. **BUG: Handler takes `&CachedClient`** — Must be `&mut CachedClient` (`.get()` requires `&mut self`).
5. **BUG: Missing `use_color` parameter** — Every existing handler receives `use_color: bool`; search omits it.
6. **BUG: `resolve_token_for_command(client, ...)` wrong** — Must pass `client.http()`, not `client` (function takes `&reqwest::Client`).
7. **BUG: CachedClient does NOT auto-display cost** — Callers must manually call `cost::estimate_cost()` + `cost::display_cost()`.
8. **BUG: `should_skip_cache()` misses `next_token=`** — Only checks `pagination_token=`. Search pagination uses `next_token`. Update `cache.rs`.
9. **BUG: UTF-8 truncation panic** — `&text[..140]` panics on multi-byte characters. Use `char_indices()`.
10. **BUG: Sort pseudo-code no-op contradicts plan text** — `_ => {}` should be `_ => return Err(...)` (fail fast).

### Key Research Findings

- **`-is:retweet` can leak retweets** — Known intermittent X API bug. Add client-side dedup via `referenced_tweets` field.
- **`sort_order=relevancy` breaks pagination** — Known X API bug: `next_token` not returned. Do not use `sort_order=relevancy` for multi-page searches.
- **Phantom `next_token` on last page** — X API sometimes returns `next_token` with zero data. Break on empty `data` array, not just missing token.
- **Duplicate tweets at page boundaries** — Deduplicate by tweet ID across pages.
- **OAuth1 also works for `search/recent`** — OpenAPI spec confirms all three auth types (OAuth2User, OAuth1, Bearer).
- **Pay-per-use pricing (Feb 2026)** — 24hr UTC dedup window means repeated searches for the same tweets within a day are free.
- **X API search accepts both `next_token` and `pagination_token`** — Use `next_token` consistently (matches response field name).

---

## Overview

Add `bird search <query>` -- a focused tweet search command that wraps the X API `GET /2/tweets/search/recent`. The command provides engagement-based filtering, post-fetch sorting, noise reduction, multi-page pagination, and JSON output. It uses Plan 1's `CachedClient` for transparent caching and manual cost display.

**Plan series:**

| # | Plan | Status |
|---|------|--------|
| 1 | Transparent Cache Layer | Predecessor (must be implemented first) |
| **2** | **Search Command** (this plan) | Active |
| 3 | Profile & Thread Commands | Blocked by Plan 1 |
| 4 | Watchlist & Usage Commands | Blocked by Plans 1-3 |

## Problem Statement

Research workflows on X/Twitter require searching for tweets by topic, filtering by engagement quality, and reviewing results in formats suited to different consumers (agents, humans, messaging apps). The raw `bird get /2/tweets/search/recent` works but requires manual query parameter construction, no post-fetch filtering, no sorting, and only raw JSON output.

**Concrete pain points:**

1. **Manual query building:** Users must remember `tweet.fields`, `user.fields`, `expansions`, `max_results`, and X search operator syntax. A single search requires a command like:
   ```
   bird get /2/tweets/search/recent --query "query=rust lang -is:retweet&tweet.fields=created_at,public_metrics,author_id,conversation_id&user.fields=username,name&expansions=author_id&max_results=100"
   ```

2. **No engagement filtering:** X API returns all matching tweets with no way to filter by like count or impression threshold. Noisy results dominate research sessions.

3. **No sorting:** X API returns tweets in reverse chronological order only. Sorting by engagement (likes, impressions, retweets, replies) requires post-fetch processing.

4. **No noise reduction:** Retweets pollute results. Quality filtering (minimum engagement thresholds) is manual.

5. **No research-friendly output:** Raw JSON is fine for agents but unusable for humans scanning results or pasting into chat. Markdown and Telegram formats enable different consumption patterns.

6. **Cost blindness:** Without Plan 1's cost tracking, users have no visibility into how much a multi-page search session costs.

## Proposed Solution

A new `bird search` command with:

- **Simple invocation:** `bird search "rust programming"` -- just a query string, sensible defaults handle the rest
- **Automatic noise reduction:** `-is:retweet` appended unless the user explicitly includes `is:retweet` in their query; client-side dedup via `referenced_tweets` as defense-in-depth
- **Engagement sorting:** `--sort likes|recent` (default: `recent`, preserving API order)
- **Engagement filtering:** `--min-likes N` for post-fetch quality filtering
- **Multi-page pagination:** `--pages N` (clamped 1..=10) for fetching multiple pages with per-page cost display
- **JSON output:** Compact JSON (default) or `--pretty` -- matches every other command in the CLI
- **Transparent caching:** Via Plan 1's `CachedClient` -- repeated queries within TTL (15min) are free
- **Tweet deduplication:** By tweet ID across pages to handle X API boundary duplicates

### Research Insights: Extension Points for Future Versions

Every deferred feature is a one-session addition with zero refactoring:

| Feature | How to add | Effort |
|---------|-----------|--------|
| `--archive` | Swap one URL string + adjust `max_results` clamp | 5 lines |
| `--min-impressions` | Copy the `--min-likes` filter pattern | 3 lines |
| `--sort retweets\|replies\|impressions` | Add match arms to `sort_tweets` | 1 line each |
| `--format telegram\|markdown` | New formatting functions + `--format` flag | ~80 lines in `format.rs` |
| `--fields minimal\|standard\|full` | Build `FieldProfile` enum when Plan 3 needs it | ~40 lines in `fields.rs` |
| `--quality` | One-line alias for `--min-likes 10` | 1 line |

## Technical Approach

### New File: `src/search.rs`

**Responsibility:** Search query building, API interaction, post-fetch filtering/sorting, JSON output.

**Estimated size:** ~100 lines (well under the 200-line refactor trigger).

### Handler Signature

```rust
// src/search.rs

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{CacheContext, CachedClient};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::requirements::AuthType;
use reqwest::header::HeaderMap;
use std::collections::HashSet;

/// Search options bundled to avoid clippy::too_many_arguments.
pub struct SearchOpts<'a> {
    pub query: &'a str,
    pub pretty: bool,
    pub sort: &'a str,        // "recent" or "likes"
    pub min_likes: Option<u64>,
    pub max_results: u32,      // clamped 10..=100
    pub pages: u32,            // clamped 1..=10
}

pub async fn run_search(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    opts: SearchOpts<'_>,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { ... }
```

### Research Insights: Handler Design

- **`SearchOpts` struct** replaces 12 positional parameters. The existing `run_raw` has 9 params with `#[allow(clippy::too_many_arguments)]`; this avoids compounding that debt.
- **`&mut CachedClient`** is required because `CachedClient.get()` takes `&mut self` for cache writes.
- **`use_color: bool`** follows the pattern in `raw.rs:24` and `bookmarks.rs:14`.
- **`resolve_token_for_command(client.http(), config, "search")`** -- pass `.http()` to get the inner `&reqwest::Client`, per `raw.rs:39` and `bookmarks.rs:17`.
- **Cost display is manual** -- call `cost::estimate_cost()` + `cost::display_cost()` explicitly per page, as `raw.rs:52-57` does. `CachedClient` does NOT auto-display cost.

**Pattern:** Follows `src/raw.rs` for auth resolution and request building, and `src/bookmarks.rs` for pagination loop structure.

### Command Enum Addition

**File:** `src/main.rs` (after `Bookmarks` variant, inside `enum Command`)

```rust
/// Search recent tweets
Search {
    /// Search query (X API search syntax)
    query: String,

    /// Pretty-print JSON output
    #[arg(long)]
    pretty: bool,

    /// Sort results: recent (default), likes
    #[arg(long, default_value = "recent")]
    sort: String,

    /// Minimum like count threshold
    #[arg(long)]
    min_likes: Option<u64>,

    /// Maximum results per page (default: 100, X API max)
    #[arg(long)]
    max_results: Option<u32>,

    /// Number of pages to fetch (default: 1, max: 10)
    #[arg(long)]
    pages: Option<u32>,
},
```

**Dispatch addition in `run()`** (`src/main.rs`, inside `match command`):

```rust
Command::Search { query, pretty, sort, min_likes, max_results, pages } => {
    let opts = search::SearchOpts {
        query: &query,
        pretty,
        sort: &sort,
        min_likes,
        max_results: max_results.unwrap_or(100).clamp(10, 100),
        pages: pages.unwrap_or(1).clamp(1, 10),
    };
    search::run_search(client, &config, opts, use_color)
        .await
        .map_err(|e| BirdError::Command { name: "search", source: e })?;
}
```

### Auth Requirements

**File:** `src/requirements.rs` (inside `requirements_for_command`)

### Research Insights: Auth Types

The OpenAPI spec confirms `search/recent` accepts **all three** auth types: BearerToken, OAuth2UserToken, and OAuth1 UserToken. This matches the existing `RAW_ACCEPTED` pattern. Use `RAW_ACCEPTED` directly rather than defining a new constant.

```rust
// Inside requirements_for_command():
"search" => CommandReqs {
    accepted: RAW_ACCEPTED,  // OAuth2User, OAuth1, Bearer -- all work for search/recent
    oauth2_hint: OAUTH2_HINT,
    oauth1_hint: OAUTH1_HINT,
    bearer_hint: BEARER_HINT,
},
```

Also add `"search"` to the `command_names_with_auth()` array.

### Query Building

The query builder constructs the full URL with query parameters for the X API search endpoint.

**Endpoint:** `https://api.x.com/2/tweets/search/recent` (only endpoint for v1).

**Hardcoded field constants** (inline, no `FieldProfile` module):

```rust
// Sensible defaults for research workflows. Extract to shared module when Plan 3 needs it.
const TWEET_FIELDS: &str = "created_at,public_metrics,author_id,conversation_id,referenced_tweets";
const USER_FIELDS: &str = "username,name";
const EXPANSIONS: &str = "author_id";
```

### Research Insights: Field Selection

- **`referenced_tweets`** is included for client-side retweet dedup (`-is:retweet` can leak).
- **`conversation_id`** enables future thread reconstruction (Plan 3).
- **`public_metrics`** is essential for filtering/sorting by engagement.
- Expansion objects in `includes` are billable. `author_id` expansion adds ~100 user objects per page (~$1.00 extra per page). This is acceptable for research workflows but worth documenting.

**Query parameter construction pseudo-code:**

```rust
fn build_search_url(
    query: &str,
    max_results: u32,
    next_token: Option<&str>,
) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent").unwrap();

    url.query_pairs_mut().append_pair("query", query);
    url.query_pairs_mut().append_pair("tweet.fields", TWEET_FIELDS);
    url.query_pairs_mut().append_pair("user.fields", USER_FIELDS);
    url.query_pairs_mut().append_pair("expansions", EXPANSIONS);
    url.query_pairs_mut().append_pair("max_results", &max_results.to_string());

    if let Some(token) = next_token {
        url.query_pairs_mut().append_pair("next_token", token);
    }

    url.to_string()
}
```

### Research Insights: URL Construction Security

The `url::Url::query_pairs_mut().append_pair()` method applies `application/x-www-form-urlencoded` encoding to both key and value. A malicious query like `"test&tweet.fields=all"` becomes `query=test%26tweet.fields%3Dall`. No injection possible. This is the correct approach.

**Automatic noise reduction + client-side dedup:**

```rust
fn apply_noise_reduction(query: &str) -> String {
    if query.contains("is:retweet") {
        query.to_string()
    } else {
        format!("{} -is:retweet", query)
    }
}

/// Client-side retweet filter. `-is:retweet` can leak retweets (known X API bug).
/// Check `referenced_tweets` array for `type: "retweeted"` as defense-in-depth.
fn is_retweet(tweet: &serde_json::Value) -> bool {
    tweet.get("referenced_tweets")
        .and_then(|rt| rt.as_array())
        .map(|arr| arr.iter().any(|r| r.get("type").and_then(|t| t.as_str()) == Some("retweeted")))
        .unwrap_or(false)
}
```

This check is case-sensitive and matches any occurrence of `is:retweet` in the query, including `-is:retweet`. The client-side `is_retweet()` check is a defense-in-depth measure for when the server-side filter leaks.

### Pagination

Multi-page fetching follows the token extraction pattern from `src/bookmarks.rs` but collects results in memory (needed for filtering/sorting).

**Pagination pseudo-code:**

```rust
let mut all_tweets: Vec<serde_json::Value> = Vec::new();
let mut seen_ids: HashSet<String> = HashSet::new();
let mut all_users: Vec<serde_json::Value> = Vec::new();
let mut next_token: Option<String> = None;

let effective_query = apply_noise_reduction(opts.query);

for page_num in 1..=opts.pages {
    let url = build_search_url(&effective_query, opts.max_results, next_token.as_deref());

    // Auth resolution (same pattern as raw.rs:39)
    let token = resolve_token_for_command(client.http(), config, "search").await?;

    // Build headers and send via CachedClient
    let mut headers = HeaderMap::new();
    // ... set Authorization header based on token type ...
    let ctx = CacheContext {
        auth_type: &AuthType::OAuth2User, // or Bearer, based on token
        username: config.username.as_deref(),
    };
    let response = client.get(&url, &ctx, headers).await?;

    // Manual cost display (CachedClient does NOT auto-display)
    let page: serde_json::Value = serde_json::from_str(&response.body)
        .unwrap_or(serde_json::Value::Null);
    let estimate = cost::estimate_cost(&page, &url, response.cache_hit);
    cost::display_cost(&estimate, use_color);

    // Early termination: break if page has no data (phantom next_token)
    let data = match page.get("data").and_then(|d| d.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => break,
    };

    // Per-page filtering + dedup (reduces memory when filters are active)
    for tweet in data {
        let id = tweet.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if seen_ids.contains(id) { continue; }  // cross-page dedup
        if is_retweet(tweet) { continue; }       // client-side retweet dedup
        if let Some(min) = opts.min_likes {
            let likes = extract_metric(tweet, "like_count");
            if likes < min { continue; }
        }
        seen_ids.insert(id.to_string());
        all_tweets.push(tweet.clone());
    }

    // Extract included users for future formatted output
    if let Some(includes) = page.get("includes") {
        if let Some(users) = includes.get("users").and_then(|u| u.as_array()) {
            all_users.extend(users.iter().cloned());
        }
    }

    // Page progress on stderr
    eprintln!("[search] page {}/{}: {} tweets", page_num, opts.pages, all_tweets.len());

    // Extract next_token
    next_token = page.get("meta")
        .and_then(|m| m.get("next_token"))
        .and_then(|t| t.as_str())
        .map(String::from);

    if next_token.is_none() { break; }

    // Rate limiting: 150ms between pages (no RateLimiter module exists)
    if page_num < opts.pages {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
```

### Research Insights: Pagination Gotchas

1. **Phantom `next_token` on last page** — X API sometimes returns `next_token` even when there is no more data. The next request returns `result_count: 0`. The `data.is_empty()` check handles this.
2. **Duplicate tweets at page boundaries** — Tweets can appear on adjacent pages. The `seen_ids: HashSet<String>` deduplicates by tweet ID.
3. **`sort_order=relevancy` breaks pagination** — Known X API bug: no `next_token` returned. Bird does not use `sort_order` at all (client-side sorting only), so this is not a concern.
4. **Both `next_token` and `pagination_token` are accepted** by the search API. Use `next_token` consistently (matches response field name).

**Key pagination decisions:**

- `max_results` defaults to 100 (X API maximum for `search/recent`), clamped to 10..=100
- `pages` defaults to 1, clamped to 1..=10 (caps cost at ~$5.00 per invocation)
- Per-page cost is displayed manually via `cost::estimate_cost()` + `cost::display_cost()`
- Rate limiting: inline `tokio::time::sleep(150ms)` between pages (no RateLimiter module)
- Per-page filtering reduces memory by discarding low-engagement tweets early
- **Cache interaction:** First page (no `next_token`) IS cached at 15min TTL. Subsequent pages with `next_token=` need `should_skip_cache()` updated (see Critical Fix #8)

### Post-Fetch Filtering

Filtering happens **per-page during accumulation** (not after all pages, see Pagination section). This reduces peak memory when filters are active.

### Research Insights: Per-Page Filtering

If `--min-likes 10` filters out 80% of tweets (common for quality research), peak memory drops from ~3MB to ~600KB for 500 tweets. The per-page stderr output shows "42 tweets (18 passed filters)" giving the user immediate quality feedback.

**Filtering is done inline in the pagination loop** (see pseudo-code above). The `extract_metric` helper:

```rust
fn extract_metric(tweet: &serde_json::Value, metric_name: &str) -> u64 {
    tweet.get("public_metrics")
        .and_then(|m| m.get(metric_name))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}
```

**Missing `public_metrics`:** If the field is absent (shouldn't happen with our hardcoded fields, but defensive), `extract_metric` returns 0, which fails any `--min-likes` threshold. This is correct -- if metrics are unavailable, we can't assert quality.

### Post-Fetch Sorting

Sorting happens after all pages are accumulated and filtered. The X API returns tweets in reverse chronological order; `--sort likes` re-sorts by engagement.

**Sort options:**

| `--sort` value | JSON path | Direction | Default |
|---------------|-----------|-----------|---------|
| `recent` | `created_at` | Descending (newest first) | Yes |
| `likes` | `public_metrics.like_count` | Descending | |

**Sorting pseudo-code:**

```rust
fn sort_tweets(tweets: &mut Vec<serde_json::Value>, sort_by: &str) {
    match sort_by {
        "recent" => {} // Already in API order (reverse chronological), no-op
        "likes" => tweets.sort_by(|a, b| {
            let a_likes = extract_metric(a, "like_count");
            let b_likes = extract_metric(b, "like_count");
            b_likes.cmp(&a_likes)  // descending
        }),
        _ => return Err(format!(
            "invalid --sort value \"{}\"; expected: recent, likes", sort_by
        ).into()),
    }
}
```

**Invalid sort key handling:** Returns an error **before** any API calls (fail fast):

```
search failed: invalid --sort value "foo"; expected: recent, likes
```

### Research Insights: Sorting Performance

For n=500 tweets, `sort_by` with per-element `extract_metric` does ~4,500 comparisons with ~9,000 HashMap lookups. This completes in microseconds. A Schwartzian transform (pre-extract keys) would reduce extractions from O(n log n) to O(n), saving ~10 microseconds. Not worth the complexity at this scale.

### Output Formatting

Two output modes, matching every other command in the CLI:

| Flag | Mode | Description |
|------|------|-------------|
| (none) | JSON | Compact JSON to stdout |
| `--pretty` | Pretty JSON | Indented JSON to stdout |

### Research Insights: Output Design

- **Passthrough API response shape** — The JSON output wraps the filtered+sorted results but preserves the standard X API response structure (`data`, `includes`, `meta`). This means `jq` scripts and agent parsers that expect the X API shape work unchanged. Search metadata (pages fetched, filter stats) goes to stderr, not into the JSON body.
- **stdout for data, stderr for diagnostics** — Cost estimates, page progress, and filter statistics go to stderr. This follows POSIX conventions and matches the existing `cost.rs` and `bookmarks.rs` patterns.
- **No enriched `meta` object** — The original plan added `query`, `sort`, `pages_fetched`, `filtered_count` to the `meta` field. This breaks the "JSON output = API response" contract that every other command follows.

**Output pseudo-code:**

```rust
// Build output JSON
let output = serde_json::json!({
    "data": all_tweets,
    "includes": { "users": all_users },
});

if opts.pretty {
    println!("{}", serde_json::to_string_pretty(&output)?);
} else {
    println!("{}", serde_json::to_string(&output)?);
}

// Metadata on stderr (not in JSON body)
eprintln!("[search] {} results | sorted by {} | {} pages fetched",
    all_tweets.len(), opts.sort, pages_fetched);
```

### Interaction with Plan 1 Components

| Plan 1 Component | How Search Uses It | Notes |
|------------------|--------------------| ------|
| `CachedClient` | All GET calls go through `CachedClient.get()`. Cache key includes the full URL with query params. | Different queries = different cache entries |
| Cache TTL | Search endpoints get 15-minute TTL. | Matches `/2/tweets/search/*` pattern |
| Cache exclusion | First page (no `next_token`) IS cached. Pages 2+ need `should_skip_cache()` updated to also check `next_token=`. | **BUG: current code only checks `pagination_token=`** |
| Cost tracking | **Manual.** Callers must call `cost::estimate_cost()` + `cost::display_cost()` per page. | `CachedClient` does NOT auto-display cost |
| Field profiles | **Do not exist.** Hardcode field constants inline. | Plan 1 deferred `FieldProfile` to Plan 2; we defer it further until Plan 3 needs it |
| Rate limiting | **Does not exist in CachedClient.** Add inline `tokio::time::sleep(150ms)` between pages. | Plan 1 cut `RateLimiter` to a "2-line sleep" |
| `--refresh` / `--no-cache` | Work transparently via `CacheOpts` in `CachedClient` construction. | No handler code needed |

### Prerequisite: Update `should_skip_cache()` in `cache.rs`

```rust
// src/cache.rs — update existing function
fn should_skip_cache(url: &str) -> bool {
    url.contains("/oauth2/token")
        || url.contains("pagination_token=")
        || url.contains("next_token=")  // <-- ADD THIS for search pagination
}
```

Also add a test: `assert!(should_skip_cache("https://api.x.com/2/tweets/search/recent?query=test&next_token=abc"));`

### Error Handling

Errors follow the existing `BirdError::Command` pattern:

| Error Condition | Behavior |
|----------------|----------|
| Invalid `--sort` value | Return error before API call: `invalid --sort value "foo"; expected: recent, likes` |
| `--max-results` out of range | Clamped silently to 10..=100 in dispatch (done in `main.rs`) |
| `--pages` out of range | Clamped silently to 1..=10 in dispatch (done in `main.rs`) |
| Auth failure | Delegated to `resolve_token_for_command` (returns `AuthRequiredError`) |
| API error (403, 429, 5xx) | Surfaced via response status check |
| Empty results | Output empty data array (`{"data":[],"includes":{"users":[]}}`) -- not an error |
| Network error | Propagated from `CachedClient` / reqwest |

**Validation order:** Validate `--sort` before making any API calls. Fail fast on invalid input. `--max-results` and `--pages` are clamped in the dispatch arm, not validated as errors.

### Module Declaration

**File:** `src/main.rs` (line 10, after `mod bookmarks;`)

```rust
mod search;
```

## Acceptance Criteria

### Functional Requirements

- [ ] `bird search "query"` returns tweets matching the query as compact JSON to stdout
- [ ] Default query appends `-is:retweet` unless query already contains `is:retweet`
- [ ] Client-side retweet dedup via `referenced_tweets` field (defense-in-depth)
- [ ] `--min-likes N` filters results to tweets with >= N likes
- [ ] `--sort recent` (default) preserves API reverse-chronological order
- [ ] `--sort likes` sorts results by `like_count` descending
- [ ] `--pretty` outputs pretty-printed JSON
- [ ] `--max-results N` controls results per page (clamped 10..=100)
- [ ] `--pages N` fetches multiple pages with pagination (clamped 1..=10)
- [ ] Invalid `--sort` value produces clear error before any API call
- [ ] Auth works with OAuth2User, OAuth1, and Bearer tokens
- [ ] `bird doctor search` shows auth availability for the search command
- [ ] Search uses Plan 1's `CachedClient` for all GET calls
- [ ] Cost is displayed on stderr for each page fetched (manual `cost::estimate_cost` + `cost::display_cost`)
- [ ] 150ms sleep between paginated page fetches
- [ ] Tweet deduplication by ID across page boundaries
- [ ] Empty `data` array terminates pagination (phantom `next_token` handling)

### Non-Functional Requirements

- [ ] `src/search.rs` stays under 200 lines (est. ~100 lines)
- [ ] No new crate dependencies (uses Plan 1's infrastructure)
- [ ] Filtering and sorting handle missing `public_metrics` gracefully (default to 0)
- [ ] stdout for data, stderr for diagnostics (cost, page progress, filter stats)
- [ ] JSON output preserves X API response shape (`data`, `includes`)

### Quality Gates

- [ ] Unit tests for: `apply_noise_reduction`, `build_search_url`, `is_retweet`, `extract_metric`, `sort_tweets`
- [ ] Unit test: invalid `--sort` value returns error before API call
- [ ] Unit test: tweet deduplication across pages by ID
- [ ] Unit test: client-side retweet dedup filters `referenced_tweets` with `type: "retweeted"`
- [ ] Unit test: `extract_metric` returns 0 for missing `public_metrics`
- [ ] All existing tests continue to pass (including `should_skip_cache` with `next_token=`)
- [ ] `cargo clippy` clean
- [ ] `cargo fmt` clean

## Implementation Phases

### Phase 1: Prerequisites and Scaffolding

- [x] Update `should_skip_cache()` in `src/cache.rs` to also check `next_token=`
- [x] Add test for `should_skip_cache` with `next_token=` URL
- [x] Create `src/search.rs` with module structure, constants, and `SearchOpts` struct
- [x] Add `mod search;` to `src/main.rs`
- [x] Add `Search` variant to `Command` enum
- [x] Add dispatch arm in `run()` with clamping logic
- [x] Add `"search"` to `requirements_for_command()` and `command_names_with_auth()`

### Phase 2: Core Search Logic

- [x] Implement `apply_noise_reduction()` -- auto `-is:retweet` logic
- [x] Implement `build_search_url()` -- hardcoded field constants, query params, pagination token
- [x] Implement `is_retweet()` -- client-side retweet dedup via `referenced_tweets`
- [x] Implement `extract_metric()` helper
- [x] Implement `sort_tweets()` with `recent` and `likes` keys (fail fast on invalid)
- [x] Unit tests for all helpers

### Phase 3: Pagination and Search Handler

- [x] Implement `run_search()` with multi-page fetching loop
- [x] Per-page: auth resolution, CachedClient GET, manual cost display
- [x] Per-page: inline filtering (`min_likes`, retweet dedup, tweet ID dedup)
- [x] Accumulate tweets + included users across pages
- [x] Break on empty `data` array (phantom `next_token` handling)
- [x] 150ms sleep between pages
- [x] Post-loop: sort, build output JSON, print to stdout
- [x] Metadata summary on stderr

### Phase 4: Integration and Polish

- [x] Verify `bird doctor search` shows correct auth info
- [ ] Verify cache interaction: repeated query within TTL returns cached result
- [ ] Verify `--refresh` flag bypasses cache for search
- [x] Run `cargo clippy` and `cargo fmt`
- [x] All existing tests pass
- [x] Update `docs/CLI_DESIGN.md` with search command section

## Alternative Approaches Considered

### 1. Streaming output (like bookmarks) instead of collect-then-sort

**Rejected.** The bookmarks command streams each page to stdout as it arrives because bookmarks are consumed in order and not sorted. Search requires post-fetch sorting and filtering, which means all results must be collected in memory before output. The memory cost is negligible -- even 500 tweets at ~2KB each is ~1MB.

### 2. Struct-based tweet representation instead of serde_json::Value

**Considered.** Defining `struct Tweet { id: String, text: String, public_metrics: Option<PublicMetrics>, ... }` would give compile-time field access. However, this creates a maintenance burden as the X API adds fields, requires keeping the struct in sync with field profiles, and loses the ability to pass through unknown fields. The `serde_json::Value` approach is more flexible and matches the existing pattern in `src/raw.rs` and `src/bookmarks.rs`.

### 3. Server-side filtering via X API search operators

**Considered.** X search syntax supports operators like `min_faves:10` (undocumented, unreliable) and `min_retweets:N`. These are not part of the official API and may change without notice. Post-fetch filtering is more reliable and gives us exact control over thresholds. The tradeoff is that we fetch more data than we display, but with caching and X's 24hr dedup, the cost impact is manageable.

### 4. Front-loading all 11 flags in v1

**Rejected by simplicity review.** The original plan included `--quality`, `--archive`, `--format telegram|markdown`, `--fields`, `--min-impressions`, and 5 sort keys. This was ~296 lines of code for speculative features with no current consumer. The simplified 6-flag version covers actual use cases; every deferred feature is a one-session addition (see Extension Points table).

## Dependencies & Risks

### Dependencies

| Dependency | Type | Status |
|-----------|------|--------|
| Plan 1 (Cache Layer) | Hard dependency | Implemented (PR #2) |
| `CachedClient` | Code dependency | Exists in `src/cache.rs` |
| `cost::estimate_cost` + `cost::display_cost` | Code dependency | Exists in `src/cost.rs` |
| `should_skip_cache()` update | Prerequisite change | Needs `next_token=` check added |
| No new crate dependencies | Constraint | All needed crates already in `Cargo.toml` |

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| X API search rate limits (450 req/15min) hit during multi-page fetches | Medium | Medium | 150ms sleep between pages; 10-page cap limits to 10 requests per invocation |
| `-is:retweet` leaks retweets (known X API bug) | Medium | Low | Client-side `is_retweet()` dedup via `referenced_tweets` field |
| Phantom `next_token` on last page causes extra API call | Medium | Low | Break on empty `data` array, not just missing token |
| Duplicate tweets at page boundaries | Medium | Low | `HashSet<String>` dedup by tweet ID |
| Post-fetch sorting reverses X's dedup benefit | Low | Low | Sorting is in-memory only; does not cause additional API calls |
| Large result sets use significant memory | Low | Low | 100 tweets * 10 pages * ~2KB = ~2MB; well within CLI budget |
| `public_metrics` field becomes restricted | Low | High | Graceful degradation: `extract_metric` defaults to 0 |

## Future Considerations

Features deferred from v1 (see Extension Points table in Proposed Solution for effort estimates):

- **`--archive` flag:** Swap URL to `GET /2/tweets/search/all` (requires Pro tier, $5K/mo). 5 lines.
- **`--format telegram|markdown`:** Formatted output for humans/chat. ~80 lines in a new `format.rs`.
- **`--fields minimal|standard|full`:** Build `FieldProfile` enum when Plan 3 (Profile & Thread) needs shared field selection across commands.
- **`--quality` flag:** One-line alias for `--min-likes 10`.
- **`--min-impressions N`:** Copy the `--min-likes` pattern. 3 lines.
- **`--sort retweets|replies|impressions`:** Additional sort keys. 1 line each.
- **`--since` / `--until` date filters:** Pass `start_time` and `end_time` to the X API.
- **`--lang` language filter:** Add `lang:en` to query (available via raw query syntax today).
- **`--json-lines` output:** NDJSON for piping to `jq` line-by-line.
- **Saved searches:** Store frequently-used queries in config.toml for quick re-execution.

## References

### Internal References

- **Plan 1 (Cache Layer):** `docs/plans/2026-02-11-feat-transparent-cache-layer-plan.md`
- **Plan 1 Solution Doc:** `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
- **Brainstorm:** `docs/brainstorms/2026-02-11-research-commands-and-caching-brainstorm.md`
- **Command handler template:** `src/raw.rs` -- `run_raw()` (line 15)
- **Pagination pattern:** `src/bookmarks.rs` -- `run_bookmarks()` (line 11)
- **Auth requirements registry:** `src/requirements.rs` -- `requirements_for_command()` (line 51), `command_names_with_auth()` (line 95)
- **Auth resolution:** `src/auth.rs` -- `resolve_token_for_command()` (line 298)
- **Cache skip logic:** `src/cache.rs` -- `should_skip_cache()` (line 511)
- **Cost estimation:** `src/cost.rs` -- `estimate_cost()` + `display_cost()`
- **CLI structure:** `src/main.rs` -- `Command` enum (line 145), `run()` dispatch (line 236)
- **Output formatting helpers:** `src/output.rs` (91 lines)
- **CLI design doc:** `docs/CLI_DESIGN.md`

### External References

- **X API Tweet Search (Recent):** `GET /2/tweets/search/recent` -- returns tweets from the last 7 days matching a query
- **X API Tweet Search (All):** `GET /2/tweets/search/all` -- full-archive search (Pro tier+)
- **X API Search Query Syntax:** Operators like `-is:retweet`, `from:username`, `has:links`, `lang:en`
- **X API Tweet Fields:** `tweet.fields` parameter -- `created_at`, `public_metrics`, `author_id`, `conversation_id`, `referenced_tweets`, etc.
- **X API Expansions:** `expansions=author_id` returns user objects in `includes.users`
- **X API Rate Limits:** 450 req/15min (recent, app auth), 300 req/15min (recent, user auth)
- **X API Billing:** $0.005 per tweet read, $0.010 per user read, 24hr UTC dedup window
