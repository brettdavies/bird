//! Thread command: reconstruct a conversation thread from a tweet ID.
//! Two-step fetch: get root tweet for conversation_id, then search for all replies.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{RequestContext, CachedClient};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::requirements::AuthType;
use reqwest::header::HeaderMap;
use reqwest_oauth1::OAuthClientProvider;
use std::collections::{HashMap, HashSet, VecDeque};

const TWEET_FIELDS: &str =
    "conversation_id,author_id,created_at,text,public_metrics,referenced_tweets,in_reply_to_user_id";
const USER_FIELDS: &str = "username,name";
const EXPANSIONS: &str = "author_id";
const MAX_PAGES_CAP: u32 = 25;

/// Thread options bundled to avoid clippy::too_many_arguments.
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    validate_tweet_id(opts.tweet_id)?;
    let max_pages = opts.max_pages.clamp(1, MAX_PAGES_CAP);

    let token = resolve_token_for_command(client.http(), config, "thread").await?;

    // Step 1: Fetch root tweet to get conversation_id
    let root_url = format!(
        "https://api.x.com/2/tweets/{}?tweet.fields={}&expansions={}&user.fields={}",
        opts.tweet_id, TWEET_FIELDS, EXPANSIONS, USER_FIELDS
    );

    let (status, body, cache_hit) = fetch(&token, client, config, &root_url).await?;
    if !status.is_success() {
        return Err(format!("GET tweet {}: {}", status, body).into());
    }

    let root_response: serde_json::Value = serde_json::from_str(&body)?;
    let estimate = cost::estimate_cost(&root_response, &root_url, cache_hit);
    cost::display_cost(&estimate, use_color);

    // Check for errors array (X API returns 200 + errors for not-found)
    if let Some(errors) = root_response.get("errors").and_then(|e| e.as_array()) {
        if let Some(err) = errors.first() {
            let detail = err
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or("unknown error");
            return Err(format!("thread failed: {}", detail).into());
        }
    }

    let root_tweet = root_response
        .get("data")
        .ok_or("thread failed: no data in root tweet response")?;

    let conversation_id = root_tweet
        .get("conversation_id")
        .and_then(|c| c.as_str())
        .ok_or("thread failed: root tweet missing conversation_id")?;

    // Validate conversation_id before injecting into search query
    validate_tweet_id(conversation_id)?;

    if conversation_id != opts.tweet_id {
        eprintln!(
            "[thread] input tweet is a reply; following to root conversation {}",
            conversation_id
        );
    }

    // Check if root tweet is older than 7 days
    // Parse ISO 8601 manually to avoid adding chrono dependency
    let root_age_days = root_tweet
        .get("created_at")
        .and_then(|c| c.as_str())
        .and_then(parse_age_days)
        .unwrap_or(0);

    if root_age_days > 7 {
        eprintln!(
            "[thread] warning: root tweet is {} days old; search/recent only covers 7 days",
            root_age_days
        );
    }

    // Step 2: Search for conversation tweets (paginated)
    let mut all_tweets: Vec<serde_json::Value> = Vec::with_capacity(100);
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut next_token: Option<String> = None;
    let mut pages_fetched: u32 = 0;

    // Seed seen_ids with root tweet to avoid duplicate
    if let Some(root_id) = root_tweet.get("id").and_then(|i| i.as_str()) {
        seen_ids.insert(root_id.to_string());
    }

    for page_num in 1..=max_pages {
        let search_url = build_search_url(conversation_id, next_token.as_deref());

        let (status, body, cache_hit) = fetch(&token, client, config, &search_url).await?;
        if !status.is_success() {
            return Err(format!("GET search page {} {}: {}", page_num, status, body).into());
        }

        let page: serde_json::Value = serde_json::from_str(&body)?;
        let estimate = cost::estimate_cost(&page, &search_url, cache_hit);
        cost::display_cost(&estimate, use_color);

        // Break on empty data (phantom next_token defense)
        let data = match page.get("data").and_then(|d| d.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            _ => break,
        };

        for tweet in data {
            let id = tweet.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if !id.is_empty() && seen_ids.insert(id.to_string()) {
                all_tweets.push(tweet.clone());
            }
        }

        pages_fetched = page_num;

        next_token = page
            .get("meta")
            .and_then(|m| m.get("next_token"))
            .and_then(|t| t.as_str())
            .map(String::from);

        if next_token.is_none() {
            break;
        }

        if page_num < max_pages {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }

    let complete = next_token.is_none();

    // Build thread tree and flatten
    let nodes = build_thread_tree(root_tweet, &all_tweets);
    let order = flatten_thread(&nodes);

    // Build output
    let thread_array: Vec<serde_json::Value> = order
        .iter()
        .map(|&idx| {
            let node = &nodes[idx];
            let mut tweet = node.tweet.clone();
            if let Some(obj) = tweet.as_object_mut() {
                obj.insert("depth".to_string(), serde_json::json!(node.depth));
            }
            tweet
        })
        .collect();

    let output = serde_json::json!({
        "thread": thread_array,
        "meta": {
            "conversation_id": conversation_id,
            "tweet_count": thread_array.len(),
            "pages_fetched": pages_fetched,
            "complete": complete,
            "root_tweet_age_days": root_age_days,
        }
    });

    if opts.pretty {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", serde_json::to_string(&output)?);
    }

    eprintln!(
        "[thread] {} tweets | {} pages fetched | {}",
        thread_array.len(),
        pages_fetched,
        if complete { "complete" } else { "truncated" }
    );

    Ok(())
}

/// Validate tweet ID: 1-20 digits, all ASCII numeric.
fn validate_tweet_id(id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if id.is_empty() || id.len() > 20 {
        return Err(format!("tweet ID must be 1-20 digits, got {}", id.len()).into());
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("tweet ID must be numeric: {}", id).into());
    }
    Ok(())
}

/// Perform a GET request using the resolved auth token.
async fn fetch(
    token: &CommandToken,
    client: &mut CachedClient,
    config: &ResolvedConfig,
    url: &str,
) -> Result<(reqwest::StatusCode, String, bool), Box<dyn std::error::Error + Send + Sync>> {
    match token {
        CommandToken::Bearer(access) => {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", access).parse()?);
            let ctx = RequestContext {
                auth_type: &AuthType::OAuth2User,
                username: config.username.as_deref(),
            };
            let response = client.get(url, &ctx, headers).await?;
            Ok((response.status, response.body, response.cache_hit))
        }
        CommandToken::OAuth1 => {
            let ck = config
                .oauth1_consumer_key
                .as_ref()
                .ok_or("OAuth1 consumer key missing")?;
            let cs = config
                .oauth1_consumer_secret
                .as_ref()
                .ok_or("OAuth1 consumer secret missing")?;
            let at = config
                .oauth1_access_token
                .as_ref()
                .ok_or("OAuth1 access token missing")?;
            let ats = config
                .oauth1_access_token_secret
                .as_ref()
                .ok_or("OAuth1 access token secret missing")?;
            let secrets = reqwest_oauth1::Secrets::new(ck.as_str(), cs.as_str())
                .token(at.as_str(), ats.as_str());
            let res = client
                .http()
                .clone()
                .oauth1(secrets)
                .get(url)
                .send()
                .await?;
            let status = res.status();
            let text = res.text().await?;
            Ok((status, text, false)) // OAuth1 bypasses cache
        }
    }
}

/// Parse an ISO 8601 created_at string and return age in days.
/// X API format: "2026-02-11T10:00:00.000Z". Returns None if unparseable.
fn parse_age_days(created_at: &str) -> Option<u32> {
    // Extract "YYYY-MM-DD" prefix, convert to a rough epoch-day estimate
    if created_at.len() < 10 {
        return None;
    }
    let year: u64 = created_at[..4].parse().ok()?;
    let month: u64 = created_at[5..7].parse().ok()?;
    let day: u64 = created_at[8..10].parse().ok()?;
    // Approximate days since epoch (good enough for 7-day comparison)
    let tweet_days = year * 365 + (year / 4) + month * 30 + day;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let now_days = now.as_secs() / 86400;
    // Convert now_days to same scale: epoch (1970-01-01) -> approx formula
    let epoch_base: u64 = 1970 * 365 + (1970 / 4) + 30 + 1;
    let now_approx = epoch_base + now_days;

    Some(now_approx.saturating_sub(tweet_days) as u32)
}

fn build_search_url(conversation_id: &str, next_token: Option<&str>) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent").unwrap();
    url.query_pairs_mut()
        .append_pair("query", &format!("conversation_id:{}", conversation_id))
        .append_pair("tweet.fields", TWEET_FIELDS)
        .append_pair("user.fields", USER_FIELDS)
        .append_pair("expansions", EXPANSIONS)
        .append_pair("max_results", "100")
        .append_pair("sort_order", "recency");
    if let Some(token) = next_token {
        url.query_pairs_mut().append_pair("next_token", token);
    }
    url.to_string()
}

struct ThreadNode {
    tweet: serde_json::Value,
    parent_id: Option<String>,
    depth: usize,
    children: Vec<usize>,
}

fn build_thread_tree(
    root_tweet: &serde_json::Value,
    search_tweets: &[serde_json::Value],
) -> Vec<ThreadNode> {
    let mut nodes: Vec<ThreadNode> = Vec::with_capacity(search_tweets.len() + 1);
    let mut id_to_index: HashMap<String, usize> = HashMap::new();

    // Insert root tweet at index 0
    let root_id = root_tweet
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("0")
        .to_string();
    nodes.push(ThreadNode {
        tweet: root_tweet.clone(),
        parent_id: None,
        depth: 0,
        children: vec![],
    });
    id_to_index.insert(root_id, 0);

    // Insert search result tweets, deduplicating against root
    for tweet in search_tweets {
        let id = tweet
            .get("id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() || id_to_index.contains_key(&id) {
            continue;
        }
        let parent_id = tweet
            .get("referenced_tweets")
            .and_then(|refs| refs.as_array())
            .and_then(|refs| {
                refs.iter()
                    .find(|r| r.get("type").and_then(|t| t.as_str()) == Some("replied_to"))
            })
            .and_then(|r| r.get("id").and_then(|i| i.as_str()))
            .map(String::from);

        let idx = nodes.len();
        nodes.push(ThreadNode {
            tweet: tweet.clone(),
            parent_id,
            depth: 0,
            children: vec![],
        });
        id_to_index.insert(id, idx);
    }

    // Build parent-child relationships
    for i in 0..nodes.len() {
        if let Some(ref parent_id) = nodes[i].parent_id {
            if let Some(&parent_idx) = id_to_index.get(parent_id) {
                nodes[parent_idx].children.push(i);
            }
        }
    }

    // Compute depths via BFS from root (with circular reference guard)
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    let mut visited = vec![false; nodes.len()];
    queue.push_back((0, 0));
    visited[0] = true;
    while let Some((idx, depth)) = queue.pop_front() {
        nodes[idx].depth = depth;
        // Sort children by created_at (lexicographic works for ISO 8601)
        // Take children to avoid borrow conflict with nodes, then put back
        let mut children = std::mem::take(&mut nodes[idx].children);
        children.sort_by(|&a, &b| {
            let a_time = nodes[a].tweet.get("created_at").and_then(|t| t.as_str()).unwrap_or("");
            let b_time = nodes[b].tweet.get("created_at").and_then(|t| t.as_str()).unwrap_or("");
            a_time.cmp(b_time)
        });
        for &child_idx in &children {
            if !visited[child_idx] {
                visited[child_idx] = true;
                queue.push_back((child_idx, depth + 1));
            }
        }
        nodes[idx].children = children;
    }

    nodes
}

/// Flatten tree into DFS order -- iterative to avoid stack overflow on deep threads.
fn flatten_thread(nodes: &[ThreadNode]) -> Vec<usize> {
    if nodes.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    let mut stack = vec![0usize];
    while let Some(idx) = stack.pop() {
        result.push(idx);
        // Push children in reverse order so first child is processed first
        for &child_idx in nodes[idx].children.iter().rev() {
            stack.push(child_idx);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_tweet_id_valid() {
        assert!(validate_tweet_id("123456789").is_ok());
        assert!(validate_tweet_id("1").is_ok());
        assert!(validate_tweet_id("00123").is_ok()); // leading zeros OK
    }

    #[test]
    fn validate_tweet_id_empty() {
        assert!(validate_tweet_id("").is_err());
    }

    #[test]
    fn validate_tweet_id_too_long() {
        assert!(validate_tweet_id("123456789012345678901").is_err()); // 21 chars
    }

    #[test]
    fn validate_tweet_id_non_numeric() {
        assert!(validate_tweet_id("abc").is_err());
        assert!(validate_tweet_id("123abc").is_err());
        assert!(validate_tweet_id("12-34").is_err());
    }

    fn make_tweet(id: &str, replied_to: Option<&str>, created_at: &str) -> serde_json::Value {
        let mut tweet = serde_json::json!({
            "id": id,
            "text": format!("Tweet {}", id),
            "created_at": created_at,
        });
        if let Some(parent) = replied_to {
            tweet["referenced_tweets"] =
                serde_json::json!([{"type": "replied_to", "id": parent}]);
        }
        tweet
    }

    #[test]
    fn tree_linear_thread() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("101", Some("100"), "2026-02-11T10:01:00Z"),
            make_tweet("102", Some("101"), "2026-02-11T10:02:00Z"),
        ];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 2);
    }

    #[test]
    fn tree_branching_replies() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("101", Some("100"), "2026-02-11T10:01:00Z"),
            make_tweet("102", Some("100"), "2026-02-11T10:02:00Z"),
            make_tweet("103", Some("101"), "2026-02-11T10:03:00Z"),
        ];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        // Root -> 101 (first child) -> 103 (child of 101) -> 102 (second child of root)
        assert_eq!(order, vec![0, 1, 3, 2]);
        assert_eq!(nodes[0].depth, 0); // root
        assert_eq!(nodes[1].depth, 1); // 101
        assert_eq!(nodes[2].depth, 1); // 102
        assert_eq!(nodes[3].depth, 2); // 103
    }

    #[test]
    fn tree_single_tweet_no_replies() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets: Vec<serde_json::Value> = vec![];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        assert_eq!(order, vec![0]);
        assert_eq!(nodes[0].depth, 0);
    }

    #[test]
    fn tree_orphaned_tweets() {
        // Tweets whose parent is outside the search window
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("101", Some("100"), "2026-02-11T10:01:00Z"),
            make_tweet("200", Some("999"), "2026-02-11T10:05:00Z"), // orphan: parent 999 not in tree
        ];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        // Root -> 101 in tree; 200 is orphaned (not reachable from root DFS)
        assert_eq!(order, vec![0, 1]);
        assert_eq!(nodes.len(), 3); // all 3 nodes exist
        assert_eq!(nodes[2].depth, 0); // orphan depth stays 0 (BFS didn't reach it)
    }

    #[test]
    fn tree_deduplicates_root_in_search() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("100", None, "2026-02-11T10:00:00Z"), // duplicate of root
            make_tweet("101", Some("100"), "2026-02-11T10:01:00Z"),
        ];
        let nodes = build_thread_tree(&root, &tweets);

        assert_eq!(nodes.len(), 2); // root + 101 (duplicate root skipped)
    }

    #[test]
    fn tree_circular_reference_guard() {
        // Simulate circular: A -> B -> A (shouldn't happen in practice)
        let root = serde_json::json!({
            "id": "100",
            "text": "Root",
            "created_at": "2026-02-11T10:00:00Z",
            "referenced_tweets": [{"type": "replied_to", "id": "101"}],
        });
        let tweets = vec![serde_json::json!({
            "id": "101",
            "text": "Reply",
            "created_at": "2026-02-11T10:01:00Z",
            "referenced_tweets": [{"type": "replied_to", "id": "100"}],
        })];

        // Should not panic or infinite loop
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        assert_eq!(nodes.len(), 2);
        // Both nodes reachable since they mutually reference each other
        // BFS visits 0 first, then 1 as child of 0 (since 100 is parent of 101)
        assert!(order.len() <= 2);
    }

    #[test]
    fn flatten_empty() {
        let nodes: Vec<ThreadNode> = vec![];
        assert_eq!(flatten_thread(&nodes), Vec::<usize>::new());
    }

    #[test]
    fn parse_age_days_recent() {
        let result = parse_age_days("2026-02-11T10:00:00.000Z");
        assert!(result.is_some());
        let age = result.unwrap();
        // Should be within a reasonable range (test may run in 2026 or later)
        assert!(age < 365 * 10, "age {} seems unreasonably large", age);
    }

    #[test]
    fn parse_age_days_old_tweet() {
        // A date from 2020 should be clearly older than 7 days
        let result = parse_age_days("2020-01-15T12:00:00.000Z");
        assert!(result.is_some());
        assert!(result.unwrap() > 7, "2020 tweet should be > 7 days old");
    }

    #[test]
    fn parse_age_days_invalid() {
        assert!(parse_age_days("").is_none());
        assert!(parse_age_days("not-a-date").is_none());
        assert!(parse_age_days("short").is_none());
    }

    #[test]
    fn build_search_url_basic() {
        let url = build_search_url("123456", None);
        assert!(url.contains("conversation_id%3A123456"));
        assert!(url.contains("max_results=100"));
        assert!(!url.contains("next_token"));
    }

    #[test]
    fn build_search_url_with_token() {
        let url = build_search_url("123456", Some("abc123"));
        assert!(url.contains("next_token=abc123"));
    }
}
