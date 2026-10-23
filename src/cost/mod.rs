//! Cost estimation and stderr display for X API billing: a read is priced
//! per resource in its response, a write per request from the request
//! itself, at the rates in `rates`.

mod rates;

pub use rates::WriteRate;
use rates::{OWNED_READ_SUFFIXES, OWNED_READ_USD, POST_READ_USD, USER_READ_USD};

/// `GET /2/users/{id}/<suffix>` endpoints whose `data` holds posts, not users.
const USER_POST_SUFFIXES: &[&str] = &[
    "tweets",
    "mentions",
    "liked_tweets",
    "bookmarks",
    "timelines",
];

/// `POST /2/users/{id}/<suffix>` creates the interaction and
/// `DELETE /2/users/{id}/<suffix>/{target}` removes it.
const USER_INTERACTION_SUFFIXES: &[&str] =
    &["likes", "retweets", "following", "muting", "blocking"];

/// A request as bird sent it. A write is priced from the request, a read
/// from its response.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    /// Full URL or bare path; any query string is ignored.
    pub url: &'a str,
    /// Request body; a post's text decides its write rate.
    pub body: Option<&'a str>,
    /// The authenticated user's id. Owned-read pricing applies only when the
    /// path's `{id}` is this id.
    pub caller_id: Option<&'a str>,
}

impl<'a> Request<'a> {
    pub fn get(url: &'a str) -> Self {
        Self {
            method: "GET",
            url,
            body: None,
            caller_id: None,
        }
    }
}

pub struct CostEstimate {
    pub posts_read: u32,
    pub users_read: u32,
    /// The read is billed at the owned-read rate.
    pub owned_read: bool,
    /// The write row a non-GET request is billed under, when bird knows it.
    pub write: Option<WriteRate>,
    pub estimated_usd: f64,
    pub cache_hit: bool,
}

/// Estimate cost assuming a fresh (non-cached) request.
/// Eliminates the "magic false" from D3 by giving the intent a name.
pub fn estimate_raw_cost(request: &Request<'_>, response: &serde_json::Value) -> CostEstimate {
    estimate_cost(request, response, false)
}

/// Price a request: a write from its method, path, and body; a read from
/// the resources in its response. A cache hit costs nothing. Pure function.
pub fn estimate_cost(
    request: &Request<'_>,
    response: &serde_json::Value,
    cache_hit: bool,
) -> CostEstimate {
    let mut estimate = CostEstimate {
        posts_read: 0,
        users_read: 0,
        owned_read: false,
        write: None,
        estimated_usd: 0.0,
        cache_hit,
    };
    if cache_hit {
        return estimate;
    }

    let path = path_of(request.url);
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if !request.method.eq_ignore_ascii_case("GET") {
        estimate.write = classify_write(request.method, &segments, request.body);
        estimate.estimated_usd = estimate.write.map_or(0.0, WriteRate::usd);
        return estimate;
    }

    let (posts, users) = count_resources(&segments, response);
    estimate.posts_read = posts;
    estimate.users_read = users;
    estimate.owned_read = is_owned_read(&segments, request.caller_id);
    // The page prices an Owned Read endpoint per resource with no split by
    // type, so its includes bill at the owned rate too.
    estimate.estimated_usd = if estimate.owned_read {
        f64::from(posts + users) * OWNED_READ_USD
    } else {
        f64::from(posts) * POST_READ_USD + f64::from(users) * USER_READ_USD
    };
    estimate
}

fn path_of(url: &str) -> String {
    url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| {
            url.split_once('?')
                .map_or(url, |(path, _)| path)
                .to_string()
        })
}

fn is_owned_read(segments: &[&str], caller_id: Option<&str>) -> bool {
    matches!(
        segments,
        ["2", "users", id, suffix]
            if OWNED_READ_SUFFIXES.contains(suffix) && caller_id == Some(*id)
    )
}

fn data_holds_users(segments: &[&str]) -> bool {
    match segments {
        ["2", "users", _, suffix, ..] => !USER_POST_SUFFIXES.contains(suffix),
        ["2", "users", ..] => true,
        _ => false,
    }
}

/// Count the posts and users a read response returns, `data` and `includes`.
fn count_resources(segments: &[&str], response: &serde_json::Value) -> (u32, u32) {
    let mut posts: u32 = 0;
    let mut users: u32 = 0;
    let data_count = match response.get("data") {
        Some(serde_json::Value::Array(items)) => items.len() as u32,
        Some(serde_json::Value::Object(_)) => 1,
        _ => 0,
    };
    if data_holds_users(segments) {
        users += data_count;
    } else {
        posts += data_count;
    }
    if let Some(includes) = response.get("includes") {
        if let Some(inc_users) = includes.get("users").and_then(|u| u.as_array()) {
            users += inc_users.len() as u32;
        }
        if let Some(inc_posts) = includes.get("posts").and_then(|t| t.as_array()) {
            posts += inc_posts.len() as u32;
        }
    }
    (posts, users)
}

fn classify_write(method: &str, segments: &[&str], body: Option<&str>) -> Option<WriteRate> {
    let method = method.to_ascii_uppercase();
    match (method.as_str(), segments) {
        ("POST", ["2", "tweets"]) => Some(classify_post_create(body)),
        ("POST", ["2", "dm_conversations", ..]) => Some(WriteRate::DmInteractionCreate),
        ("POST", ["2", "users", _, suffix]) if USER_INTERACTION_SUFFIXES.contains(suffix) => {
            Some(WriteRate::UserInteractionCreate)
        }
        ("DELETE", ["2", "users", _, "muting", _]) => Some(WriteRate::MuteDelete),
        ("DELETE", ["2", "users", _, suffix, _]) if USER_INTERACTION_SUFFIXES.contains(suffix) => {
            Some(WriteRate::InteractionDelete)
        }
        _ => None,
    }
}

fn classify_post_create(body: Option<&str>) -> WriteRate {
    let body: serde_json::Value = body
        .and_then(|b| serde_json::from_str(b).ok())
        .unwrap_or_default();
    let text = body.get("text").and_then(|t| t.as_str()).unwrap_or("");
    // The page does not say which rate a summoned reply carrying a URL pays;
    // the URL rate is the higher, so the estimate does not undercount.
    if text_has_url(text) {
        WriteRate::PostCreateWithUrl
    } else if body.get("reply").is_some() {
        WriteRate::PostCreateSummoned
    } else {
        WriteRate::PostCreate
    }
}

fn text_has_url(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("http://") || text.contains("https://") || text.contains("www.")
}

/// Format and print cost to `stderr`. Honors `cfg.suppress_diag()` per KTD-1:
/// when suppressed the function returns before any formatting work, preserving
/// the zero-allocation property. Write failures are dropped (fire-and-forget):
/// stderr write errors must not promote into the API error path.
pub fn display_cost(
    cfg: &crate::output::OutputConfig,
    stderr: &mut dyn std::io::Write,
    estimate: &CostEstimate,
) {
    let quiet = cfg.suppress_diag();
    if quiet {
        return;
    }
    let use_color = cfg.use_color;
    let mut parts = Vec::new();
    if estimate.posts_read > 0 {
        parts.push(format!(
            "{} post{}",
            estimate.posts_read,
            if estimate.posts_read == 1 { "" } else { "s" }
        ));
    }
    if estimate.users_read > 0 {
        parts.push(format!(
            "{} user{}",
            estimate.users_read,
            if estimate.users_read == 1 { "" } else { "s" }
        ));
    }
    if estimate.owned_read && !parts.is_empty() {
        parts.push("owned read".to_string());
    }

    let msg = if estimate.cache_hit {
        "[cost] $0.00 (from store)".to_string()
    } else if let Some(write) = estimate.write {
        format!("[cost] ~${:.4} ({})", estimate.estimated_usd, write.label())
    } else if parts.is_empty() {
        "[cost] $0.00 (no billable objects, cache miss)".to_string()
    } else {
        format!(
            "[cost] ~${:.4} ({}, cache miss)",
            estimate.estimated_usd,
            parts.join(", ")
        )
    };

    if use_color {
        use owo_colors::OwoColorize;
        writeln!(stderr, "{}", msg.bright_black()).ok();
    } else {
        writeln!(stderr, "{}", msg).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_costs_zero() {
        let body = serde_json::json!({"data": [{"id": "1"}]});
        let est = estimate_cost(&Request::get("/2/tweets/search/recent"), &body, true);
        assert!(est.cache_hit);
        assert_eq!(est.estimated_usd, 0.0);
    }

    #[test]
    fn post_search_costs() {
        let body = serde_json::json!({
            "data": [{"id": "1"}, {"id": "2"}, {"id": "3"}]
        });
        let est = estimate_cost(&Request::get("/2/tweets/search/recent"), &body, false);
        assert_eq!(est.posts_read, 3);
        assert_eq!(est.users_read, 0);
        assert!((est.estimated_usd - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn user_endpoint_costs() {
        let body = serde_json::json!({"data": {"id": "1", "username": "test"}});
        let est = estimate_cost(&Request::get("/2/users/me"), &body, false);
        assert_eq!(est.users_read, 1);
        assert_eq!(est.posts_read, 0);
        assert!((est.estimated_usd - 0.010).abs() < f64::EPSILON);
    }

    #[test]
    fn includes_counted() {
        let body = serde_json::json!({
            "data": [{"id": "1"}],
            "includes": {
                "users": [{"id": "u1"}, {"id": "u2"}],
                "posts": [{"id": "t1"}]
            }
        });
        let est = estimate_cost(&Request::get("/2/tweets/search/recent"), &body, false);
        assert_eq!(est.posts_read, 2); // 1 data + 1 include
        assert_eq!(est.users_read, 2);
        assert!((est.estimated_usd - (0.010 + 0.020)).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_response() {
        let body = serde_json::json!({});
        let est = estimate_cost(&Request::get("/2/tweets/search/recent"), &body, false);
        assert_eq!(est.posts_read, 0);
        assert_eq!(est.users_read, 0);
        assert_eq!(est.estimated_usd, 0.0);
    }

    fn posts(n: usize) -> serde_json::Value {
        let data: Vec<_> = (0..n)
            .map(|i| serde_json::json!({"id": i.to_string()}))
            .collect();
        serde_json::json!({ "data": data })
    }

    fn assert_usd(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected ${expected}, got ${actual}"
        );
    }

    #[test]
    fn search_page_of_100_posts_costs_fifty_cents() {
        let url = "https://api.x.com/2/tweets/search/recent?query=rust&max_results=100";
        let est = estimate_cost(&Request::get(url), &posts(100), false);
        assert_eq!(est.posts_read, 100);
        assert_usd(est.estimated_usd, 0.50);
    }

    #[test]
    fn owned_read_page_of_10_posts_costs_one_cent() {
        let url = "https://api.x.com/2/users/42/bookmarks?max_results=10";
        let request = Request {
            caller_id: Some("42"),
            ..Request::get(url)
        };
        let est = estimate_cost(&request, &posts(10), false);
        assert_eq!(est.posts_read, 10);
        assert_usd(est.estimated_usd, 0.01);
    }

    #[test]
    fn user_timeline_counts_posts_not_users() {
        let est = estimate_cost(
            &Request::get("https://api.x.com/2/users/7/tweets"),
            &posts(5),
            false,
        );
        assert_eq!(est.posts_read, 5);
        assert_eq!(est.users_read, 0);
        assert_usd(est.estimated_usd, 0.025);
    }

    #[test]
    fn owned_read_endpoint_for_another_user_costs_post_reads() {
        let request = Request {
            caller_id: Some("42"),
            ..Request::get("https://api.x.com/2/users/7/tweets")
        };
        let est = estimate_cost(&request, &posts(10), false);
        assert_usd(est.estimated_usd, 0.05);
    }

    fn write(
        method: &'static str,
        url: &'static str,
        body: Option<&'static str>,
    ) -> Request<'static> {
        Request {
            method,
            url,
            body,
            caller_id: None,
        }
    }

    /// What `POST /2/tweets` returns: the created post, which X does not
    /// bill as a read.
    fn created_post() -> serde_json::Value {
        serde_json::json!({"data": {"id": "1", "text": "hello"}})
    }

    #[test]
    fn post_create_costs_one_and_a_half_cents() {
        let request = write(
            "POST",
            "https://api.x.com/2/tweets",
            Some(r#"{"text":"hello"}"#),
        );
        let est = estimate_cost(&request, &created_post(), false);
        assert_usd(est.estimated_usd, 0.015);
    }

    #[test]
    fn post_with_url_costs_twenty_cents() {
        let request = write(
            "POST",
            "https://api.x.com/2/tweets",
            Some(r#"{"text":"read this https://example.com/a"}"#),
        );
        let est = estimate_cost(&request, &created_post(), false);
        assert_usd(est.estimated_usd, 0.20);
    }

    #[test]
    fn summoned_reply_costs_one_cent() {
        let request = write(
            "POST",
            "https://api.x.com/2/tweets",
            Some(r#"{"text":"thanks","reply":{"in_reply_to_tweet_id":"9"}}"#),
        );
        let est = estimate_cost(&request, &created_post(), false);
        assert_usd(est.estimated_usd, 0.010);
    }

    #[test]
    fn reply_with_url_costs_the_url_rate() {
        let request = write(
            "POST",
            "https://api.x.com/2/tweets",
            Some(r#"{"text":"see www.example.com","reply":{"in_reply_to_tweet_id":"9"}}"#),
        );
        let est = estimate_cost(&request, &created_post(), false);
        assert_usd(est.estimated_usd, 0.20);
    }

    #[test]
    fn interaction_writes_cost_their_rows() {
        let cases = [
            ("POST", "https://api.x.com/2/users/42/likes", 0.015),
            ("POST", "https://api.x.com/2/users/42/retweets", 0.015),
            ("POST", "https://api.x.com/2/users/42/following", 0.015),
            ("POST", "https://api.x.com/2/users/42/muting", 0.015),
            ("POST", "https://api.x.com/2/users/42/blocking", 0.015),
            ("DELETE", "https://api.x.com/2/users/42/likes/9", 0.010),
            ("DELETE", "https://api.x.com/2/users/42/retweets/9", 0.010),
            ("DELETE", "https://api.x.com/2/users/42/following/7", 0.010),
            ("DELETE", "https://api.x.com/2/users/42/blocking/7", 0.010),
            ("DELETE", "https://api.x.com/2/users/42/muting/7", 0.005),
            (
                "POST",
                "https://api.x.com/2/dm_conversations/with/7/messages",
                0.015,
            ),
        ];
        let response = serde_json::json!({"data": {"ok": true}});
        for (method, url, usd) in cases {
            let est = estimate_cost(&write(method, url, Some("{}")), &response, false);
            assert!(
                (est.estimated_usd - usd).abs() < 1e-9,
                "{method} {url}: expected ${usd}, got ${}",
                est.estimated_usd
            );
        }
    }

    #[test]
    fn write_cost_line_names_the_price_row() {
        let cfg = crate::output::OutputConfig {
            format: crate::output::OutputFormat::Text,
            use_color: false,
            quiet: false,
            raw: false,
        };
        let request = write(
            "POST",
            "https://api.x.com/2/tweets",
            Some(r#"{"text":"hello"}"#),
        );
        let est = estimate_cost(&request, &created_post(), false);
        let mut stderr = Vec::new();
        display_cost(&cfg, &mut stderr, &est);
        assert_eq!(
            String::from_utf8(stderr).expect("utf8"),
            "[cost] ~$0.0150 (Post: Create)\n"
        );
    }

    #[test]
    fn owned_read_cost_line_says_so() {
        let cfg = crate::output::OutputConfig {
            format: crate::output::OutputFormat::Text,
            use_color: false,
            quiet: false,
            raw: false,
        };
        let request = Request {
            caller_id: Some("42"),
            ..Request::get("https://api.x.com/2/users/42/bookmarks")
        };
        let est = estimate_cost(&request, &posts(10), false);
        let mut stderr = Vec::new();
        display_cost(&cfg, &mut stderr, &est);
        assert_eq!(
            String::from_utf8(stderr).expect("utf8"),
            "[cost] ~$0.0100 (10 posts, owned read, cache miss)\n"
        );
    }

    #[test]
    fn bookmarks_endpoint_counts_posts() {
        let body = serde_json::json!({
            "data": [{"id": "1"}, {"id": "2"}]
        });
        let est = estimate_cost(&Request::get("/2/users/123/bookmarks"), &body, false);
        assert_eq!(est.posts_read, 2);
        assert_eq!(est.users_read, 0);
    }
}
