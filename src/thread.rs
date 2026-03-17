//! Thread command: reconstruct a conversation thread from a tweet ID.
//! Two-step fetch: get root tweet for conversation_id, then search for all replies.

use crate::cost;
use crate::db::{BirdClient, RequestContext};
use crate::diag;
use crate::fields;
use crate::output;
use crate::requirements::AuthType;
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_PAGES_CAP: u32 = 25;

/// Thread options bundled to avoid clippy::too_many_arguments.
pub struct ThreadOpts<'a> {
    pub tweet_id: &'a str,
    pub pretty: bool,
    pub max_pages: u32,
}

pub fn run_thread(
    client: &mut BirdClient,
    opts: ThreadOpts<'_>,
    use_color: bool,
    quiet: bool,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    validate_tweet_id(opts.tweet_id)?;
    let max_pages = opts.max_pages.clamp(1, MAX_PAGES_CAP);

    let ctx = RequestContext {
        auth_type,
        username: None,
    };

    // Step 1: Fetch root tweet to get conversation_id
    let root_url = {
        let mut url =
            url::Url::parse(&format!("https://api.x.com/2/tweets/{}", opts.tweet_id)).unwrap();
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in fields::tweet_query_params() {
                pairs.append_pair(key, value);
            }
        }
        url.to_string()
    };

    let response = client.get(&root_url, &ctx)?;
    if !response.is_success() {
        return Err(format!(
            "GET tweet {}: {}",
            response.status,
            output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }

    let root_response = response.json.ok_or("invalid JSON from tweet lookup")?;
    let estimate = cost::estimate_cost(&root_response, &root_url, response.cache_hit);
    cost::display_cost(&estimate, use_color, quiet);

    // Check for errors array (X API returns 200 + errors for not-found)
    if let Some(errors) = root_response.get("errors").and_then(|e| e.as_array())
        && let Some(err) = errors.first()
    {
        let detail = err
            .get("detail")
            .and_then(|d| d.as_str())
            .unwrap_or("unknown error");
        return Err(format!("thread failed: {}", detail).into());
    }

    let root_tweet = root_response
        .get("data")
        .ok_or("thread failed: no data in root tweet response")?;

    let conversation_id = root_tweet
        .get("conversation_id")
        .and_then(|c| c.as_str())
        .ok_or("thread failed: root tweet missing conversation_id")?;

    validate_tweet_id(conversation_id)?;

    if conversation_id != opts.tweet_id {
        diag!(
            quiet,
            "[thread] input tweet is a reply; following to root conversation {}",
            conversation_id
        );
    }

    let root_age_days = root_tweet
        .get("created_at")
        .and_then(|c| c.as_str())
        .and_then(parse_age_days)
        .unwrap_or(0);

    if root_age_days > 7 {
        diag!(
            quiet,
            "[thread] warning: root tweet is {} days old; search/recent only covers 7 days",
            root_age_days
        );
    }

    // Step 2: Search for conversation tweets (paginated)
    let mut all_tweets: Vec<serde_json::Value> = Vec::with_capacity(100);
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut next_token: Option<String> = None;
    let mut pages_fetched: u32 = 0;

    if let Some(root_id) = root_tweet.get("id").and_then(|i| i.as_str()) {
        seen_ids.insert(root_id.to_string());
    }

    for page_num in 1..=max_pages {
        let search_url = build_search_url(conversation_id, next_token.as_deref());

        let response = client.get(&search_url, &ctx)?;
        if !response.is_success() {
            return Err(format!(
                "GET search page {} {}: {}",
                page_num,
                response.status,
                output::sanitize_for_stderr(&response.body, 200)
            )
            .into());
        }

        let page = response.json.ok_or("invalid JSON from search")?;
        let estimate = cost::estimate_cost(&page, &search_url, response.cache_hit);
        cost::display_cost(&estimate, use_color, quiet);

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
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }

    let complete = next_token.is_none();

    let nodes = build_thread_tree(root_tweet, &all_tweets);
    let order = flatten_thread(&nodes);

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

    diag!(
        quiet,
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

/// Parse an ISO 8601 created_at string and return age in days.
fn parse_age_days(created_at: &str) -> Option<u32> {
    if created_at.len() < 10 {
        return None;
    }
    let year: u64 = created_at[..4].parse().ok()?;
    let month: u64 = created_at[5..7].parse().ok()?;
    let day: u64 = created_at[8..10].parse().ok()?;
    let tweet_days = year * 365 + (year / 4) + month * 30 + day;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let now_days = now.as_secs() / 86400;
    let epoch_base: u64 = 1970 * 365 + (1970 / 4) + 30 + 1;
    let now_approx = epoch_base + now_days;

    Some(now_approx.saturating_sub(tweet_days) as u32)
}

fn build_search_url(conversation_id: &str, next_token: Option<&str>) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent").unwrap();
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("query", &format!("conversation_id:{}", conversation_id));
        for (key, value) in fields::tweet_query_params() {
            pairs.append_pair(key, value);
        }
        pairs.append_pair("max_results", "100");
        pairs.append_pair("sort_order", "recency");
    }
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

    for i in 0..nodes.len() {
        if let Some(ref parent_id) = nodes[i].parent_id
            && let Some(&parent_idx) = id_to_index.get(parent_id)
        {
            nodes[parent_idx].children.push(i);
        }
    }

    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    let mut visited = vec![false; nodes.len()];
    queue.push_back((0, 0));
    visited[0] = true;
    while let Some((idx, depth)) = queue.pop_front() {
        nodes[idx].depth = depth;
        let mut children = std::mem::take(&mut nodes[idx].children);
        children.sort_by(|&a, &b| {
            let a_time = nodes[a]
                .tweet
                .get("created_at")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let b_time = nodes[b]
                .tweet
                .get("created_at")
                .and_then(|t| t.as_str())
                .unwrap_or("");
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

fn flatten_thread(nodes: &[ThreadNode]) -> Vec<usize> {
    if nodes.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    let mut stack = vec![0usize];
    while let Some(idx) = stack.pop() {
        result.push(idx);
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
        assert!(validate_tweet_id("00123").is_ok());
    }

    #[test]
    fn validate_tweet_id_empty() {
        assert!(validate_tweet_id("").is_err());
    }

    #[test]
    fn validate_tweet_id_too_long() {
        assert!(validate_tweet_id("123456789012345678901").is_err());
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
            tweet["referenced_tweets"] = serde_json::json!([{"type": "replied_to", "id": parent}]);
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
        ];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        assert_eq!(order.len(), 3);
        assert_eq!(order[0], 0); // root first
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 1);
    }

    #[test]
    fn tree_single_tweet_no_replies() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let nodes = build_thread_tree(&root, &[]);
        let order = flatten_thread(&nodes);

        assert_eq!(order, vec![0]);
        assert_eq!(nodes[0].depth, 0);
    }

    #[test]
    fn tree_orphaned_tweets() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![make_tweet("200", Some("999"), "2026-02-11T10:01:00Z")];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);

        assert_eq!(order, vec![0]); // orphan not reachable from root
        assert_eq!(nodes.len(), 2); // still stored
    }

    #[test]
    fn tree_deduplicates_root_in_search() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("100", None, "2026-02-11T10:00:00Z"), // duplicate of root
            make_tweet("101", Some("100"), "2026-02-11T10:01:00Z"),
        ];
        let nodes = build_thread_tree(&root, &tweets);
        assert_eq!(nodes.len(), 2); // root + 101, no duplicate
    }

    #[test]
    fn tree_circular_reference_guard() {
        let root = make_tweet("100", None, "2026-02-11T10:00:00Z");
        let tweets = vec![
            make_tweet("101", Some("102"), "2026-02-11T10:01:00Z"),
            make_tweet("102", Some("101"), "2026-02-11T10:02:00Z"),
        ];
        let nodes = build_thread_tree(&root, &tweets);
        let order = flatten_thread(&nodes);
        // Should not infinite loop — orphaned cycle won't be visited
        assert_eq!(order, vec![0]);
    }

    #[test]
    fn flatten_empty() {
        assert_eq!(flatten_thread(&[]), Vec::<usize>::new());
    }

    #[test]
    fn build_search_url_basic() {
        let url = build_search_url("123", None);
        assert!(url.contains("conversation_id%3A123"));
        assert!(!url.contains("next_token"));
    }

    #[test]
    fn build_search_url_with_token() {
        let url = build_search_url("123", Some("abc"));
        assert!(url.contains("next_token=abc"));
    }

    #[test]
    fn parse_age_days_recent() {
        // A tweet from "today-ish" should have age 0-2
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let secs = now.as_secs();
        let days = secs / 86400;
        // Rough reverse: epoch days to date (imprecise but good enough for test)
        let year = 1970 + days / 365;
        let remaining = days % 365;
        let month = 1 + remaining / 30;
        let day = 1 + remaining % 30;
        let ts = format!("{:04}-{:02}-{:02}T00:00:00Z", year, month, day);
        let age = parse_age_days(&ts);
        assert!(age.is_some());
        assert!(
            age.unwrap() <= 2,
            "recent tweet should be ~0 days old, got {}",
            age.unwrap()
        );
    }

    #[test]
    fn parse_age_days_old_tweet() {
        let age = parse_age_days("2020-01-01T00:00:00Z");
        assert!(age.is_some());
        assert!(age.unwrap() > 365);
    }

    #[test]
    fn parse_age_days_invalid() {
        assert!(parse_age_days("short").is_none());
        assert!(parse_age_days("").is_none());
    }
}
