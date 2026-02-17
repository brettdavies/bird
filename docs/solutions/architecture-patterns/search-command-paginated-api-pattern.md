---
title: "Search Command: Paginated API with Client-Side Processing"
category: architecture-patterns
tags: [search, pagination, filtering, deduplication, caching, cost-estimation, noise-reduction]
module: src/search.rs
symptom: "Need to implement tweet search with client-side filtering, cross-page deduplication, sorting, and cost-aware pagination"
root_cause: "X API /2/tweets/search/recent requires a wrapper to handle multi-page pagination, client-side filtering, deduplication, cost estimation, and noise reduction"
severity: feature
date_solved: "2026-02-11"
related_prs: [3]
---

# Search Command: Paginated API with Client-Side Processing

## Problem

Bird CLI needed a `bird search <query>` command wrapping `GET /2/tweets/search/recent` with research-oriented features: client-side filtering (retweet removal, min-likes threshold), cross-page deduplication, sorting, and cost-aware pagination. The X API endpoint has known quirks (retweet leaks, phantom pagination tokens, duplicate tweets across pages) that require defensive handling.

## Solution

### 1. Parameter Bundling

Bundle command parameters into a struct to avoid `clippy::too_many_arguments`:

```rust
pub struct SearchOpts<'a> {
    pub query: &'a str,
    pub pretty: bool,
    pub sort: &'a str,
    pub min_likes: Option<u64>,
    pub max_results: u32,
    pub pages: u32,
}
```

### 2. Accumulate-Then-Output (Not Streaming)

Unlike bookmarks (streaming output), search accumulates all results before output because client-side processing requires the complete dataset:

- Client-side sorting requires seeing all tweets before output
- Cross-page deduplication needs seen_ids tracking
- min-likes filtering changes the result set

```rust
let mut all_tweets: Vec<serde_json::Value> = Vec::new();
let mut seen_ids: HashSet<String> = HashSet::new();

for page_num in 1..=opts.pages {
    // fetch, filter, dedup per page
    for tweet in data {
        if seen_ids.insert(id.to_string()) {
            all_tweets.push(tweet.clone());
        }
    }
}

sort_tweets(&mut all_tweets, opts.sort);
println!("{}", serde_json::to_string(&output)?);
```

### 3. Dual Deduplication

Maintain separate `HashSet` instances for tweets and users:

```rust
let mut seen_ids: HashSet<String> = HashSet::new();
let mut seen_user_ids: HashSet<String> = HashSet::new();

// HashSet::insert() returns true for new entries, false for duplicates
if !uid.is_empty() && seen_user_ids.insert(uid.to_string()) {
    all_users.push(user.clone());
}
```

### 4. Token-Based Query Operator Detection

Use `split_whitespace` token matching to avoid false positives on substrings like `"crisis:retweet"`:

```rust
fn apply_noise_reduction(query: &str) -> String {
    let has_retweet_op = query
        .split_whitespace()
        .any(|t| t == "is:retweet" || t == "-is:retweet");
    if has_retweet_op {
        query.to_string()
    } else {
        format!("{} -is:retweet", query)
    }
}
```

### 5. Defense-in-Depth Retweet Filtering

The X API's `-is:retweet` operator can leak retweets (known bug). Client-side filter backs it up:

```rust
fn is_retweet(tweet: &serde_json::Value) -> bool {
    tweet.get("referenced_tweets")
        .and_then(|rt| rt.as_array())
        .map(|arr| arr.iter().any(|r|
            r.get("type").and_then(|t| t.as_str()) == Some("retweeted")
        ))
        .unwrap_or(false)
}
```

### 6. Cache Integration

- First page (no `next_token`) is cacheable through `CachedClient`
- Subsequent pages skipped via `should_skip_cache()` checking `next_token=`
- `cache_hit` propagated from `ApiResponse` to cost estimation
- OAuth1 GET requests go through the cache (signing deferred until cache miss)

### 7. Dedicated Auth Constant

Each curated command gets its own auth constant, even if values currently match `RAW_ACCEPTED`:

```rust
const SEARCH_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
```

This makes auth requirements explicit and independently evolvable.

### 8. Defensive OAuth1 Credential Access

Use `ok_or()` instead of `unwrap()` even when `resolve_token_for_command` guarantees values exist:

```rust
let ck = config.oauth1_consumer_key.as_ref()
    .ok_or("OAuth1 consumer key missing")?;
```

## Review Findings and Fixes

Key learnings from 7-agent code review:

| Finding | Severity | Fix |
|---------|----------|-----|
| `unwrap_or(Null)` silently swallows JSON parse errors | P1 | Use `?` operator |
| Cost estimation hardcoded `cache_hit: false` | P2 | Propagate from `ApiResponse` |
| Substring matching for query operators | P2 | Token-based `split_whitespace` |
| OAuth1 `unwrap()` could panic | P2 | `ok_or()` with descriptive message |
| Users not deduplicated across pages | P2 | `seen_user_ids: HashSet<String>` |
| Test tested HashSet (stdlib), not search code | P2 | Replaced with edge case test |
| `--cache-ttl` unbounded, u64-to-i64 overflow | Security | Capped at 86400s (24h) |
| Reused `RAW_ACCEPTED` for search auth | Pattern | Dedicated `SEARCH_ACCEPTED` |

## Prevention: Checklist for New Curated Commands

### Error Handling
- [ ] No silent failures: never `unwrap_or(default)` on parse operations; use `?`
- [ ] Invariants have guards: use `ok_or()` not `unwrap()` for config values
- [ ] Numeric inputs bounded: cap unbounded `u64` to prevent overflow

### API Integration
- [ ] Dedicated auth constant in `requirements.rs`
- [ ] `cache_hit` propagated from response to cost estimation
- [ ] Query operator detection is token-based, not substring

### Pagination
- [ ] All entity types deduplicated across pages (tweets AND users)
- [ ] Pagination URLs excluded from cache (`next_token=`, `pagination_token=`)
- [ ] Empty data array breaks loop (handles phantom `next_token`)

### Testing
- [ ] Tests exercise actual code paths, not stdlib behavior
- [ ] Edge cases for operator detection (substring false positives)
- [ ] Sort validation tested (or eliminated via enum)

## Common Pitfalls with Paginated API Responses

| Pitfall | Impact | Prevention |
|---------|--------|------------|
| No dedup across pages | Duplicate results in output | `HashSet<Id>` per entity type |
| Hardcoded cache_hit | Wrong cost estimates | Propagate from `ApiResponse` |
| Substring operator matching | False positives on queries | Token-based `split_whitespace` |
| Unbounded numeric params | Integer overflow | Validate and cap at parse time |
| Reused auth constants | Wrong auth applied to command | Dedicated constant per command |
| `unwrap_or(Null)` on JSON parse | Silent data loss | Always use `?` operator |

## Related Documentation

- [SQLite Cache Layer](../performance-issues/sqlite-cache-layer-api-cost-reduction.md) -- CachedClient, cost estimation, cache skip rules
- [Security Audit](../security-issues/rust-cli-security-code-quality-audit.md) -- BirdError, shared client, token permissions
- [CLI Design](../../CLI_DESIGN.md) -- Auth requirements, doctor command, error messaging
- PR #3: feat(search) -- https://github.com/brettdavies/bird/pull/3

## Files Changed

| File | Change |
|------|--------|
| `src/search.rs` | New: search handler, helpers, 13 unit tests |
| `src/main.rs` | Added `Command::Search` variant and dispatch |
| `src/requirements.rs` | Added `SEARCH_ACCEPTED` constant and registry entry |
| `src/cache.rs` | Added `next_token=` to `should_skip_cache()`, TTL cap |
| `docs/CLI_DESIGN.md` | Added search to command list and auth table |
