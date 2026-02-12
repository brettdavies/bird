---
title: "feat: Profile and Thread Commands"
type: feat
date: 2026-02-11
series: "Research Commands & Caching Layer"
plan: 3 of 4
depends_on: "2026-02-11-feat-transparent-cache-layer-plan"
deepened: 2026-02-11
---

## Enhancement Summary

**Deepened on:** 2026-02-11
**Sections enhanced:** 12
**Research agents used:** Architecture Strategist, Security Sentinel, Performance Oracle, Pattern Recognition Specialist, Code Simplicity Reviewer, Agent-Native Reviewer, Spec Flow Analyzer, Best Practices Researcher, Framework Docs Researcher, Repo Research Analyst, Learnings Researcher

### Key Improvements

1. **CRITICAL handler signature fixes** -- `&CachedClient` must be `&mut CachedClient`, missing `use_color: bool` parameter, missing `max_pages` in `run_thread` signature, need `ProfileOpts`/`ThreadOpts` structs to avoid `clippy::too_many_arguments`
2. **Security hardening** -- conversation_id validation before search query injection, X API returns HTTP 200 with `errors` array for not-found users (not 404), strip leading `@` from username, sanitize terminal output in human-readable formats
3. **Scope simplification** -- defer `--fields` and `--format` flags to v2 (no existing command has them, YAGNI), reduce `--max-pages` cap from 100 to 25, defer user map to v2

### New Considerations Discovered

- X API user-not-found returns HTTP 200 with `errors[]` array, NOT a 404 status code
- `conversation_id` search does NOT return the root tweet -- must be fetched separately and merged
- OAuth1 auth for thread should be INCLUDED (search endpoint supports it via `reqwest_oauth1`), matching `SEARCH_ACCEPTED`
- Thread tree should use iterative DFS (not recursive) to prevent stack overflow on deep threads
- Phantom `next_token` defense needed (break on empty `data` array), matching search command pattern
- Need 150ms inter-page delay for thread pagination, matching search command pattern
- Circular reference guard needed in BFS depth computation

---

# feat: Profile and Thread Commands

## Overview

Add two new research commands to Bird: `bird profile <username>` for user profile lookup and `bird thread <tweet_id>` for reconstructing conversation threads. Both commands build on Plan 1's transparent cache layer (`CachedClient`, field profiles, cost tracking, rate limiting) and are designed for research workflows where an agent or human needs to quickly inspect a user's public presence or follow a conversation.

**Plan series:**

| # | Plan | Status |
|---|------|--------|
| 1 | Transparent Cache Layer | Prerequisite (active) |
| 2 | Search Command | Blocked by Plan 1 |
| **3** | **Profile & Thread Commands** (this plan) | Blocked by Plan 1 |
| 4 | Watchlist & Usage Commands | Blocked by Plan 1 |

## Problem Statement

Research workflows on X/Twitter frequently require two operations that Bird currently cannot perform directly:

1. **Profile inspection:** Understanding who is behind a tweet -- their bio, follower count, account age, verified status -- is essential context for evaluating tweet credibility and deciding whether to follow a thread. Currently, a user must either open a browser or use `bird get /2/users/by/username/{username} -p username=elonmusk --query user.fields=...` with manually-specified field parameters. This is error-prone, requires knowing the exact API parameter names, and bypasses cost tracking and caching.

2. **Thread following:** Conversations on X are fragmented. A single tweet may be part of a long thread or a deep reply chain. Reconstructing the full conversation requires multiple API calls: fetching the root tweet to obtain its `conversation_id`, then searching for all tweets in that conversation, then sorting them into chronological order with reply relationships intact. This multi-step process is exactly the kind of workflow that should be automated behind a single command.

**Cost implications:**

- A profile lookup costs $0.010 (1 user object). With Plan 1's 1-hour cache TTL on user endpoints, repeated lookups within an hour are free.
- A thread reconstruction costs $0.005 per tweet in the thread. A 50-tweet thread costs $0.25 on first fetch but $0.00 on subsequent fetches within the 15-minute search cache TTL. The 24-hour X API billing dedup means re-fetching after cache expiry is also free.

## Proposed Solution

Two new command modules (`src/profile.rs` and `src/thread.rs`) that use `CachedClient` from Plan 1 for all HTTP requests. Both follow the established handler pattern from `src/search.rs` (opts struct, `&mut CachedClient`, `use_color` parameter).

### Profile Command

```
bird profile <username> [--pretty]
```

A single API call to `GET /2/users/by/username/{username}` with standard fields. Outputs JSON by default.

> **v1 scope note:** `--fields` and `--format` flags are deferred to v2. No existing Bird command has these flags, and adding them now would be a YAGNI violation. The standard field set covers the research use case. If needed later, they can be added without breaking changes.

### Thread Command

```
bird thread <tweet_id> [--pretty] [--max-pages N]
```

A two-step fetch:
1. Get the root tweet to extract `conversation_id`
2. Search for all tweets in the conversation via `GET /2/tweets/search/recent?query=conversation_id:{id}`
3. Build a thread tree from `referenced_tweets` relationships
4. Output in chronological order

> **v1 scope note:** `--format readable` is deferred to v2. It requires building a user map from includes, terminal-safe output sanitization, and SI-suffix formatting -- significant complexity for a niche use case. JSON output with thread metadata serves both agents and humans who can pipe to `jq`.

## Technical Approach

### Profile Command (`src/profile.rs`)

**New file:** `src/profile.rs` (~80 lines)

**Handler signature:**

```rust
// src/profile.rs
pub struct ProfileOpts<'a> {
    pub username: &'a str,
    pub pretty: bool,
}

pub async fn run_profile(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    opts: ProfileOpts<'_>,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
```

### Research Insights: Profile Handler

**Signature corrections (from pattern recognition review):**
- Must be `&mut CachedClient` (not `&CachedClient`) -- all existing handlers use `&mut`
- Must include `use_color: bool` parameter -- all existing handlers include it
- Must use opts struct -- follows `SearchOpts` pattern from `src/search.rs`, avoids `clippy::too_many_arguments`

**API endpoint:** `GET /2/users/by/username/{username}`

**Query parameters (standard set):**
```
user.fields=created_at,public_metrics,description,profile_image_url,location,verified,url
```

**Auth:** OAuth2User, OAuth1, or Bearer -- same as raw GET commands. All three auth types support the users/by endpoint per the X API spec.

**Cache behavior:**
- Endpoint pattern `/2/users/by/*` matches Plan 1's 1-hour TTL (see Plan 1, "Per-Endpoint TTL Defaults" table)
- Cache key includes username, auth type, and effective user (from Plan 1's cache key design)
- `--refresh` bypasses cache (passes through to `CachedClient`)

**Implementation flow:**

```
run_profile(client, config, opts, use_color)
  |
  +--> Strip leading '@' from username if present
  |
  +--> Validate username (alphanumeric + underscore, 1-15 chars)
  |
  +--> Build URL:
  |      https://api.x.com/2/users/by/username/{username}?user.fields=created_at,...
  |
  +--> Resolve auth token via resolve_token_for_command(client.http(), config, "profile")
  |
  +--> Branch on auth type (Bearer vs OAuth1 vs OAuth2) -- follow raw.rs pattern
  |
  +--> Parse response JSON with `?` operator (never unwrap_or)
  |
  +--> Check for `errors` array in response body (NOT HTTP 404!)
  |      X API returns HTTP 200 with {"errors":[{"detail":"..."}]} for not-found users
  |
  +--> Display cost on stderr via cost module
  |
  +--> Format output: compact JSON or pretty-printed JSON
  |
  +--> Ok(())
```

**Username validation:**

X usernames are 1-15 characters, alphanumeric plus underscores. Validate before making the API call to fail fast with a clear error rather than a cryptic API response.

```rust
fn validate_username(username: &str) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
    // Strip leading '@' for convenience (users naturally type @username)
    let username = username.strip_prefix('@').unwrap_or(username);
    if username.is_empty() || username.len() > 15 {
        return Err(format!("username must be 1-15 characters, got {}", username.len()).into());
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("username contains invalid characters: {}", username).into());
    }
    Ok(username)
}
```

### Research Insights: Profile Error Handling

**CRITICAL -- User not-found returns HTTP 200 (from best-practices research):**

The X API does NOT return HTTP 404 for nonexistent users. Instead, it returns HTTP 200 with an `errors` array:

```json
{
  "errors": [
    {
      "value": "nonexistent_user",
      "detail": "Could not find user with username: [nonexistent_user].",
      "title": "Not Found Error",
      "resource_type": "user",
      "parameter": "username",
      "type": "https://api.twitter.com/2/problems/resource-not-found"
    }
  ]
}
```

The handler MUST check for the `errors` array in the response body, not rely on HTTP status codes. Pattern:

```rust
let body: serde_json::Value = serde_json::from_str(&text)?;
if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
    if let Some(err) = errors.first() {
        let detail = err.get("detail").and_then(|d| d.as_str()).unwrap_or("unknown error");
        return Err(format!("profile failed: {}", detail).into());
    }
}
```

**Security (from security review):**
- Username validation prevents path traversal (already handled by alphanumeric check)
- Do NOT echo untrusted usernames in error messages verbatim if outputting to terminal -- the validation step ensures only safe characters pass through

### Thread Command (`src/thread.rs`)

**New file:** `src/thread.rs` (~180 lines)

**Handler signature:**

```rust
// src/thread.rs
pub struct ThreadOpts<'a> {
    pub tweet_id: &'a str,
    pub pretty: bool,
    pub max_pages: u32,
}

pub async fn run_thread(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    opts: ThreadOpts<'_>,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
```

### Research Insights: Thread Handler

**Signature corrections (from pattern recognition + spec flow reviews):**
- Must be `&mut CachedClient` (not `&CachedClient`)
- Must include `use_color: bool` parameter
- Must include `max_pages` in opts struct (was missing from original handler signature)
- Use `ThreadOpts` struct to match `SearchOpts` pattern

**Auth:** OAuth2User, OAuth1, or Bearer.

> **Auth correction (from architecture review):** The original plan excluded OAuth1 from thread, but the search endpoint IS accessible via OAuth1 through `reqwest_oauth1` (same as `SEARCH_ACCEPTED` in search.rs). Thread should use the same auth types as search for consistency. The existing search command successfully uses OAuth1 with the search endpoint.

```rust
const THREAD_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
```

**Cache behavior:**
- Root tweet fetch: `/2/tweets/{id}` has 15-minute TTL (Plan 1)
- Conversation search: `/2/tweets/search/recent` has 15-minute TTL (Plan 1)
- First search page is cacheable; subsequent pages skipped via `should_skip_cache()` checking `next_token=`
- `cache_hit` propagated from `ApiResponse` to cost estimation (matching search pattern)

**Two-step fetch approach:**

Step 1 -- Fetch root tweet:

```
GET /2/tweets/{tweet_id}?tweet.fields=conversation_id,author_id,created_at,text,public_metrics,referenced_tweets
                        &expansions=author_id
                        &user.fields=username,name
```

Extract `conversation_id` from the response. The `conversation_id` equals the tweet ID of the first tweet in the thread. If the provided `tweet_id` IS the root tweet, `conversation_id == tweet_id`. If the provided tweet is a reply in the thread, `conversation_id` points to the root.

**CRITICAL (from best-practices research):** The conversation_id search does NOT return the root tweet itself. The root tweet must be fetched separately in Step 1 and merged into the result set.

Step 2 -- Search for conversation tweets (paginated):

```
GET /2/tweets/search/recent?query=conversation_id:{conversation_id}
                            &tweet.fields=conversation_id,author_id,created_at,text,public_metrics,referenced_tweets,in_reply_to_user_id
                            &expansions=author_id
                            &user.fields=username,name
                            &max_results=100
                            &sort_order=recency
```

Paginate until no `next_token` or `--max-pages` limit reached.

### Research Insights: conversation_id Validation (Security)

**HIGH priority (from security review):**

The `conversation_id` extracted from the root tweet response is injected into the search query string: `conversation_id:{id}`. If the API returned a malformed conversation_id (or if the response was tampered with in a MITM scenario), this could inject arbitrary search operators.

**Mitigation:** Validate `conversation_id` is a pure numeric string before using it in the query:

```rust
fn validate_tweet_id(id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if id.is_empty() || id.len() > 20 {
        return Err(format!("tweet ID must be 1-20 digits, got {}", id.len()).into());
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("tweet ID must be numeric: {}", id).into());
    }
    Ok(())
}
```

Apply this validation to BOTH the user-provided `tweet_id` AND the API-returned `conversation_id` before constructing the search query.

### Research Insights: Thread Pagination Safety

**From multiple reviews:**

- **Reduce `--max-pages` cap from 100 to 25** (security + performance reviews) -- 100 pages at 100 tweets/page = 10,000 tweets = $50 API cost. Cap at 25 pages (2,500 tweets, $12.50 max) which covers nearly all real threads while preventing accidental cost blow-up. Users who truly need more can use `bird get` directly.
- **150ms inter-page delay** (performance review) -- match the existing pattern in `src/search.rs` for polite pagination
- **Phantom `next_token` defense** (spec flow review) -- break on empty `data` array even if `next_token` is present, matching search command pattern
- **Dual deduplication** (from search pattern) -- deduplicate both tweets AND users across pages using `HashSet<String>`

**Important limitation:** The search/recent endpoint only covers the last 7 days. Threads older than 7 days will return partial or empty results. The command should warn the user when the root tweet's `created_at` is older than 7 days.

**Thread tree building:**

```rust
struct ThreadNode {
    tweet: serde_json::Value,  // the tweet data
    tweet_id: String,
    parent_id: Option<String>, // from referenced_tweets where type == "replied_to"
    depth: usize,              // 0 for root, incremented for replies
    children: Vec<usize>,      // indices into flat Vec<ThreadNode>
}
```

### Research Insights: Tree Data Structure

**From framework docs research:**
- `Vec<ThreadNode>` + `HashMap<String, usize>` (index-based arena) is idiomatic Rust for trees -- avoids lifetime complexity of reference-based trees
- `VecDeque` for BFS depth computation is idiomatic (from `std::collections`)
- `serde_json::Value` is acceptable for CLI passthrough (no need for typed structs)

**From performance review:**
- Convert recursive DFS (`flatten_thread`) to iterative with explicit stack to prevent stack overflow on deeply nested threads (1000+ depth possible on viral conversations)
- Use lexicographic string comparison for `created_at` sorting (ISO 8601 format sorts correctly as strings)
- ~10MB peak memory at 10,000 tweets is acceptable for a CLI tool

**From spec flow review -- circular reference guard:**
- Add a `visited: HashSet<usize>` in BFS depth computation to guard against circular references (shouldn't happen with valid API data, but defensive programming)

```rust
fn build_thread_tree(
    root_tweet: &serde_json::Value,
    search_tweets: &[serde_json::Value],
) -> Vec<ThreadNode> {
    // 1. Create a flat list of all tweets (root + search results)
    let mut nodes: Vec<ThreadNode> = Vec::new();
    let mut id_to_index: HashMap<String, usize> = HashMap::new();

    // 2. Insert root tweet at index 0
    let root_id = root_tweet["id"].as_str()
        .ok_or("root tweet missing id")?
        .to_string();
    nodes.push(ThreadNode {
        tweet: root_tweet.clone(),
        tweet_id: root_id.clone(),
        parent_id: None,
        depth: 0,
        children: vec![],
    });
    id_to_index.insert(root_id, 0);

    // 3. Insert search result tweets, deduplicating against root
    for tweet in search_tweets {
        let id = tweet["id"].as_str()
            .ok_or("tweet missing id")?
            .to_string();
        if id_to_index.contains_key(&id) {
            continue; // skip duplicate (root tweet may appear in search)
        }
        let parent_id = tweet.get("referenced_tweets")
            .and_then(|refs| refs.as_array())
            .and_then(|refs| refs.iter().find(|r| r["type"] == "replied_to"))
            .and_then(|r| r["id"].as_str())
            .map(String::from);

        let idx = nodes.len();
        nodes.push(ThreadNode {
            tweet: tweet.clone(),
            tweet_id: id.clone(),
            parent_id,
            depth: 0, // computed in step 4
            children: vec![],
        });
        id_to_index.insert(id, idx);
    }

    // 4. Build parent-child relationships
    for i in 0..nodes.len() {
        if let Some(ref parent_id) = nodes[i].parent_id {
            if let Some(&parent_idx) = id_to_index.get(parent_id) {
                nodes[parent_idx].children.push(i);
            }
        }
    }

    // 5. Compute depths via BFS from root (with circular reference guard)
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    let mut visited: HashSet<usize> = HashSet::new();
    queue.push_back((0, 0)); // (index, depth)
    visited.insert(0);
    while let Some((idx, depth)) = queue.pop_front() {
        nodes[idx].depth = depth;
        // Sort children by created_at (lexicographic works for ISO 8601)
        nodes[idx].children.sort_by(|&a, &b| {
            let a_time = nodes[a].tweet["created_at"].as_str().unwrap_or("");
            let b_time = nodes[b].tweet["created_at"].as_str().unwrap_or("");
            a_time.cmp(b_time)
        });
        for &child_idx in &nodes[idx].children.clone() {
            if visited.insert(child_idx) {
                queue.push_back((child_idx, depth + 1));
            }
        }
    }

    nodes
}

// 6. Flatten tree into DFS order -- ITERATIVE (not recursive) to avoid stack overflow
fn flatten_thread(nodes: &[ThreadNode]) -> Vec<usize> {
    let mut result = Vec::new();
    let mut stack = vec![0usize]; // start from root
    while let Some(idx) = stack.pop() {
        result.push(idx);
        // Push children in reverse order so first child is processed first
        for &child_idx in nodes[idx].children.iter().rev() {
            stack.push(child_idx);
        }
    }
    result
}
```

**Implementation flow:**

```
run_thread(client, config, opts, use_color)
  |
  +--> Validate tweet_id (numeric string, 1-20 digits) via validate_tweet_id()
  |
  +--> Resolve auth token via resolve_token_for_command(client.http(), config, "thread")
  |
  +--> Step 1: Fetch root tweet
  |      GET /2/tweets/{tweet_id}?tweet.fields=conversation_id,...
  |      via client.get() or client.http().clone().oauth1() for OAuth1
  |      Use ok_or() for OAuth1 credentials (never unwrap)
  |
  +--> Check for errors array in response (not-found returns 200 + errors)
  |
  +--> Extract conversation_id from response
  |      Validate conversation_id is numeric via validate_tweet_id()
  |      If conversation_id != tweet_id, note on stderr: "following to root tweet {conversation_id}"
  |      If root tweet created_at > 7 days ago, warn on stderr about partial results
  |
  +--> Step 2: Search for conversation tweets (paginated)
  |      let mut all_tweets: Vec<serde_json::Value> = Vec::new();
  |      let mut seen_ids: HashSet<String> = HashSet::new();
  |      let mut seen_user_ids: HashSet<String> = HashSet::new();
  |      seed seen_ids with root tweet ID
  |
  |      for page_num in 1..=opts.max_pages {
  |        GET /2/tweets/search/recent?query=conversation_id:{conversation_id}&max_results=100
  |        Parse response with ? operator (never unwrap_or)
  |        Propagate cache_hit to cost estimation
  |        Display per-page cost on stderr
  |
  |        if data array is empty: break (phantom next_token defense)
  |
  |        Deduplicate tweets: seen_ids.insert(id)
  |        Deduplicate users: seen_user_ids.insert(uid)
  |
  |        if no next_token: break
  |        if page_num < opts.max_pages: sleep 150ms (polite pagination)
  |      }
  |
  +--> Build thread tree (root tweet + all search result tweets)
  |
  +--> Flatten tree in iterative DFS order
  |
  +--> Build output JSON:
  |      {
  |        "thread": [ ... tweets in thread order with depth ... ],
  |        "meta": {
  |          "conversation_id": "...",
  |          "tweet_count": N,
  |          "pages_fetched": N,
  |          "complete": bool,       // true if no truncation
  |          "root_tweet_age_days": N // for 7-day window awareness
  |        }
  |      }
  |
  +--> Format output: compact JSON or pretty-printed JSON
  |
  +--> Ok(())
```

### Research Insights: Thread Output Schema (Agent-Native)

**From agent-native review:**

The `meta` object should include machine-readable completeness indicators so agents can programmatically determine if they have the full conversation:

- `complete: bool` -- `true` if all pages fetched without hitting `max_pages` limit
- `pages_fetched: u32` -- how many search pages were consumed
- `root_tweet_age_days: u32` -- days since root tweet was posted (for 7-day window awareness)

This enables agent logic like: "if not complete, increase max_pages and retry" or "if root_tweet_age_days > 7, results are inherently incomplete."

**Output format:**

Default JSON output:

```json
{
  "thread": [
    {"id":"100","text":"Root tweet","author_id":"1","depth":0,"created_at":"2026-02-11T10:00:00Z"},
    {"id":"101","text":"Reply from bob","author_id":"2","depth":1,"created_at":"2026-02-11T10:05:00Z"},
    {"id":"102","text":"Alice responds","author_id":"1","depth":2,"created_at":"2026-02-11T10:10:00Z"}
  ],
  "meta": {
    "conversation_id": "100",
    "tweet_count": 3,
    "pages_fetched": 1,
    "complete": true,
    "root_tweet_age_days": 0
  }
}
```

**Pagination safety:**

The `--max-pages` flag (default: 10, maximum: 25) prevents runaway pagination on viral threads with thousands of replies. At 100 tweets per page, the default limit of 10 pages covers threads up to 1,000 tweets. Cost at max: 2,500 tweets * $0.005 = $12.50. The cost display on stderr (from Plan 1) makes this visible.

### Command Enum Additions

**File:** `src/main.rs` (lines 118-191, `Command` enum)

Add two new variants to the `Command` enum:

```rust
/// Look up a user profile by username
Profile {
    /// X/Twitter username (with or without @)
    username: String,
    /// Human-readable output
    #[arg(long)]
    pretty: bool,
},

/// Reconstruct a conversation thread from a tweet
Thread {
    /// Tweet ID (root tweet or any reply in the thread)
    tweet_id: String,
    /// Human-readable output
    #[arg(long)]
    pretty: bool,
    /// Maximum number of search result pages (default: 10, max: 25)
    #[arg(long, default_value = "10")]
    max_pages: u32,
},
```

### Run Dispatch Additions

**File:** `src/main.rs` (lines 193-249, `run()` function)

Add match arms in the `run()` function, following the existing pattern:

```rust
Command::Profile { username, pretty } => {
    profile::run_profile(
        client,
        &config,
        profile::ProfileOpts {
            username: &username,
            pretty,
        },
        use_color,
    )
    .await
    .map_err(|e| BirdError::Command { name: "profile", source: e })?;
}
Command::Thread { tweet_id, pretty, max_pages } => {
    thread::run_thread(
        client,
        &config,
        thread::ThreadOpts {
            tweet_id: &tweet_id,
            pretty,
            max_pages,
        },
        use_color,
    )
    .await
    .map_err(|e| BirdError::Command { name: "thread", source: e })?;
}
```

### Module Declarations

**File:** `src/main.rs` (lines 1-11, module declarations)

Add (in alphabetical order, matching existing convention):

```rust
mod profile;
mod thread;
```

### Auth Requirements

**File:** `src/requirements.rs`

**Profile command:**

```rust
const PROFILE_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
```

Profile accepts all three auth types because `GET /2/users/by/username/{username}` supports OAuth 2.0 User, OAuth 1.0a User, and Bearer (app-only) per the X API spec.

**Thread command:**

```rust
const THREAD_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
```

### Research Insights: Thread Auth

**Correction from architecture review:** The original plan excluded OAuth1 from thread, reasoning that the search endpoint doesn't support it. However, `SEARCH_ACCEPTED` in `src/search.rs` already includes `AuthType::OAuth1`, and the search command successfully uses OAuth1 via `reqwest_oauth1`. Thread uses the same search endpoint and should accept the same auth types for consistency. Using different auth for the same underlying endpoint would confuse users.

**Add to `requirements_for_command()`:**

```rust
"profile" => CommandReqs {
    accepted: PROFILE_ACCEPTED,
    oauth2_hint: OAUTH2_HINT,
    oauth1_hint: OAUTH1_HINT,
    bearer_hint: BEARER_HINT,
},
"thread" => CommandReqs {
    accepted: THREAD_ACCEPTED,
    oauth2_hint: OAUTH2_HINT,
    oauth1_hint: OAUTH1_HINT,
    bearer_hint: BEARER_HINT,
},
```

**Add to `command_names_with_auth()`:**

```rust
pub fn command_names_with_auth() -> &'static [&'static str] {
    &["login", "me", "bookmarks", "search", "profile", "thread", "get", "post", "put", "delete"]
}
```

### How Both Commands Use Plan 1 Infrastructure

| Plan 1 Component | Profile Usage | Thread Usage |
|---|---|---|
| `CachedClient.get()` | Single GET for user lookup | Root tweet GET + paginated search GETs |
| Cache TTL matching | `/2/users/by/*` -> 1hr TTL | `/2/tweets/{id}` -> 15min, `/2/tweets/search/*` -> 15min |
| Cost tracking (stderr) | Shows "$0.01 (1 user)" | Shows "$0.05 (10 tweets)" per page |
| `--refresh` | Bypasses 1hr user cache | Bypasses both tweet and search caches |
| `--no-cache` | Skips cache entirely | Skips cache entirely |
| `should_skip_cache()` | Not applicable | Skips cache for pages with `next_token=` |

### Token Resolution

Both commands use the existing `resolve_token_for_command()` from `src/auth.rs`. This function accepts `client.http()` (the inner `reqwest::Client`) for token refresh operations that bypass the cache.

```rust
// In src/profile.rs
let token = resolve_token_for_command(client.http(), config, "profile").await?;

// In src/thread.rs
let token = resolve_token_for_command(client.http(), config, "thread").await?;
```

For OAuth1 paths, use `ok_or()` (never `unwrap()`) for credential access, matching the pattern in `src/search.rs`:

```rust
let ck = config.oauth1_consumer_key.as_ref()
    .ok_or("OAuth1 consumer key missing")?;
```

## Acceptance Criteria

### Functional Requirements

- [ ] `bird profile <username>` fetches and displays user profile data as JSON
- [ ] `bird profile @username` strips the leading `@` automatically
- [ ] `bird profile` validates username format (1-15 chars, alphanumeric + underscore) before API call
- [ ] `bird profile` handles not-found users via `errors` array in HTTP 200 response (not HTTP 404)
- [ ] `bird profile` uses Plan 1's 1-hour cache TTL for user endpoints
- [ ] `bird thread <tweet_id>` reconstructs a conversation thread in chronological order
- [ ] `bird thread` validates tweet_id format (numeric, 1-20 digits) before API call
- [ ] `bird thread` validates conversation_id extracted from API response before using in search query
- [ ] `bird thread` handles the case where the input tweet is a reply (not the root) by following `conversation_id` to the root
- [ ] `bird thread` paginates the conversation search with `--max-pages` safety limit (default 10, max 25)
- [ ] `bird thread` deduplicates tweets AND users across pages using `HashSet<String>`
- [ ] `bird thread` breaks on empty `data` array (phantom `next_token` defense)
- [ ] `bird thread` sleeps 150ms between paginated search requests
- [ ] `bird thread` warns on stderr when the root tweet is older than 7 days (search/recent limitation)
- [ ] `bird thread` output includes `meta` object with `complete`, `pages_fetched`, `root_tweet_age_days`
- [ ] Both commands respect `--pretty` for pretty-printed JSON output
- [ ] Both commands display cost estimates on stderr via Plan 1's cost tracking
- [ ] Both commands appear in `bird doctor` output with correct availability status
- [ ] Both commands support `--refresh`, `--no-cache`, `--cache-ttl` (inherited from Plan 1 global flags)

### Error Handling

- [ ] Username not found: check `errors` array in HTTP 200 response, return descriptive error
- [ ] Tweet not found: check `errors` array in HTTP 200 response, return descriptive error
- [ ] Suspended/protected user returns descriptive error from API `errors` array
- [ ] Empty conversation search (no replies) returns just the root tweet with `meta.tweet_count: 1`
- [ ] Auth failures use the existing `AuthRequiredError` pattern with command-specific hints
- [ ] Network/timeout errors propagate through `BirdError::Command` with exit code 1
- [ ] All JSON parsing uses `?` operator (never `unwrap_or` on parse operations)
- [ ] OAuth1 credentials accessed via `ok_or()` (never `unwrap()`)

### Non-Functional Requirements

- [ ] `src/profile.rs` stays under 100 lines (single responsibility: one API call, format, output)
- [ ] `src/thread.rs` stays under 200 lines (thread tree building is the complex part)
- [ ] No new dependencies beyond what Plan 1 introduces
- [ ] Both commands use `&mut CachedClient` and `use_color: bool` in handler signatures
- [ ] Both commands use opts structs (`ProfileOpts`, `ThreadOpts`)

### Quality Gates

- [ ] Unit tests for username validation (including leading `@` stripping)
- [ ] Unit tests for tweet_id validation (including conversation_id validation)
- [ ] Unit tests for thread tree building with various topologies:
  - [ ] Linear thread (A -> B -> C)
  - [ ] Branching replies (A -> B, A -> C, B -> D)
  - [ ] Single tweet, no replies
  - [ ] Missing parent references (orphaned tweets from outside the 7-day window)
  - [ ] Root tweet appearing in search results (deduplication)
  - [ ] Circular reference handling (defensive guard)
- [ ] Unit tests for `is_retweet()` filter if applicable
- [ ] `cargo clippy` clean
- [ ] All existing tests continue to pass
- [ ] `cargo test` passes for new modules

## Implementation Phases

### Phase 1: Profile Command (simpler, establishes pattern)

**Estimated effort:** ~80 lines of new code + ~20 lines of modifications to existing files.

- [x] Create `src/profile.rs` with `ProfileOpts` struct and `run_profile()` handler
  - [x] Username validation function (with `@` stripping)
  - [x] URL construction with standard user.fields
  - [x] Auth token resolution
  - [x] Bearer/OAuth1/OAuth2 branching (following raw.rs pattern)
  - [x] API call via `CachedClient.get()` or `client.http().clone().oauth1()` for OAuth1
  - [x] Response parsing: check for `errors` array first, then extract `data`
  - [x] Cost display on stderr
  - [x] Default JSON output (compact and pretty)
- [x] Add `mod profile;` to `src/main.rs` (alphabetical order)
- [x] Add `Profile` variant to `Command` enum in `src/main.rs`
- [x] Add `Profile` match arm to `run()` in `src/main.rs`
- [x] Add `PROFILE_ACCEPTED` and `"profile"` entry to `src/requirements.rs`
- [x] Add `"profile"` to `command_names_with_auth()` in `src/requirements.rs`
- [x] Write unit tests for username validation (empty, too long, invalid chars, leading @, valid)
- [x] `cargo clippy` clean, `cargo test` passes

### Phase 2: Thread Command (builds on profile pattern)

**Estimated effort:** ~180 lines of new code + ~15 lines of modifications to existing files.

- [x] Create `src/thread.rs` with `ThreadOpts` struct and `run_thread()` handler
  - [x] Tweet ID validation function (reusable for both user input and conversation_id)
  - [x] Step 1: Root tweet fetch with conversation_id extraction
    - [x] Validate conversation_id from API response
    - [x] Check for `errors` array (not HTTP 404)
    - [x] Age check: warn on stderr if root tweet > 7 days old
  - [x] Step 2: Conversation search with pagination loop
    - [x] Dual deduplication: `seen_ids` for tweets, `seen_user_ids` for users
    - [x] Phantom `next_token` defense (break on empty data array)
    - [x] 150ms inter-page delay
    - [x] `cache_hit` propagated to cost estimation
    - [x] Per-page cost display on stderr
  - [x] Thread tree building (`build_thread_tree()`)
    - [x] Index-based arena with `Vec<ThreadNode>` + `HashMap<String, usize>`
    - [x] BFS depth computation with circular reference guard
  - [x] Tree flattening via iterative DFS (not recursive)
  - [x] JSON output with `thread` array and `meta` object
- [x] Add `mod thread;` to `src/main.rs`
- [x] Add `Thread` variant to `Command` enum in `src/main.rs`
- [x] Add `Thread` match arm to `run()` in `src/main.rs`
- [x] Add `THREAD_ACCEPTED` and `"thread"` entry to `src/requirements.rs`
- [x] Add `"thread"` to `command_names_with_auth()` in `src/requirements.rs`
- [x] Write unit tests:
  - [x] Tweet ID validation (empty, too long, non-numeric, leading zeros OK, valid)
  - [x] Thread tree building: linear, branching, single tweet, orphaned, dedup, circular
  - [x] Flatten ordering correctness
- [x] `cargo clippy` clean, `cargo test` passes

### Phase 3: Integration + Doctor

- [ ] Verify both commands appear in `bird doctor` output with correct availability
- [ ] Verify cost tracking on stderr for both commands
- [ ] Verify cache behavior: first call is cache miss, second call within TTL is cache hit
- [ ] Verify `--refresh` forces cache bypass for both commands
- [ ] Update `docs/CLI_DESIGN.md` with new commands
- [ ] End-to-end test with real API (manual, not automated -- requires valid auth)

## Alternative Approaches Considered

### 1. Thread via Conversation Lookup endpoint (`GET /2/tweets/{id}/quote_tweets` or similar)

**Rejected.** The X API v2 does not have a dedicated "get conversation" endpoint. The only way to retrieve all tweets in a conversation is to search by `conversation_id`. The `GET /2/tweets/search/recent` endpoint with `query=conversation_id:{id}` is the documented approach (see X API docs on conversation threading).

### 2. Thread via Reverse Walk (reply chain up from any tweet)

**Considered.** Alternative approach: start from a reply tweet, walk UP the chain via `referenced_tweets[type=replied_to].id` until reaching the root (where `conversation_id == id`), then walk DOWN from root. This would avoid the search endpoint entirely but:
- Requires N sequential API calls to walk up the chain (one per ancestor)
- Cannot discover sibling branches (other replies at the same level)
- Loses the "full conversation" view that search provides

The two-step approach (fetch root + search conversation) is both faster (2 API calls minimum vs. N for chain walking) and more complete (captures the full conversation tree).

### 3. Profile via user ID instead of username

**Rejected.** `GET /2/users/{id}` requires knowing the numeric user ID, which users and agents rarely have. `GET /2/users/by/username/{username}` accepts the human-readable `@username` which is what people actually know. If a user has the numeric ID, they can already use `bird get /2/users/{id}`.

### 4. Combined profile+thread module (single file)

**Rejected.** Profile and thread have fundamentally different complexity levels. Profile is a single API call with formatting (~80 lines). Thread involves multi-step fetching, tree building, and pagination (~180 lines). Combining them would create a 260-line file that violates the 200-line refactor trigger and mixes two responsibilities. SRP dictates separate modules.

### 5. Thread format as streaming output (like bookmarks)

**Considered.** The bookmarks command (`src/bookmarks.rs`) streams each page to stdout as it arrives, wrapping in a JSON array. This works for bookmarks because ordering is preserved per-page. For threads, we need all tweets before we can build the tree and output in conversation order. Streaming individual pages would produce chronologically disordered output. The thread command must collect all pages before building and outputting the tree.

### 6. Omit `--max-pages` and fetch entire conversation

**Rejected.** Viral tweets can generate thousands of replies. Without a safety limit, `bird thread` on a popular tweet could trigger hundreds of search pages, costing $50+ in API fees. The `--max-pages 10` default (1,000 tweets maximum) provides a reasonable ceiling with explicit opt-in for larger threads via `--max-pages 25`.

### 7. Include `--fields` and `--format` flags in v1 (deferred, YAGNI)

**Deferred to v2.** No existing Bird command has `--fields` or `--format` flags. Adding them to profile and thread would be a YAGNI violation -- introducing patterns that don't yet exist in the codebase. The standard field set and JSON output cover the research use case. These can be added later as a cross-cutting enhancement to all commands without breaking changes.

## Dependencies & Risks

### Dependencies

| Dependency | Type | Status | Risk |
|---|---|---|---|
| Plan 1 (Transparent Cache Layer) | Hard | In progress | Blocks all work. CachedClient API, cost tracking, and rate limiter must be implemented first. |
| `GET /2/users/by/username/{username}` | External API | Stable | Low risk. Well-documented, stable endpoint. |
| `GET /2/tweets/{id}` | External API | Stable | Low risk. Core endpoint. |
| `GET /2/tweets/search/recent` | External API | Stable | Medium risk. Subject to rate limits (180 requests/15min for app-only, 450/15min for user context). |
| `conversation_id` support in search | External API behavior | Documented | Medium risk. The `conversation_id` query operator is documented but its behavior on very old or very large conversations is less well-documented. |
| `referenced_tweets` field | External API behavior | Stable | Low risk. Standard tweet field for reply relationships. |

### Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Search/recent 7-day window misses old threads | High | Medium | Warn user on stderr when root tweet is older than 7 days. Document limitation. Future: add `--archive` flag for full-archive search (higher tier API). |
| Conversation search returns incomplete results | Medium | Medium | Some replies may be from protected accounts or deleted. The tree builder handles missing parents gracefully (orphaned tweets shown at root level). `meta.complete` flag communicates truncation to agents. |
| Very large conversations exhaust rate limit | Medium | High | `--max-pages` default of 10 caps at 1,000 tweets. Max of 25 caps at 2,500. Plan 1's rate limiter handles 429 responses with wait-and-retry. Cost display on stderr provides real-time visibility. |
| Thread tree building produces incorrect order | Low | High | Comprehensive unit tests with various tree topologies. Iterative DFS traversal with chronological child ordering is well-understood. Circular reference guard prevents infinite loops. |
| `conversation_id` not returned in root tweet response | Low | High | Fail with clear error message. The standard tweet.fields includes `conversation_id` by default. |
| User not-found returns HTTP 200 (not 404) | High | Medium | Check `errors` array in response body. This is documented X API behavior. |
| conversation_id injection in search query | Low | High | Validate conversation_id is numeric before constructing query string. |
| Plan 1 CachedClient API changes before this plan starts | Medium | Low | This plan specifies handler signatures in terms of CachedClient. If Plan 1's API differs, adapt during implementation. The handler logic is independent of the cache wrapper's exact API. |

### Limitations to Document

1. **7-day search window:** `bird thread` only sees replies from the last 7 days. Older threads will be incomplete.
2. **No full-archive search:** The `GET /2/tweets/search/all` endpoint (full archive) requires a higher API tier. This could be a future `--archive` flag.
3. **No real-time updates:** Thread results are a point-in-time snapshot. New replies added after the fetch will not appear until the next invocation (after cache expiry or with `--refresh`).
4. **Protected accounts:** Replies from protected/private accounts will not appear in search results unless the authenticated user follows them.
5. **Root tweet not in search results:** The conversation_id search does NOT return the root tweet. It is always fetched separately in Step 1 and merged.

## Checklist: New Curated Command Pattern (from docs/solutions/)

Consolidated from all three solution documents. Apply to both profile and thread:

### Error Handling
- [ ] No silent failures: never `unwrap_or(default)` on parse operations; use `?`
- [ ] Invariants have guards: use `ok_or()` not `unwrap()` for config values
- [ ] Numeric inputs bounded: cap unbounded `u64`/`u32` to prevent overflow
- [ ] Check `errors` array in HTTP 200 responses (X API pattern for not-found)

### API Integration
- [ ] Dedicated auth constant in `requirements.rs` (e.g., `PROFILE_ACCEPTED`, `THREAD_ACCEPTED`)
- [ ] `cache_hit` propagated from response to cost estimation
- [ ] Query operator detection is token-based, not substring (if applicable)
- [ ] OAuth1 credential access via `ok_or()`, never `unwrap()`

### Pagination (thread only)
- [ ] All entity types deduplicated across pages (tweets AND users)
- [ ] Pagination URLs excluded from cache (`next_token=`)
- [ ] Empty data array breaks loop (handles phantom `next_token`)
- [ ] 150ms inter-page delay for polite pagination
- [ ] `max_pages` parameter capped at reasonable maximum (25)

### Handler Signature
- [ ] Uses `&mut CachedClient` (not `&CachedClient`)
- [ ] Includes `use_color: bool` parameter
- [ ] Uses opts struct to avoid `clippy::too_many_arguments`
- [ ] Returns `Result<(), Box<dyn std::error::Error + Send + Sync>>`

### Registration
- [ ] `mod` declaration in `main.rs` (alphabetical order)
- [ ] `Command` enum variant in `main.rs`
- [ ] Match arm in `run()` function in `main.rs`
- [ ] Auth constant and entry in `requirements_for_command()`
- [ ] Command name in `command_names_with_auth()`

### Testing
- [ ] Tests exercise actual code paths, not stdlib behavior
- [ ] Edge cases for input validation
- [ ] Tree topology tests (thread only)
- [ ] `cargo clippy` clean

## References

### Internal References

- Brainstorm: `docs/brainstorms/2026-02-11-research-commands-and-caching-brainstorm.md`
- Plan 1 (dependency): `docs/plans/2026-02-11-feat-transparent-cache-layer-plan.md`
- Search command pattern: `src/search.rs` (closest existing pattern for both commands)
- Command handler template: `src/raw.rs`
- Streaming pagination pattern: `src/bookmarks.rs`
- Auth requirements registry: `src/requirements.rs`
- Auth token resolution: `src/auth.rs`
- Config loading: `src/config.rs`
- CLI structure and Command enum: `src/main.rs`
- BirdError pattern: `src/main.rs`
- CLI design doc: `docs/CLI_DESIGN.md`
- Solution: Search command pattern: `docs/solutions/architecture-patterns/search-command-paginated-api-pattern.md`
- Solution: Cache layer: `docs/solutions/performance-issues/sqlite-cache-layer-api-cost-reduction.md`
- Solution: Security audit: `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md`

### External References

- X API Users Lookup by Username: `GET /2/users/by/username/{username}` -- returns user object with requested fields
- X API Tweet Lookup: `GET /2/tweets/{id}` -- returns tweet with conversation_id
- X API Recent Search: `GET /2/tweets/search/recent` -- supports `conversation_id:{id}` query operator
- X API conversation_id field: Identifies the root tweet of a conversation; set on all replies
- X API referenced_tweets field: Array with `type` (replied_to, quoted, retweeted) and `id` for relationship tracking
- X API search/recent rate limits: 180 requests/15min (app-only), 450 requests/15min (user context)
- X API search/recent 7-day window: Only returns tweets from the last 7 days
- X API billing: $0.005/tweet read, $0.010/user read, 24hr UTC dedup window
- X API error responses: User/tweet not-found returns HTTP 200 with `errors` array (not 404)
