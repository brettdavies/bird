//! Bookmark and like notes: written once, when first seen, linking an
//! account's note to a post's note.

use std::path::PathBuf;

use super::markdown::escape_text;
use super::note::{Kind, Note, RegionSource};
use super::yaml::Field;

/// The relation note recording that `account_id` bookmarked or liked
/// `post_id`. Both ids are already validated.
pub fn relation_note(kind: Kind, account_id: &str, account: Option<&str>, post_id: &str) -> Note {
    let (type_name, dir, verb) = match kind {
        Kind::Like => ("x-like", "likes", "liked"),
        _ => ("x-bookmark", "bookmarks", "bookmarked"),
    };
    let post_url = format!("https://x.com/i/status/{post_id}");
    let mut keys = vec![
        ("type", Field::Str(type_name.into())),
        ("account_id", Field::Str(account_id.into())),
    ];
    if let Some(name) = account {
        keys.push(("account", Field::Str(name.into())));
    }
    keys.push(("post_id", Field::Str(post_id.into())));
    keys.push(("post_url", Field::Str(post_url)));
    let who = account.map_or_else(|| format!("user {account_id}"), |n| format!("@{n}"));
    let region = format!(
        "[{}](../../users/{account_id}.md) {verb} [post {post_id}](../../posts/{post_id}.md)",
        escape_text(&who)
    );
    Note {
        kind,
        path: PathBuf::from(dir)
            .join(account_id)
            .join(format!("{post_id}.md")),
        keys,
        source: RegionSource::Fixed(region),
    }
}
