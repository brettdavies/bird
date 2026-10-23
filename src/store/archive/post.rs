//! A post's note: its keys and its region.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

use super::known::Known;
use super::markdown::{escape_text, link, safe_url};
use super::note::{
    Kind, Note, RegionSource, display_time, int_key, list_key, str_key, str_of, valid_id,
};
use super::yaml::Field;

/// Each referenced-post key, the reference type it reads, and its region label.
const REFS: [(&str, &str, &str); 3] = [
    ("reply_to", "replied_to", "Replying to"),
    ("quotes", "quoted", "Quoting"),
    ("repost_of", "retweeted", "Reposting"),
];

/// The `includes` of one response that a post's note reads: authors' usernames
/// and media by key.
#[derive(Default)]
pub struct Includes<'a> {
    usernames: HashMap<&'a str, &'a str>,
    media: HashMap<&'a str, &'a Value>,
}

impl<'a> Includes<'a> {
    /// Indexes a response body's `includes`.
    pub fn of(body: &'a Value) -> Self {
        let includes = body.get("includes");
        let list = |name: &str| {
            includes
                .and_then(|i| i.get(name))
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
        };
        Self {
            usernames: list("users")
                .iter()
                .filter_map(|u| Some((str_of(u, "id")?, str_of(u, "username")?)))
                .collect(),
            media: list("media")
                .iter()
                .filter_map(|m| Some((str_of(m, "media_key")?, m)))
                .collect(),
        }
    }

    /// The username of user `id`, when the response included it.
    pub fn username(&self, id: &str) -> Option<&'a str> {
        self.usernames.get(id).copied()
    }
}

/// The post id a referenced-post entry of `kind` points at.
fn referenced(post: &Value, kind: &str) -> Option<String> {
    ["referenced_posts", "referenced_tweets"]
        .iter()
        .find_map(|f| post.get(*f).and_then(Value::as_array))?
        .iter()
        .find(|r| str_of(r, "type") == Some(kind))
        .and_then(|r| str_of(r, "id"))
        .filter(|id| valid_id(id))
        .map(str::to_string)
}

/// The strings at `entities.<list>[].<field>`, falling back to `fallback`.
fn entity_strings(post: &Value, list: &str, field: &str, fallback: Option<&str>) -> Vec<String> {
    post.get("entities")
        .and_then(|e| e.get(list))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    str_of(item, field)
                        .or_else(|| fallback.and_then(|f| str_of(item, f)))
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `(type, url)` of each attached media item the response included.
fn media(post: &Value, includes: &Includes<'_>) -> Vec<(String, String)> {
    post.get("attachments")
        .and_then(|a| a.get("media_keys"))
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .filter_map(|k| includes.media.get(k))
                .filter_map(|m| {
                    let url = str_of(m, "url").or_else(|| str_of(m, "preview_image_url"))?;
                    let kind = str_of(m, "type").unwrap_or("media");
                    Some((kind.to_string(), url.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The note for `post`, or the offending id when it cannot name a file.
pub fn post_note(post: &Value, includes: &Includes<'_>) -> Result<Note, String> {
    let id = str_of(post, "id").unwrap_or_default();
    if !valid_id(id) {
        return Err(id.to_string());
    }
    let url = format!("https://x.com/i/status/{id}");
    let author_id = str_of(post, "author_id").filter(|a| valid_id(a));
    let author = author_id.and_then(|a| includes.username(a));
    let attached = media(post, includes);

    let mut keys = vec![
        ("type", Field::Str("x-post".into())),
        ("id", Field::Str(id.into())),
        ("url", Field::Str(url)),
    ];
    if let Some(a) = author_id {
        keys.push(("author_id", Field::Str(a.into())));
    }
    if let Some(name) = author {
        keys.push(("author", Field::Str(name.into())));
    }
    str_key(&mut keys, "created_at", post, "created_at");
    str_key(&mut keys, "conversation_id", post, "conversation_id");
    for (key, kind, _) in REFS {
        if let Some(target) = referenced(post, kind) {
            keys.push((key, Field::Str(target)));
        }
    }
    str_key(&mut keys, "lang", post, "lang");
    let metrics = post.get("public_metrics");
    for (name, fields) in [
        ("like_count", &["like_count"][..]),
        ("repost_count", &["repost_count", "retweet_count"][..]),
        ("reply_count", &["reply_count"][..]),
        ("quote_count", &["quote_count"][..]),
        ("bookmark_count", &["bookmark_count"][..]),
        ("impression_count", &["impression_count"][..]),
    ] {
        int_key(&mut keys, name, metrics, fields);
    }
    list_key(
        &mut keys,
        "hashtags",
        entity_strings(post, "hashtags", "tag", None),
    );
    list_key(
        &mut keys,
        "mentions",
        entity_strings(post, "mentions", "username", None),
    );
    list_key(
        &mut keys,
        "links",
        entity_strings(post, "urls", "expanded_url", Some("url")),
    );
    list_key(
        &mut keys,
        "media",
        attached.iter().map(|(_, u)| u.clone()).collect(),
    );

    let text = post
        .get("note_tweet")
        .and_then(|n| str_of(n, "text"))
        .or_else(|| str_of(post, "text"))
        .unwrap_or_default();
    Ok(Note {
        kind: Kind::Post,
        path: PathBuf::from("posts").join(format!("{id}.md")),
        keys,
        source: RegionSource::Post {
            text: text.to_string(),
            media: attached,
        },
    })
}

/// A post's region: its text, a line linking its author's note with its UTC
/// time and X URL, a line per referenced post, and a link per media item.
pub fn region(known: &Known<'_>, text: &str, media: &[(String, String)]) -> String {
    let mut meta = Vec::new();
    if let Some(a) = known.str("author_id").filter(|a| valid_id(a)) {
        let name = known
            .str("author")
            .map_or_else(|| format!("user {a}"), |n| format!("@{n}"));
        meta.push(format!("[{}](../users/{a}.md)", escape_text(&name)));
    }
    if let Some(t) = known.str("created_at") {
        meta.push(display_time(t));
    }
    meta.push(format!("[x.com]({})", known.str("url").unwrap_or_default()));
    let mut lines = vec![escape_text(text), String::new(), meta.join(" · ")];
    for (key, _, label) in REFS {
        if let Some(target) = known.str(key).filter(|t| valid_id(t)) {
            lines.push(format!("{label} [post {target}]({target}.md)"));
        }
    }
    if media.is_empty() {
        lines.extend(
            known
                .list("media")
                .into_iter()
                .map(|u| stored_media(known, u)),
        );
    } else {
        lines.extend(media.iter().map(|(kind, url)| link(kind, url)));
    }
    lines.join("\n")
}

/// The link for a media URL the response did not include: the stored
/// region's line for it, which names its type, else a `media` link.
fn stored_media(known: &Known<'_>, url: &str) -> String {
    let line = safe_url(url).and_then(|href| {
        let tail = format!("]({href})");
        known.stored_line(|l| {
            l.strip_suffix(tail.as_str())
                .and_then(|l| l.strip_prefix('['))
                .is_some_and(|label| !label.contains("]("))
        })
    });
    line.map_or_else(|| link("media", url), str::to_string)
}
