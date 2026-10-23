//! The Markdown archive: one note per post, user, bookmark, and like, named by
//! X id, under a directory the user can point at a notes vault.
//!
//! What a response archives is decided by its endpoint shape, not a path
//! prefix, so a liked-posts or timeline page files posts and never users.

pub mod frontmatter;
pub mod markdown;
pub mod merge;
pub mod note;
pub mod post;
pub mod relation;
pub mod user;
pub mod yaml;

#[cfg(test)]
mod merge_tests;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use self::merge::MergeError;
use self::note::{Kind, Note, valid_id};
use self::post::{Includes, post_note};
use self::relation::relation_note;
use self::user::user_note;
use super::status::{Area, StoreStatus};

/// Shapes whose `data` holds posts.
const POST_SHAPES: &[&str] = &[
    "/2/tweets",
    "/2/tweets/{}",
    "/2/tweets/search/recent",
    "/2/tweets/search/all",
    "/2/tweets/{}/quote_tweets",
    "/2/users/{}/tweets",
    "/2/users/{}/mentions",
    "/2/users/{}/timelines/reverse_chronological",
    "/2/lists/{}/tweets",
    "/2/users/{}/bookmarks",
    "/2/users/{}/liked_tweets",
];

/// Shapes whose `data` holds users.
const USER_SHAPES: &[&str] = &[
    "/2/users",
    "/2/users/{}",
    "/2/users/me",
    "/2/users/by",
    "/2/users/by/username/{}",
    "/2/users/{}/followers",
    "/2/users/{}/following",
    "/2/users/{}/muting",
    "/2/users/{}/blocking",
    "/2/tweets/{}/liking_users",
    "/2/tweets/{}/retweeted_by",
    "/2/lists/{}/members",
];

/// The endpoint shape of `url`: its path with every all-digit segment, and
/// the segment after `/by/username/`, written as `{}`.
pub fn endpoint_shape(url: &str) -> String {
    let path = url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.split(['?', '#']).next().unwrap_or_default().to_string());
    let segments: Vec<&str> = path.split('/').collect();
    segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let after_username = i >= 2 && segments[i - 2] == "by" && segments[i - 1] == "username";
            let id = i > 1 && !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit());
            if after_username || id { "{}" } else { seg }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The relation a shape adds for the user id in its path.
fn relation_of(shape: &str) -> Option<Kind> {
    match shape {
        "/2/users/{}/bookmarks" => Some(Kind::Bookmark),
        "/2/users/{}/liked_tweets" => Some(Kind::Like),
        _ => None,
    }
}

/// The notes a response body yields, and every id that could not name one.
pub fn plan(url: &str, body: &Value, account: Option<&str>) -> (Vec<Note>, Vec<String>) {
    let shape = endpoint_shape(url);
    let holds_posts = POST_SHAPES.contains(&shape.as_str());
    if !holds_posts && !USER_SHAPES.contains(&shape.as_str()) {
        return (Vec::new(), Vec::new());
    }
    let includes = Includes::of(body);
    let items = |v: Option<&Value>| -> Vec<Value> {
        match v {
            Some(Value::Array(a)) => a.clone(),
            Some(o @ Value::Object(_)) => vec![o.clone()],
            _ => Vec::new(),
        }
    };
    let included = |name: &str| items(body.get("includes").and_then(|i| i.get(name)));
    let mut results: Vec<Result<Note, String>> = Vec::new();
    let data = items(body.get("data"));
    if holds_posts {
        results.extend(data.iter().map(|p| post_note(p, &includes)));
    } else {
        results.extend(data.iter().map(user_note));
    }
    let account_id = url::Url::parse(url)
        .ok()
        .and_then(|u| u.path().split('/').nth(3).map(str::to_string))
        .filter(|id| valid_id(id));
    if let (Some(kind), Some(account_id)) = (relation_of(&shape), account_id) {
        let relations: Vec<Note> = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .filter_map(|n| match n.get("id") {
                Some(yaml::Field::Str(id)) => Some(relation_note(kind, &account_id, account, id)),
                _ => None,
            })
            .collect();
        results.extend(relations.into_iter().map(Ok));
    }
    results.extend(included("users").iter().map(user_note));
    for name in ["tweets", "posts"] {
        results.extend(included(name).iter().map(|p| post_note(p, &includes)));
    }

    let mut seen = HashSet::new();
    let (mut notes, mut bad) = (Vec::new(), Vec::new());
    for result in results {
        match result {
            Ok(note) if seen.insert(note.path.clone()) => notes.push(note),
            Ok(_) => {}
            Err(id) => bad.push(id),
        }
    }
    (notes, bad)
}

/// The archive under one root directory.
pub struct Archive {
    root: PathBuf,
    status: Arc<StoreStatus>,
}

impl Archive {
    /// An archive rooted at `root`, reporting failures to `status`.
    pub fn new(root: PathBuf, status: Arc<StoreStatus>) -> Self {
        Self { root, status }
    }

    /// The archive's root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Archives what a 2xx GET body holds, filed by the endpoint shape of
    /// `url`. `account` is the requesting username when known. Every failure
    /// goes to the store status; nothing is returned.
    pub fn observe(
        &self,
        url: &str,
        body: &Value,
        account: Option<&str>,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        let (notes, bad) = plan(url, body, account);
        for id in bad {
            let err = io::Error::new(
                io::ErrorKind::InvalidData,
                format!("id {id:?} is not 1 to 20 digits"),
            );
            self.status
                .fail(Area::Archive, "archive id check", &self.root, &err);
        }
        let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        for note in notes {
            self.write(&note, &stamp);
        }
    }

    fn write(&self, note: &Note, stamp: &str) {
        let path = self.root.join(&note.path);
        if let Err(e) = merge::write_note(&path, note, stamp) {
            let what = match e {
                MergeError::Io(_) => "archive write",
                MergeError::Unparseable(_) | MergeError::Changed => "archive merge",
            };
            self.status.fail(Area::Archive, what, &path, &e.to_io());
        }
    }
}
