//! A user's note: its keys and its region.

use std::path::PathBuf;

use serde_json::Value;

use super::markdown::{escape_text, link};
use super::note::{Kind, Note, int_key, str_key, str_of, valid_id};
use super::yaml::Field;

/// The user's website: the expanded profile URL, else the short one.
fn website(user: &Value) -> Option<&str> {
    user.get("entities")
        .and_then(|e| e.get("url"))
        .and_then(|u| u.get("urls"))
        .and_then(Value::as_array)
        .and_then(|urls| urls.first())
        .and_then(|u| str_of(u, "expanded_url"))
        .or_else(|| str_of(user, "url"))
        .filter(|u| !u.is_empty())
}

/// The note for `user`, or the offending id when it cannot name a file.
pub fn user_note(user: &Value) -> Result<Note, String> {
    let id = str_of(user, "id").unwrap_or_default();
    if !valid_id(id) {
        return Err(id.to_string());
    }
    let url = format!("https://x.com/i/user/{id}");
    let username = str_of(user, "username");
    let site = website(user);

    let mut keys = vec![
        ("type", Field::Str("x-user".into())),
        ("id", Field::Str(id.into())),
    ];
    str_key(&mut keys, "username", user, "username");
    str_key(&mut keys, "name", user, "name");
    keys.push(("url", Field::Str(url.clone())));
    str_key(&mut keys, "description", user, "description");
    str_key(&mut keys, "location", user, "location");
    if let Some(site) = site {
        keys.push(("website", Field::Str(site.into())));
    }
    str_key(&mut keys, "created_at", user, "created_at");
    for flag in ["verified", "protected"] {
        if let Some(b) = user.get(flag).and_then(Value::as_bool) {
            keys.push((flag, Field::Bool(b)));
        }
    }
    str_key(&mut keys, "profile_image_url", user, "profile_image_url");
    let metrics = user.get("public_metrics");
    for (name, fields) in [
        ("followers_count", &["followers_count"][..]),
        ("following_count", &["following_count"][..]),
        ("post_count", &["post_count", "tweet_count"][..]),
        ("listed_count", &["listed_count"][..]),
        ("like_count", &["like_count"][..]),
        ("media_count", &["media_count"][..]),
    ] {
        int_key(&mut keys, name, metrics, fields);
    }
    if let Some(pinned) = ["pinned_post_id", "pinned_tweet_id"]
        .iter()
        .find_map(|f| str_of(user, f))
        .filter(|p| valid_id(p))
    {
        keys.push(("pinned_post_id", Field::Str(pinned.into())));
    }
    if let Some(name) = username {
        keys.push(("aliases", Field::List(vec![format!("@{name}")])));
    }

    let heading = match (str_of(user, "name"), username) {
        (Some(name), Some(handle)) => format!("{} (@{handle})", name),
        (Some(name), None) => name.to_string(),
        (None, Some(handle)) => format!("@{handle}"),
        (None, None) => format!("user {id}"),
    };
    let mut lines = vec![format!("# {}", escape_text(&heading))];
    if let Some(bio) = str_of(user, "description").filter(|b| !b.is_empty()) {
        lines.push(String::new());
        lines.push(escape_text(bio));
    }
    let mut meta = vec![format!("[x.com]({url})")];
    if let Some(site) = site {
        meta.push(link("website", site));
    }
    if let Some(place) = str_of(user, "location").filter(|l| !l.is_empty()) {
        meta.push(escape_text(place));
    }
    lines.push(String::new());
    lines.push(meta.join(" · "));
    Ok(Note {
        kind: Kind::User,
        path: PathBuf::from("users").join(format!("{id}.md")),
        keys,
        region: lines.join("\n"),
    })
}
