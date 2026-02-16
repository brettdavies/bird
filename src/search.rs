//! Search command: query building, pagination, filtering, sorting, JSON output.

use crate::auth::{resolve_token_for_command, CommandToken};
use crate::cache::{RequestContext, CachedClient};
use crate::config::ResolvedConfig;
use crate::cost;
use crate::output;
use crate::requirements::AuthType;
use reqwest::header::HeaderMap;
use std::collections::HashSet;

// Sensible defaults for research workflows. Extract to shared module when Plan 3 needs it.
const TWEET_FIELDS: &str = "created_at,public_metrics,author_id,conversation_id,referenced_tweets";
const USER_FIELDS: &str = "username,name";
const EXPANSIONS: &str = "author_id";

/// Search options bundled to avoid clippy::too_many_arguments.
pub struct SearchOpts<'a> {
    pub query: &'a str,
    pub pretty: bool,
    pub sort: &'a str,
    pub min_likes: Option<u64>,
    pub max_results: u32,
    pub pages: u32,
}

pub async fn run_search(
    client: &mut CachedClient,
    config: &ResolvedConfig,
    opts: SearchOpts<'_>,
    use_color: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate sort key before any API calls (fail fast)
    if !matches!(opts.sort, "recent" | "likes") {
        return Err(format!(
            "invalid --sort value \"{}\"; expected: recent, likes",
            opts.sort
        )
        .into());
    }

    let effective_query = apply_noise_reduction(opts.query);
    let token = resolve_token_for_command(client.http(), config, "search").await?;

    let mut all_tweets: Vec<serde_json::Value> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut all_users: Vec<serde_json::Value> = Vec::new();
    let mut seen_user_ids: HashSet<String> = HashSet::new();
    let mut next_token: Option<String> = None;
    let mut pages_fetched: u32 = 0;

    for page_num in 1..=opts.pages {
        let url = build_search_url(&effective_query, opts.max_results, next_token.as_deref());

        let response = match &token {
            CommandToken::Bearer(access) => {
                let mut headers = HeaderMap::new();
                headers.insert("Authorization", format!("Bearer {}", access).parse()?);
                let ctx = RequestContext {
                    auth_type: &AuthType::OAuth2User,
                    username: config.username.as_deref(),
                };
                client.get(&url, &ctx, headers).await?
            }
            CommandToken::OAuth1 => client.oauth1_request("GET", &url, config, None).await?,
        };

        if !response.status.is_success() {
            return Err(format!(
                "GET search {}: {}",
                response.status,
                output::sanitize_for_stderr(&response.body, 200)
            )
            .into());
        }

        let page = response.json.ok_or("invalid JSON from search")?;

        // Manual cost display per page
        let estimate = cost::estimate_cost(&page, &url, response.cache_hit);
        cost::display_cost(&estimate, use_color);

        // Break on empty data (handles phantom next_token)
        let data = match page.get("data").and_then(|d| d.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            _ => break,
        };

        // Per-page filtering + dedup
        let before = all_tweets.len();
        for tweet in data {
            let id = tweet.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() || seen_ids.contains(id) {
                continue;
            }
            if is_retweet(tweet) {
                continue;
            }
            if let Some(min) = opts.min_likes {
                if extract_metric(tweet, "like_count") < min {
                    continue;
                }
            }
            seen_ids.insert(id.to_string());
            all_tweets.push(tweet.clone());
        }
        let passed = all_tweets.len() - before;
        pages_fetched = page_num;

        // Collect included users (deduplicated across pages)
        if let Some(includes) = page.get("includes") {
            if let Some(users) = includes.get("users").and_then(|u| u.as_array()) {
                for user in users {
                    let uid = user.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !uid.is_empty() && seen_user_ids.insert(uid.to_string()) {
                        all_users.push(user.clone());
                    }
                }
            }
        }

        eprintln!(
            "[search] page {}/{}: {} new tweets ({} total)",
            page_num,
            opts.pages,
            passed,
            all_tweets.len()
        );

        // Extract next_token
        next_token = page
            .get("meta")
            .and_then(|m| m.get("next_token"))
            .and_then(|t| t.as_str())
            .map(String::from);

        if next_token.is_none() {
            break;
        }

        // Rate limiting between pages
        if page_num < opts.pages {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }

    // Post-fetch sorting
    sort_tweets(&mut all_tweets, opts.sort);

    // Build output JSON preserving API response shape
    let output = serde_json::json!({
        "data": all_tweets,
        "includes": { "users": all_users },
    });

    if opts.pretty {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", serde_json::to_string(&output)?);
    }

    eprintln!(
        "[search] {} results | sorted by {} | {} pages fetched",
        all_tweets.len(),
        opts.sort,
        pages_fetched
    );

    Ok(())
}

fn build_search_url(query: &str, max_results: u32, next_token: Option<&str>) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent").unwrap();
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("tweet.fields", TWEET_FIELDS)
        .append_pair("user.fields", USER_FIELDS)
        .append_pair("expansions", EXPANSIONS)
        .append_pair("max_results", &max_results.to_string());
    if let Some(token) = next_token {
        url.query_pairs_mut().append_pair("next_token", token);
    }
    url.to_string()
}

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

/// Client-side retweet filter. `-is:retweet` can leak retweets (known X API bug).
fn is_retweet(tweet: &serde_json::Value) -> bool {
    tweet
        .get("referenced_tweets")
        .and_then(|rt| rt.as_array())
        .map(|arr| {
            arr.iter()
                .any(|r| r.get("type").and_then(|t| t.as_str()) == Some("retweeted"))
        })
        .unwrap_or(false)
}

fn extract_metric(tweet: &serde_json::Value, metric_name: &str) -> u64 {
    tweet
        .get("public_metrics")
        .and_then(|m| m.get(metric_name))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}

fn sort_tweets(tweets: &mut [serde_json::Value], sort_by: &str) {
    match sort_by {
        "recent" => {} // Already in API order (reverse chronological)
        "likes" => tweets.sort_by(|a, b| {
            let a_likes = extract_metric(a, "like_count");
            let b_likes = extract_metric(b, "like_count");
            b_likes.cmp(&a_likes)
        }),
        _ => {} // Validated before API calls; unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_reduction_appends_filter() {
        assert_eq!(apply_noise_reduction("rust lang"), "rust lang -is:retweet");
    }

    #[test]
    fn noise_reduction_skips_when_present() {
        assert_eq!(apply_noise_reduction("rust is:retweet"), "rust is:retweet");
        assert_eq!(
            apply_noise_reduction("rust -is:retweet"),
            "rust -is:retweet"
        );
    }

    #[test]
    fn build_url_basic() {
        let url = build_search_url("rust lang", 100, None);
        assert!(url.starts_with("https://api.x.com/2/tweets/search/recent?"));
        assert!(url.contains("query=rust+lang"));
        assert!(url.contains("max_results=100"));
        assert!(url.contains("tweet.fields="));
        assert!(!url.contains("next_token="));
    }

    #[test]
    fn build_url_with_next_token() {
        let url = build_search_url("test", 50, Some("abc123"));
        assert!(url.contains("next_token=abc123"));
    }

    #[test]
    fn build_url_escapes_query() {
        let url = build_search_url("test&evil=true", 100, None);
        // The & should be encoded, not treated as a param separator
        assert!(url.contains("query=test%26evil%3Dtrue"));
    }

    #[test]
    fn is_retweet_detects_retweet() {
        let tweet = serde_json::json!({
            "id": "1",
            "referenced_tweets": [{"type": "retweeted", "id": "2"}]
        });
        assert!(is_retweet(&tweet));
    }

    #[test]
    fn is_retweet_passes_original() {
        let tweet = serde_json::json!({"id": "1", "text": "hello"});
        assert!(!is_retweet(&tweet));
    }

    #[test]
    fn is_retweet_passes_quote() {
        let tweet = serde_json::json!({
            "id": "1",
            "referenced_tweets": [{"type": "quoted", "id": "2"}]
        });
        assert!(!is_retweet(&tweet));
    }

    #[test]
    fn extract_metric_returns_value() {
        let tweet = serde_json::json!({
            "public_metrics": {"like_count": 42, "retweet_count": 5}
        });
        assert_eq!(extract_metric(&tweet, "like_count"), 42);
        assert_eq!(extract_metric(&tweet, "retweet_count"), 5);
    }

    #[test]
    fn extract_metric_missing_returns_zero() {
        let tweet = serde_json::json!({"id": "1"});
        assert_eq!(extract_metric(&tweet, "like_count"), 0);
    }

    #[test]
    fn sort_by_likes() {
        let mut tweets = vec![
            serde_json::json!({"id": "1", "public_metrics": {"like_count": 5}}),
            serde_json::json!({"id": "2", "public_metrics": {"like_count": 100}}),
            serde_json::json!({"id": "3", "public_metrics": {"like_count": 20}}),
        ];
        sort_tweets(&mut tweets, "likes");
        assert_eq!(tweets[0]["id"], "2");
        assert_eq!(tweets[1]["id"], "3");
        assert_eq!(tweets[2]["id"], "1");
    }

    #[test]
    fn sort_by_recent_is_noop() {
        let mut tweets = vec![
            serde_json::json!({"id": "3"}),
            serde_json::json!({"id": "1"}),
            serde_json::json!({"id": "2"}),
        ];
        let original = tweets.clone();
        sort_tweets(&mut tweets, "recent");
        assert_eq!(tweets, original);
    }

    #[test]
    fn noise_reduction_ignores_substrings() {
        // "crisis:retweet" contains "is:retweet" as a substring but is NOT the operator
        assert_eq!(
            apply_noise_reduction("crisis:retweet analysis"),
            "crisis:retweet analysis -is:retweet"
        );
    }
}
