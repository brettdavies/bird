//! A stored note read back: its frontmatter split into top-level entries
//! beside their parse, and its body around bird's region.

use serde_json::Value;

use super::note::{REGION_BEGIN, REGION_END};

/// One top-level frontmatter entry: its key and its raw lines.
pub struct Entry<'a> {
    /// The key, or `None` for lines before the first key.
    pub key: Option<String>,
    /// The entry's lines, terminators included.
    pub raw: &'a str,
}

/// A stored note split into frontmatter entries, their parse, and the body.
pub struct Stored<'a> {
    /// The top-level entries in file order.
    pub entries: Vec<Entry<'a>>,
    /// The frontmatter as YAML parses it.
    pub parsed: serde_json::Map<String, Value>,
    /// Everything after the closing delimiter.
    pub body: &'a str,
}

fn is_delimiter(line: &str) -> bool {
    line.trim_end_matches(['\n', '\r']) == "---"
}

/// The key a column-0 entry line opens, quotes removed.
fn key_of(line: &str) -> String {
    let quoted = line
        .chars()
        .next()
        .filter(|c| *c == '"' || *c == '\'')
        .and_then(|q| line[1..].find(q).map(|end| line[1..=end].to_string()));
    quoted.unwrap_or_else(|| {
        line.split(':')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    })
}

/// Splits a stored note, refusing a frontmatter that does not parse or
/// whose line split disagrees with the parse.
pub fn split(text: &str) -> Result<Stored<'_>, String> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().filter(|l| is_delimiter(l));
    let Some(first) = first else {
        return Err("the note has no frontmatter".into());
    };
    let start = first.len();
    let mut offset = start;
    let mut end = None;
    for line in lines {
        if is_delimiter(line) {
            end = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let (fm_end, body_start) = end.ok_or("the frontmatter is not closed")?;
    let fm = &text[start..fm_end];

    let mut entries: Vec<Entry<'_>> = Vec::new();
    let mut at = start;
    for line in fm.split_inclusive('\n') {
        let opens = !line.trim().is_empty() && !line.starts_with([' ', '\t', '-', '#']);
        match entries.last_mut() {
            Some(last) if !opens => {
                let from = at - last.raw.len();
                last.raw = &text[from..at + line.len()];
            }
            _ => entries.push(Entry {
                key: opens.then(|| key_of(line)),
                raw: &text[at..at + line.len()],
            }),
        }
        at += line.len();
    }

    let parsed: Value = serde_saphyr::from_str(fm).map_err(|e| e.to_string())?;
    let parsed = match parsed {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        _ => return Err("the frontmatter is not a mapping".into()),
    };
    let mut split_keys: Vec<&str> = entries.iter().filter_map(|e| e.key.as_deref()).collect();
    let before = split_keys.len();
    split_keys.sort_unstable();
    split_keys.dedup();
    let mut parsed_keys: Vec<&str> = parsed.keys().map(String::as_str).collect();
    parsed_keys.sort_unstable();
    if split_keys.len() != before || split_keys != parsed_keys {
        return Err("the frontmatter's lines do not match its keys".into());
    }
    Ok(Stored {
        entries,
        parsed,
        body: &text[body_start..],
    })
}

/// The body around bird's region: the text before it, the region, and the
/// text after it. A body with no region puts all of its text after.
pub fn region_parts(body: &str) -> (&str, Option<&str>, &str) {
    let mut offset = 0;
    let mut begin = None;
    for line in body.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        match begin {
            None if bare == REGION_BEGIN => begin = Some((offset, offset + line.len())),
            Some((b, inner)) if bare == REGION_END => {
                let region = body[inner..offset].trim_end_matches(['\n', '\r']);
                return (&body[..b], Some(region), &body[offset + line.len()..]);
            }
            _ => {}
        }
        offset += line.len();
    }
    ("", None, body)
}
