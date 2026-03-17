//! Entity store: SQLite-backed entity storage with usage tracking.
//! BirdDb is the application database -- entities (tweets, users), bookmarks,
//! raw responses, and usage data.
//! Store failures are never fatal: the Option<BirdDb> pattern degrades to API-only mode.

pub mod client;
#[allow(clippy::module_inception)]
pub mod db;
pub mod usage;

// Re-export all public types so `use crate::db::{...}` works.
pub use client::{BirdClient, CacheOpts, RequestContext};
#[allow(unused_imports)]
pub(crate) use db::{BirdDb, BookmarkRow, RawResponseRow, StoreStats, TweetRow, UserRow};
pub use usage::{ActualUsageDay, DailyUsage, EndpointUsage, UsageLogEntry, UsageSummary};

use std::time::{SystemTime, UNIX_EPOCH};

/// Known literal path segments that should never be replaced with `:id`.
const KNOWN_LITERALS: &[&str] = &[
    "2",
    "tweets",
    "users",
    "search",
    "recent",
    "bookmarks",
    "me",
    "by",
    "username",
    "usage",
    "oauth2",
    "token",
    "compliance",
    "lists",
    "spaces",
    "dm_conversations",
];

/// Normalize a URL to an endpoint pattern for usage grouping.
/// Replaces numeric ID segments (2+ digits) with `:id` and the parameter after
/// `/by/username/` with `:username`. Returns the path only (strips scheme, host, query params).
pub fn normalize_endpoint(url: &str) -> String {
    let path = url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string());

    let segments: Vec<&str> = path.split('/').collect();
    let mut normalized = Vec::with_capacity(segments.len());
    let mut prev_two: (Option<&str>, Option<&str>) = (None, None);

    for seg in &segments {
        if seg.is_empty() {
            normalized.push(*seg);
            continue;
        }
        if prev_two == (Some("by"), Some("username")) {
            normalized.push(":username");
        } else if KNOWN_LITERALS.contains(seg) {
            normalized.push(seg);
        } else if seg.len() >= 2 && seg.chars().all(|c| c.is_ascii_digit()) {
            normalized.push(":id");
        } else {
            normalized.push(seg);
        }
        prev_two = (prev_two.1, Some(seg));
    }

    normalized.join("/")
}

/// Current UNIX timestamp in seconds.
pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_endpoint_search_recent() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/tweets/search/recent?query=test"),
            "/2/tweets/search/recent"
        );
    }

    #[test]
    fn normalize_endpoint_users_me() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/me"),
            "/2/users/me"
        );
    }

    #[test]
    fn normalize_endpoint_tweet_by_id() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/tweets/1234567890"),
            "/2/tweets/:id"
        );
    }

    #[test]
    fn normalize_endpoint_user_bookmarks() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/123/bookmarks"),
            "/2/users/:id/bookmarks"
        );
    }

    #[test]
    fn normalize_endpoint_username_lookup() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/users/by/username/jack"),
            "/2/users/by/username/:username"
        );
    }

    #[test]
    fn normalize_endpoint_usage_tweets() {
        assert_eq!(
            normalize_endpoint("https://api.x.com/2/usage/tweets?usage.fields=daily_project_usage"),
            "/2/usage/tweets"
        );
    }

    #[test]
    fn normalize_endpoint_strips_query() {
        let url = "https://api.x.com/2/tweets/search/recent?query=rust&max_results=100";
        assert_eq!(normalize_endpoint(url), "/2/tweets/search/recent");
    }
}
