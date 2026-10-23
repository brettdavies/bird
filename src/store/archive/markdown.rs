//! Text and link rendering for the note region.
//!
//! X-supplied text is escaped only where Markdown or Obsidian would read it as
//! syntax, so vault search still finds the text as posted. The escaper is also
//! the guard against a post planting a Dataview block, inline JavaScript, or
//! HTML in the user's vault.

/// Characters escaped wherever they appear.
const ALWAYS: &[char] = &['\\', '`', '*', '[', ']', '<', '|', '$'];

/// Characters escaped when doubled (`~~`, `==`, `%%`).
const DOUBLED: &[char] = &['~', '=', '%'];

/// Decodes the three entities X writes into post text.
pub fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Escapes X-supplied text for a note region, line by line.
pub fn escape_text(s: &str) -> String {
    decode_entities(s)
        .split('\n')
        .map(escape_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_line(line: &str) -> String {
    let line = line.trim_start_matches([' ', '\t']);
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 8);
    let marker = line_start_marker(&chars);
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        if i == marker || escapes(c, prev, next, &chars[i..], i == 0) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The index of a block marker that opens the line, when there is one: a
/// quote, a bullet, a setext underline, or the delimiter of `N.` / `N)`.
fn line_start_marker(chars: &[char]) -> usize {
    match chars.first() {
        Some('>' | '-' | '+' | '=' | '#') => 0,
        Some(c) if c.is_ascii_digit() => {
            let digits = chars.iter().take_while(|c| c.is_ascii_digit()).count();
            let after = chars.get(digits + 1);
            match chars.get(digits) {
                Some('.' | ')') if after.is_none_or(|c| c.is_whitespace()) => digits,
                _ => usize::MAX,
            }
        }
        _ => usize::MAX,
    }
}

fn escapes(c: char, prev: Option<char>, next: Option<char>, rest: &[char], first: bool) -> bool {
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    match c {
        _ if ALWAYS.contains(&c) => true,
        '_' => !word(prev) || !word(next),
        '#' => first || next.is_some_and(|n| !n.is_whitespace()),
        '&' => begins_entity(&rest[1..]),
        _ if DOUBLED.contains(&c) => prev == Some(c) || next == Some(c),
        '^' => block_ref(&rest[1..]),
        _ => false,
    }
}

/// Whether `rest` (after an `&`) reads as a named or numeric entity reference.
fn begins_entity(rest: &[char]) -> bool {
    let end = rest.iter().position(|&c| c == ';');
    let Some(end) = end.filter(|&e| e > 0 && e <= 32) else {
        return false;
    };
    let name = &rest[..end];
    match name {
        ['#', 'x' | 'X', hex @ ..] => !hex.is_empty() && hex.iter().all(char::is_ascii_hexdigit),
        ['#', dec @ ..] => !dec.is_empty() && dec.iter().all(char::is_ascii_digit),
        [first, ..] => first.is_ascii_alphabetic() && name.iter().all(char::is_ascii_alphanumeric),
        [] => false,
    }
}

/// Whether `rest` (after a `^`) is a word running to the line's end, which
/// Obsidian reads as a block reference.
fn block_ref(rest: &[char]) -> bool {
    !rest.is_empty() && rest.iter().all(|c| c.is_ascii_alphanumeric() || *c == '-')
}

/// A Markdown link to an X-supplied URL when its scheme is `http` or `https`;
/// any other URL is written as escaped text.
pub fn link(text: &str, url: &str) -> String {
    match safe_url(url) {
        Some(href) => format!("[{}]({href})", escape_text(text)),
        None => escape_text(url),
    }
}

/// `url` with spaces, parentheses, angle brackets, and controls
/// percent-encoded, when its scheme is `http` or `https`.
pub fn safe_url(url: &str) -> Option<String> {
    let scheme = url.split_once(':')?.0;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' | '(' | ')' | '<' | '>' => out.push_str(&format!("%{:02X}", u32::from(c))),
            c if c.is_control() => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Event, Parser};

    /// The text a CommonMark renderer shows for `md`, failing on any event
    /// that is not text or a line break.
    fn rendered_text(md: &str) -> String {
        let mut text = String::new();
        for event in Parser::new(md) {
            match event {
                Event::Text(t) => text.push_str(&t),
                Event::SoftBreak | Event::HardBreak => text.push('\n'),
                Event::Start(pulldown_cmark::Tag::Paragraph)
                | Event::End(pulldown_cmark::TagEnd::Paragraph) => {}
                other => panic!("{md:?} renders {other:?}"),
            }
        }
        text
    }

    #[test]
    fn escaped_text_renders_as_plain_text() {
        let corpus = [
            "# heading",
            "## also a heading",
            "**bold** and *em* and _under_",
            "[a](b) and ![img](c)",
            "<!-- bird:end -->",
            "<script>alert(1)</script>",
            "`code` and ```fence```",
            "| a | b |\n| - | - |\n| 1 | 2 |",
            "1. item",
            "2) item",
            "> quote",
            "- bullet",
            "+ plus",
            "Title\n===",
            "Title\n---",
            "&copy; and &#169; and &#xA9;",
            "~~~\nfenced\n~~~",
            "back\\slash and \\*",
            "    indented code",
            "trailing\\",
            "<https://e.com>",
            "[ref]: https://e.com",
        ];
        for text in corpus {
            let escaped = escape_text(text);
            let expected: String = text
                .split('\n')
                .map(|l| l.trim_start_matches([' ', '\t']))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(rendered_text(&escaped), expected, "{text:?} -> {escaped:?}");
        }
    }

    #[test]
    fn obsidian_syntax_is_inert() {
        assert_eq!(escape_text("[[link]]"), "\\[\\[link\\]\\]");
        assert_eq!(escape_text("==mark=="), "\\=\\=mark\\=\\=");
        assert_eq!(escape_text("%%c%%"), "\\%\\%c\\%\\%");
        assert_eq!(escape_text("$x$"), "\\$x\\$");
        assert_eq!(escape_text("see #tag"), "see \\#tag");
        assert_eq!(escape_text("block ^abc"), "block \\^abc");
        assert_eq!(escape_text("2^10 is big"), "2^10 is big");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let text = "@some_user said e.g. Q&A at 5.30";
        assert_eq!(escape_text(text), text);
        assert_eq!(
            escape_text("50% off, a=b, x ~ y, # 1"),
            "50% off, a=b, x ~ y, # 1"
        );
    }

    #[test]
    fn x_entities_are_decoded_first() {
        assert_eq!(escape_text("a &amp; b &lt;3 &gt;"), "a & b \\<3 >");
        assert_eq!(escape_text("&amp;copy;"), "\\&copy;");
    }

    #[test]
    fn leading_spaces_are_dropped() {
        assert_eq!(escape_text("  hi\n\tthere"), "hi\nthere");
    }

    #[test]
    fn only_http_urls_become_links() {
        assert_eq!(link("x", "javascript:alert(1)"), "javascript:alert(1)");
        assert_eq!(
            link("site", "https://e.com/a b(c)"),
            "[site](https://e.com/a%20b%28c%29)"
        );
        assert_eq!(link("s", "HTTP://e.com/<x>"), "[s](HTTP://e.com/%3Cx%3E)");
        assert!(safe_url("data:text/html,hi").is_none());
        assert!(safe_url("no scheme").is_none());
    }
}
