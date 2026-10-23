//! Frontmatter values as bird renders them.
//!
//! Every string is a YAML double-quoted scalar: JSON string escaping, which
//! YAML 1.2 accepts, plus `\u` escapes for the code points YAML forbids
//! unescaped. Lists are block sequences, the style Obsidian writes.

use std::fmt::Write as _;

/// One frontmatter value bird writes.
#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    /// A double-quoted string.
    Str(String),
    /// A plain integer.
    Int(u64),
    /// `true` or `false`.
    Bool(bool),
    /// A block sequence of double-quoted strings.
    List(Vec<String>),
}

impl Field {
    /// The value as a YAML parser hands it back, for comparing with a stored
    /// note.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Str(s) => serde_json::Value::String(s.clone()),
            Self::Int(n) => serde_json::Value::from(*n),
            Self::Bool(b) => serde_json::Value::Bool(*b),
            Self::List(items) => items
                .iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        }
    }
}

/// A double-quoted YAML scalar holding exactly `s`.
pub fn scalar(s: &str) -> String {
    let json = serde_json::Value::String(s.to_string()).to_string();
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if forbidden_unescaped(c) {
            let _ = write!(out, "\\u{:04X}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// Code points YAML does not allow in a double-quoted scalar unescaped, or
/// reads as a line break: DEL, the C1 controls, the line and paragraph
/// separators, and the byte-order mark.
fn forbidden_unescaped(c: char) -> bool {
    matches!(
        c,
        '\u{7F}'..='\u{9F}' | '\u{2028}' | '\u{2029}' | '\u{FEFF}'
    )
}

/// Renders one `key: value` entry, ending in a newline.
pub fn entry(key: &str, value: &Field) -> String {
    match value {
        Field::Str(s) => format!("{key}: {}\n", scalar(s)),
        Field::Int(n) => format!("{key}: {n}\n"),
        Field::Bool(b) => format!("{key}: {b}\n"),
        Field::List(items) => {
            let mut out = format!("{key}:\n");
            for item in items {
                let _ = writeln!(out, "  - {}", scalar(item));
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(s: &str) -> serde_json::Value {
        let doc = entry("v", &Field::Str(s.to_string()));
        let parsed: serde_json::Value = serde_saphyr::from_str(&doc).expect("parses");
        parsed["v"].clone()
    }

    #[test]
    fn yaml_scalars_round_trip() {
        let corpus = [
            "a: b",
            "# not a comment",
            "say \"hi\"",
            "back\\slash",
            "line one\nline two",
            "next\u{85}line",
            "sep\u{2028}arator",
            "para\u{2029}graph",
            "bom\u{FEFF}",
            "del\u{7F}",
            "tab\there",
            "emoji 🐦‍⬛ and ünïcödé",
            "yes",
            "no",
            "null",
            "~",
            "true",
            "123",
            "1849000000000000000",
            "@handle",
            "*alias",
            "&anchor",
            "!tag",
            "%directive",
            "| literal",
            "> folded",
            "- dash",
            "[flow]",
            "{map: 1}",
            "' single",
            "",
            "  leading and trailing  ",
        ];
        for s in corpus {
            assert_eq!(round_trip(s), serde_json::Value::String(s.into()), "{s:?}");
        }
    }

    #[test]
    fn ids_render_as_strings() {
        let doc = entry("id", &Field::Str("1849000000000000000".into()));
        assert_eq!(doc, "id: \"1849000000000000000\"\n");
        let parsed: serde_json::Value = serde_saphyr::from_str(&doc).expect("parses");
        assert_eq!(parsed["id"], "1849000000000000000");
    }

    #[test]
    fn lists_are_block_sequences() {
        let doc = entry("tags", &Field::List(vec!["a".into(), "b: c".into()]));
        assert_eq!(doc, "tags:\n  - \"a\"\n  - \"b: c\"\n");
        let parsed: serde_json::Value = serde_saphyr::from_str(&doc).expect("parses");
        assert_eq!(parsed["tags"], serde_json::json!(["a", "b: c"]));
    }

    #[test]
    fn numbers_and_booleans_are_plain() {
        assert_eq!(entry("n", &Field::Int(42)), "n: 42\n");
        assert_eq!(entry("b", &Field::Bool(true)), "b: true\n");
        let parsed: serde_json::Value = serde_saphyr::from_str("n: 42\nb: true\n").expect("parses");
        assert_eq!(parsed["n"], Field::Int(42).to_json());
        assert_eq!(parsed["b"], Field::Bool(true).to_json());
    }
}
