//! Cost estimation and stderr display for X API billing.
//! Stateless: estimates cost per response, displays to stderr. No persistent tracking (Plan 4).

const COST_PER_TWEET_READ: f64 = 0.005;
const COST_PER_USER_READ: f64 = 0.010;

pub struct CostEstimate {
    pub tweets_read: u32,
    pub users_read: u32,
    pub estimated_usd: f64,
    pub cache_hit: bool,
}

/// Estimate cost assuming a fresh (non-cached) request.
/// Eliminates the "magic false" from D3 by giving the intent a name.
pub fn estimate_raw_cost(body: &serde_json::Value, endpoint: &str) -> CostEstimate {
    estimate_cost(body, endpoint, false)
}

/// Count objects in a JSON response body and estimate cost.
/// Pure function — no I/O.
pub fn estimate_cost(body: &serde_json::Value, endpoint: &str, cache_hit: bool) -> CostEstimate {
    if cache_hit {
        return CostEstimate {
            tweets_read: 0,
            users_read: 0,
            estimated_usd: 0.0,
            cache_hit: true,
        };
    }

    let is_user_endpoint = endpoint.contains("/users/") && !endpoint.contains("/bookmarks");

    let mut tweets: u32 = 0;
    let mut users: u32 = 0;

    // Count items in data array
    if let Some(data) = body.get("data") {
        if let Some(arr) = data.as_array() {
            let count = arr.len() as u32;
            if is_user_endpoint {
                users += count;
            } else {
                tweets += count;
            }
        } else if data.is_object() {
            // Single object response (e.g., /2/users/me)
            if is_user_endpoint {
                users += 1;
            } else {
                tweets += 1;
            }
        }
    }

    // Count included objects
    if let Some(includes) = body.get("includes") {
        if let Some(inc_users) = includes.get("users").and_then(|u| u.as_array()) {
            users += inc_users.len() as u32;
        }
        if let Some(inc_tweets) = includes.get("tweets").and_then(|t| t.as_array()) {
            tweets += inc_tweets.len() as u32;
        }
    }

    let estimated_usd = (tweets as f64 * COST_PER_TWEET_READ) + (users as f64 * COST_PER_USER_READ);

    CostEstimate {
        tweets_read: tweets,
        users_read: users,
        estimated_usd,
        cache_hit: false,
    }
}

/// Format and print cost to stderr.
pub fn display_cost(estimate: &CostEstimate, use_color: bool, quiet: bool) {
    if quiet {
        return;
    }
    let mut parts = Vec::new();
    if estimate.tweets_read > 0 {
        parts.push(format!(
            "{} tweet{}",
            estimate.tweets_read,
            if estimate.tweets_read == 1 { "" } else { "s" }
        ));
    }
    if estimate.users_read > 0 {
        parts.push(format!(
            "{} user{}",
            estimate.users_read,
            if estimate.users_read == 1 { "" } else { "s" }
        ));
    }

    let hit_miss = if estimate.cache_hit {
        "from store"
    } else {
        "cache miss"
    };

    let msg = if estimate.cache_hit {
        format!("[cost] $0.00 ({})", hit_miss)
    } else if parts.is_empty() {
        format!("[cost] $0.00 (no billable objects, {})", hit_miss)
    } else {
        format!(
            "[cost] ~${:.4} ({}, {})",
            estimate.estimated_usd,
            parts.join(", "),
            hit_miss
        )
    };

    if use_color {
        use owo_colors::OwoColorize;
        eprintln!("{}", msg.bright_black());
    } else {
        eprintln!("{}", msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_costs_zero() {
        let body = serde_json::json!({"data": [{"id": "1"}]});
        let est = estimate_cost(&body, "/2/tweets/search/recent", true);
        assert!(est.cache_hit);
        assert_eq!(est.estimated_usd, 0.0);
    }

    #[test]
    fn tweet_search_costs() {
        let body = serde_json::json!({
            "data": [{"id": "1"}, {"id": "2"}, {"id": "3"}]
        });
        let est = estimate_cost(&body, "/2/tweets/search/recent", false);
        assert_eq!(est.tweets_read, 3);
        assert_eq!(est.users_read, 0);
        assert!((est.estimated_usd - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn user_endpoint_costs() {
        let body = serde_json::json!({"data": {"id": "1", "username": "test"}});
        let est = estimate_cost(&body, "/2/users/me", false);
        assert_eq!(est.users_read, 1);
        assert_eq!(est.tweets_read, 0);
        assert!((est.estimated_usd - 0.010).abs() < f64::EPSILON);
    }

    #[test]
    fn includes_counted() {
        let body = serde_json::json!({
            "data": [{"id": "1"}],
            "includes": {
                "users": [{"id": "u1"}, {"id": "u2"}],
                "tweets": [{"id": "t1"}]
            }
        });
        let est = estimate_cost(&body, "/2/tweets/search/recent", false);
        assert_eq!(est.tweets_read, 2); // 1 data + 1 include
        assert_eq!(est.users_read, 2);
        assert!((est.estimated_usd - (0.010 + 0.020)).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_response() {
        let body = serde_json::json!({});
        let est = estimate_cost(&body, "/2/tweets/search/recent", false);
        assert_eq!(est.tweets_read, 0);
        assert_eq!(est.users_read, 0);
        assert_eq!(est.estimated_usd, 0.0);
    }

    #[test]
    fn bookmarks_endpoint_counts_tweets() {
        let body = serde_json::json!({
            "data": [{"id": "1"}, {"id": "2"}]
        });
        let est = estimate_cost(&body, "/2/users/123/bookmarks", false);
        assert_eq!(est.tweets_read, 2);
        assert_eq!(est.users_read, 0);
    }
}
