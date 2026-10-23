//! A note as bird renders it: its kind, path, bird's keys, and region.

use std::path::PathBuf;

use super::frontmatter::Stored;
use super::known::Known;
use super::media::{self, Attachment};
use super::yaml::{Field, entry};
use super::{post, user};

/// The line that opens bird's region in a note body.
pub const REGION_BEGIN: &str = "<!-- bird:begin -->";
/// The line that closes bird's region in a note body.
pub const REGION_END: &str = "<!-- bird:end -->";
/// The note format version every note carries as `bird_format`.
pub const BIRD_FORMAT: u64 = 1;

/// What a note describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A post, under `posts/`.
    Post,
    /// A user, under `users/`.
    User,
    /// A bookmark, under `bookmarks/<account id>/`.
    Bookmark,
    /// A like, under `likes/<account id>/`.
    Like,
}

const POST_KEYS: &[&str] = &[
    "type",
    "id",
    "url",
    "author_id",
    "author",
    "created_at",
    "conversation_id",
    "reply_to",
    "quotes",
    "repost_of",
    "lang",
    "like_count",
    "repost_count",
    "reply_count",
    "quote_count",
    "bookmark_count",
    "impression_count",
    "hashtags",
    "mentions",
    "links",
    "media",
    "media_keys",
    "archived_at",
    "updated_at",
    "bird_format",
];

const USER_KEYS: &[&str] = &[
    "type",
    "id",
    "username",
    "name",
    "url",
    "description",
    "location",
    "website",
    "created_at",
    "verified",
    "protected",
    "profile_image_url",
    "followers_count",
    "following_count",
    "post_count",
    "listed_count",
    "like_count",
    "media_count",
    "pinned_post_id",
    "aliases",
    "archived_at",
    "updated_at",
    "bird_format",
];

const RELATION_KEYS: &[&str] = &[
    "type",
    "account_id",
    "account",
    "post_id",
    "post_url",
    "archived_at",
    "bird_format",
];

impl Kind {
    /// Every key bird owns in this kind of note, in the order it writes them.
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Post => POST_KEYS,
            Self::User => USER_KEYS,
            Self::Bookmark | Self::Like => RELATION_KEYS,
        }
    }

    /// Whether a note of this kind, once written, is never rewritten.
    pub fn create_only(self) -> bool {
        matches!(self, Self::Bookmark | Self::Like)
    }
}

/// What a note's region shows besides bird's keys.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionSource {
    /// A post's text, and the media keys its response names.
    Post {
        /// The text as X sent it.
        text: String,
        /// Each media key, or `None` when the response names none.
        attachments: Option<Vec<Attachment>>,
    },
    /// A user's region, which reads bird's keys alone.
    User,
    /// A relation's region, written once and never rewritten.
    Fixed(String),
}

/// One note an observation yields.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// What the note describes.
    pub kind: Kind,
    /// Its path relative to the archive root.
    pub path: PathBuf,
    /// bird's keys the observation carries, without the timestamps and
    /// `bird_format`, which [`Note::render`] adds.
    pub keys: Vec<(&'static str, Field)>,
    /// What the region shows besides those keys.
    pub source: RegionSource,
}

impl Note {
    /// The value the observation carries for `key`.
    pub fn get(&self, key: &str) -> Option<&Field> {
        self.keys.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// The text between the region markers, without them. Each of bird's
    /// keys the observation lacks is read from `stored`.
    pub fn region(&self, stored: Option<&Stored<'_>>) -> String {
        let known = Known::new(&self.keys, stored);
        match &self.source {
            RegionSource::Post { text, attachments } => {
                post::region(&known, text, attachments.as_deref())
            }
            RegionSource::User => user::region(&known),
            RegionSource::Fixed(region) => region.clone(),
        }
    }

    /// bird's value for `key` with `stored` merged in: a post's `media` and
    /// `media_keys` keep the stored URL of each key the response did not
    /// include; every other key is the observation's.
    pub fn value(&self, key: &str, stored: Option<&Stored<'_>>) -> Option<Field> {
        let RegionSource::Post {
            attachments: Some(attached),
            ..
        } = &self.source
        else {
            return self.get(key).cloned();
        };
        let urls = match key {
            "media" => true,
            "media_keys" => false,
            _ => return self.get(key).cloned(),
        };
        let known = Known::new(&self.keys, stored);
        let items: Vec<String> = media::resolve(attached, &known)
            .iter()
            .map(|m| if urls { m.url } else { m.key }.to_string())
            .collect();
        (!items.is_empty()).then_some(Field::List(items))
    }

    /// bird's frontmatter entries in key order, with the given timestamps.
    pub fn frontmatter(&self, archived_at: &str, updated_at: &str) -> String {
        let mut out = String::new();
        for &key in self.kind.keys() {
            let value = match key {
                "archived_at" => Some(Field::Str(archived_at.to_string())),
                "updated_at" => Some(Field::Str(updated_at.to_string())),
                "bird_format" => Some(Field::Int(BIRD_FORMAT)),
                _ => self.get(key).cloned(),
            };
            if let Some(value) = value {
                out.push_str(&entry(key, &value));
            }
        }
        out
    }

    /// The whole file for a new note first written at `now`.
    pub fn render(&self, now: &str) -> String {
        format!(
            "---\n{}---\n{}",
            self.frontmatter(now, now),
            region_block(&self.region(None))
        )
    }
}

/// The region with its markers, ending in a newline.
pub fn region_block(region: &str) -> String {
    format!("{REGION_BEGIN}\n{region}\n{REGION_END}\n")
}

/// Whether `id` can name a note: 1 to 20 ASCII digits, which no target OS
/// reads as anything but a plain file name.
pub fn valid_id(id: &str) -> bool {
    (1..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

/// A string field of `v`, when present.
pub fn str_of<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(serde_json::Value::as_str)
}

/// A string key built from `v[field]`, when present.
pub fn str_key(
    keys: &mut Vec<(&'static str, Field)>,
    name: &'static str,
    v: &serde_json::Value,
    field: &str,
) {
    if let Some(s) = str_of(v, field) {
        keys.push((name, Field::Str(s.to_string())));
    }
}

/// An integer key read from the first of `fields` present in `metrics`.
pub fn int_key(
    keys: &mut Vec<(&'static str, Field)>,
    name: &'static str,
    metrics: Option<&serde_json::Value>,
    fields: &[&str],
) {
    let found = fields.iter().find_map(|f| {
        metrics
            .and_then(|m| m.get(*f))
            .and_then(serde_json::Value::as_u64)
    });
    if let Some(n) = found {
        keys.push((name, Field::Int(n)));
    }
}

/// A list key, written only when it has items.
pub fn list_key(keys: &mut Vec<(&'static str, Field)>, name: &'static str, items: Vec<String>) {
    if !items.is_empty() {
        keys.push((name, Field::List(items)));
    }
}

/// The UTC time a note shows for an X timestamp.
pub fn display_time(created_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(created_at)
        .map(|t| {
            t.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| super::markdown::escape_text(created_at))
}
