//! A post's media: the keys its response names, resolved to URLs.
//!
//! A post's media are fixed once posted, so a key whose `includes.media`
//! entry the response lacks keeps the URL the stored note has for it; the
//! note's `media_keys` and `media` lists are parallel for that lookup.

use serde_json::Value;

use super::known::Known;
use super::markdown::{link, safe_url};
use super::note::str_of;
use super::post::Includes;

/// One media key a post's response names, with the `(type, url)` of its
/// `includes.media` entry when the response carried one.
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    key: String,
    included: Option<(String, String)>,
}

/// One of a post's media: its key and URL, and its type when the response
/// included it.
pub struct Media<'a> {
    /// The media key.
    pub key: &'a str,
    /// The media URL.
    pub url: &'a str,
    kind: Option<&'a str>,
}

/// Each media key `post` names, in order, or `None` when it names none.
pub fn attachments(post: &Value, includes: &Includes<'_>) -> Option<Vec<Attachment>> {
    let keys = post.get("attachments")?.get("media_keys")?.as_array()?;
    let attached = keys
        .iter()
        .filter_map(Value::as_str)
        .map(|key| Attachment {
            key: key.to_string(),
            included: includes.media(key).and_then(|m| {
                let url = str_of(m, "url").or_else(|| str_of(m, "preview_image_url"))?;
                let kind = str_of(m, "type").unwrap_or("media");
                Some((kind.to_string(), url.to_string()))
            }),
        })
        .collect();
    Some(attached)
}

/// The post's media in the response's key order: each key's `includes.media`
/// entry, else the stored note's URL for that key. A key neither names is
/// left out.
pub fn resolve<'a>(attached: &'a [Attachment], known: &Known<'a>) -> Vec<Media<'a>> {
    let stored_keys = known.stored_list("media_keys");
    let stored_urls = known.stored_list("media");
    attached
        .iter()
        .filter_map(|a| match &a.included {
            Some((kind, url)) => Some(Media {
                key: &a.key,
                url,
                kind: Some(kind),
            }),
            None => stored_keys
                .iter()
                .position(|k| *k == a.key)
                .and_then(|i| stored_urls.get(i))
                .map(|url| Media {
                    key: &a.key,
                    url,
                    kind: None,
                }),
        })
        .collect()
}

/// One region line per media item: the resolved media when any, else the
/// `media` list the note keeps.
pub fn lines(known: &Known<'_>, attached: Option<&[Attachment]>) -> Vec<String> {
    let media = attached.map(|a| resolve(a, known)).unwrap_or_default();
    if media.is_empty() {
        return known
            .list("media")
            .into_iter()
            .map(|url| stored_line(known, url))
            .collect();
    }
    media
        .iter()
        .map(|m| match m.kind {
            Some(kind) => link(kind, m.url),
            None => stored_line(known, m.url),
        })
        .collect()
}

/// The link for a media URL the response did not include: the stored
/// region's line for it, which names its type, else a `media` link.
fn stored_line(known: &Known<'_>, url: &str) -> String {
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
