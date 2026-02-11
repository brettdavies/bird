---
title: "Thread & Profile Commands: Tree Reconstruction and Single-Endpoint Patterns"
category: architecture-patterns
tags: [thread, profile, tree, bfs, dfs, pagination, conversation_id, validation, arena]
module: src/thread.rs, src/profile.rs
symptom: "Need to reconstruct conversation threads and look up user profiles via X API"
root_cause: "X API has no single endpoint for full thread data; requires two-step fetch (root tweet + conversation_id search) and client-side tree assembly"
severity: feature
date_solved: "2026-02-11"
related_prs: [4]
---

# Thread & Profile Commands: Tree Reconstruction and Single-Endpoint Patterns

## Problem

Bird CLI needed two new curated commands:
1. **`bird profile <username>`** -- Look up a user by username (single API call)
2. **`bird thread <tweet_id>`** -- Reconstruct a full conversation thread (multi-step, paginated)

The thread command is architecturally interesting because the X API has no single "get thread" endpoint. Threads must be reconstructed from two separate API calls, then assembled into a tree structure client-side.

## Solution

### Profile: Single-Endpoint Pattern

Profile demonstrates the minimal curated command template:

1. Validate input (`validate_username` strips `@`, checks `[a-zA-Z0-9_]{1,15}`)
2. Resolve auth token
3. Single API call to `GET /2/users/by/username/{username}`
4. Handle X API's errors-in-200 response pattern
5. Display cost, output JSON

```rust
// X API returns HTTP 200 + errors array for not-found users (NOT 404)
if let Some(errors) = json.get("errors").and_then(|e| e.as_array()) {
    if let Some(err) = errors.first() {
        let detail = err.get("detail").and_then(|d| d.as_str()).unwrap_or("unknown error");
        return Err(format!("profile failed: {}", detail).into());
    }
}
```

### Thread: Two-Step Fetch + Tree Reconstruction

#### Step 1: Fetch root tweet for `conversation_id`

The input tweet may be a reply, not the root. Fetch it first to discover the `conversation_id`:

```rust
let conversation_id = root_tweet.get("conversation_id")
    .and_then(|c| c.as_str())
    .ok_or("thread failed: root tweet missing conversation_id")?;

// Validate before injecting into search query (defense-in-depth)
validate_tweet_id(conversation_id)?;
```

#### Step 2: Paginated search for `conversation_id:{id}`

Search `GET /2/tweets/search/recent?query=conversation_id:{id}` with pagination up to `MAX_PAGES_CAP=25` pages of 100 tweets.

Key defenses:
- Root tweet seeded in `seen_ids` to prevent duplication
- Empty data array breaks loop (phantom `next_token` defense)
- 150ms inter-page delay for rate limiting
- `--max-pages` clamped to `1..=25` inside the handler

#### Step 3: Index-based arena tree

Build a parent-child tree using an arena pattern (flat `Vec<ThreadNode>` + `HashMap<String, usize>`):

```rust
struct ThreadNode {
    tweet: serde_json::Value,
    parent_id: Option<String>,
    depth: usize,
    children: Vec<usize>,
}
```

Why arena over `Box<Node>`:
- No recursive types, no `Box` indirection
- Children are indices into the same `Vec`
- Easy to iterate, sort, and serialize
- No lifetime complexity

#### Step 4: BFS depth computation with circular reference guard

```rust
let mut visited = vec![false; nodes.len()];
// ... BFS assigns depth, sorts children by created_at
// std::mem::take avoids clone on children during sort
let mut children = std::mem::take(&mut nodes[idx].children);
children.sort_by(|&a, &b| { /* lexicographic created_at */ });
nodes[idx].children = children;
```

#### Step 5: Iterative DFS flattening

```rust
fn flatten_thread(nodes: &[ThreadNode]) -> Vec<usize> {
    let mut stack = vec![0usize];
    // Iterative, not recursive -- prevents stack overflow on deep threads
    while let Some(idx) = stack.pop() {
        result.push(idx);
        for &child_idx in nodes[idx].children.iter().rev() {
            stack.push(child_idx);
        }
    }
}
```

#### Step 6: Output with depth annotations

Each tweet in the output gets a `depth` field injected for consumer convenience:

```json
{
  "thread": [{"id":"100","depth":0,...}, {"id":"101","depth":1,...}],
  "meta": {
    "conversation_id": "100",
    "tweet_count": 3,
    "complete": true,
    "root_tweet_age_days": 0
  }
}
```

### 7-Day Age Warning

`search/recent` only covers 7 days. A lightweight `parse_age_days` function (no chrono dependency) compares the root tweet's `created_at` to warn when threads are likely incomplete:

```rust
if root_age_days > 7 {
    eprintln!("[thread] warning: root tweet is {} days old; search/recent only covers 7 days", root_age_days);
}
```

### Extracted `fetch()` Helper

Thread needs the same Bearer/OAuth1 dispatch in two places (root tweet + each search page). Extracting `fetch()` as a module-private helper avoids inline duplication:

```rust
async fn fetch(token: &CommandToken, client: &mut CachedClient, config: &ResolvedConfig, url: &str)
    -> Result<(StatusCode, String, bool), ...>
```

## Review Findings and Fixes

Key learnings from 5-agent code review (Architecture, Security, Performance, Pattern, Simplicity):

| Finding | Severity | Fix |
|---------|----------|-----|
| `all_users`/`collect_users` collected but never emitted | P1 | Removed dead code (~22 LOC) |
| `ThreadNode.tweet_id` dead field with `#[allow(dead_code)]` | P2 | Removed field and population sites |
| BFS children double-cloned | P2 | `std::mem::take` + move-back pattern |
| `HashSet<usize>` for BFS visited | P3 | Replaced with `Vec<bool>` (dense indices) |
| `parse_age_days` untested | P3 | Added 3 unit tests |
| Vectors not pre-sized | P3 | Added `Vec::with_capacity` |
| Auth dispatch duplicated across modules | Defer | Tech debt; `fetch()` is local progress |
| Cache auth type mislabeling (Bearer→OAuth2User) | Defer | Pre-existing pattern across codebase |

## Prevention: Additions to Curated Command Checklist

### Tree/Graph Data Structures
- [ ] Use index-based arena (`Vec<Node>` + `HashMap<Id, usize>`) over `Box<Node>` trees
- [ ] BFS/DFS must have cycle guard (visited set) even when cycles "can't happen"
- [ ] Use iterative traversal, not recursive, when depth is unbounded
- [ ] Use `std::mem::take` to avoid clone when sorting children in-place

### Multi-Step API Commands
- [ ] Validate API-returned IDs before injecting into subsequent queries
- [ ] Extract shared fetch helper when same auth dispatch runs in a loop
- [ ] Warn users about API coverage limits (e.g., 7-day search window)

### Dead Code Prevention
- [ ] Never use `#[allow(dead_code)]` on struct fields -- remove the field instead
- [ ] If collecting data across pages, ensure it appears in the output JSON
- [ ] If a function exists solely to serve dead code, remove both

## Common Pitfalls with Thread Reconstruction

| Pitfall | Impact | Prevention |
|---------|--------|------------|
| Not fetching root tweet separately | conversation_id unknown for replies | Two-step fetch: root first, then search |
| Root tweet duplicated in search results | Double-counted in output | Seed `seen_ids` with root ID before pagination |
| Recursive DFS on deep threads | Stack overflow | Iterative DFS with explicit stack |
| No circular reference guard in BFS | Infinite loop on malformed data | `visited` set/vec in BFS |
| Children clone for borrow-checker workaround | Unnecessary allocation | `std::mem::take` + move-back |
| Collecting data that's never emitted | Dead code, wasted memory | Verify all collected data appears in output |
| `conversation_id` used unvalidated in query | Potential injection | Validate with same rules as tweet_id |

## Related Documentation

- [Search Command Pattern](./search-command-paginated-api-pattern.md) -- Pagination, dedup, cost estimation
- [CLI Design](../../CLI_DESIGN.md) -- Auth requirements, doctor command
- PR #4: feat(profile,thread) -- https://github.com/brettdavies/bird/pull/4

## Files Changed

| File | Change |
|------|--------|
| `src/profile.rs` | New: profile handler, username validation, 5 unit tests |
| `src/thread.rs` | New: thread handler, tree construction, DFS flattening, 16 unit tests |
| `src/main.rs` | Added `Command::Profile` and `Command::Thread` variants and dispatch |
| `src/requirements.rs` | Added `PROFILE_ACCEPTED`, `THREAD_ACCEPTED` constants and registry entries |
| `docs/CLI_DESIGN.md` | Added profile and thread to command list and auth table |
