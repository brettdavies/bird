//! A note's `aliases`: the one bird computes for a post, so Obsidian's quick
//! switcher and link autocomplete find the post by its author, day, and
//! opening words, and the merge with the aliases a stored note holds.

use std::collections::HashSet;

use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use super::frontmatter::Stored;
use super::known::Known;
use super::markdown::decode_entities;
use super::note::{Kind, Note};
use super::yaml::Field;

/// The most Unicode scalar values an alias keeps of a post's text.
const TEXT_LIMIT: usize = 60;

/// Characters that end or redirect an Obsidian link or alias.
const LINK_SYNTAX: &[char] = &['[', ']', '|', '#', '^'];

/// `@<author> <YYYY-MM-DD>: <text>`, each part left out when unknown, or
/// `None` when no part is known. The author and UTC day come from the merged
/// keys; `text` is the post text the region shows.
pub fn post_alias(known: &Known<'_>, text: &str) -> Option<String> {
    let author = known
        .str("author")
        .filter(|a| !a.is_empty())
        .map(|a| format!("@{a}"));
    let day = known.str("created_at").and_then(utc_day);
    let head = [author, day].into_iter().flatten().collect::<Vec<_>>();
    let text = cut(&clean(text));
    match (head.is_empty(), text.is_empty()) {
        (true, true) => None,
        (true, false) => Some(text),
        (false, true) => Some(head.join(" ")),
        (false, false) => Some(format!("{}: {text}", head.join(" "))),
    }
}

fn utc_day(created_at: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(created_at)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc).format("%Y-%m-%d").to_string())
}

/// The text on one line: whitespace runs as one space, `t.co` links and
/// link syntax removed.
fn clean(text: &str) -> String {
    decode_entities(text)
        .split_whitespace()
        .filter(|word| !word.starts_with("https://t.co/"))
        .map(|word| word.replace(LINK_SYNTAX, ""))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` within [`TEXT_LIMIT`], cut between graphemes at the last space
/// that fits, else at the last grapheme that fits, then `…`.
fn cut(text: &str) -> String {
    if text.chars().count() <= TEXT_LIMIT {
        return text.to_string();
    }
    let (mut fits, mut space, mut count) = (0, None, 0);
    for (at, grapheme) in text.grapheme_indices(true) {
        if grapheme.starts_with(' ') {
            space = Some(at);
        }
        count += grapheme.chars().count();
        if count > TEXT_LIMIT {
            break;
        }
        fits = at + grapheme.len();
    }
    format!("{}…", &text[..space.unwrap_or(fits)])
}

/// `aliases` after a rewrite, or `None` when the stored entry already holds
/// every alias bird has, which keeps that entry as the user wrote it.
/// Otherwise bird's alias comes first, then the old username's on a user
/// note, then every stored alias, each once.
pub fn merged(note: &Note, stored: &Stored<'_>) -> Option<Field> {
    let mut ours: Vec<String> = match note.value("aliases", Some(stored)) {
        Some(Field::List(items)) => items,
        _ => Vec::new(),
    };
    if note.kind == Kind::User
        && let Some(old) = stored.parsed.get("username").and_then(Value::as_str)
    {
        ours.push(format!("@{old}"));
    }
    let theirs: Vec<String> = match stored.parsed.get("aliases") {
        Some(Value::Array(items)) => items.iter().filter_map(alias_text).collect(),
        Some(one) => alias_text(one).into_iter().collect(),
        None => Vec::new(),
    };
    if ours.iter().all(|a| theirs.contains(a)) {
        return None;
    }
    let mut seen = HashSet::new();
    let merged = ours
        .into_iter()
        .chain(theirs)
        .filter(|a| seen.insert(a.clone()))
        .collect();
    Some(Field::List(merged))
}

/// A stored alias as Obsidian reads it: a string, or a plain scalar YAML
/// parsed as a number or boolean.
fn alias_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
