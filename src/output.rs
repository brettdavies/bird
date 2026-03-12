//! Terminal output: color choice for clap, styled helpers, and hyperlinks.

use owo_colors::OwoColorize;
use std::io::IsTerminal;

/// Color choice for clap help/errors: respect NO_COLOR and TERM=dumb, and TTY.
pub fn color_choice_for_clap() -> clap::ColorChoice {
    let stderr_tty = std::io::stderr().is_terminal();
    let no_color_env = std::env::var("NO_COLOR").is_ok();
    let term_dumb = std::env::var("TERM").as_deref() == Ok("dumb");
    if !stderr_tty || no_color_env || term_dumb {
        clap::ColorChoice::Never
    } else {
        clap::ColorChoice::Auto
    }
}

/// Section header (bold white). When use_color is false, returns s unchanged.
pub fn section(s: &str, use_color: bool) -> String {
    if use_color {
        s.bold().white().to_string()
    } else {
        s.to_string()
    }
}

/// Command name (bold cyan).
pub fn command(s: &str, use_color: bool) -> String {
    if use_color {
        s.bold().cyan().to_string()
    } else {
        s.to_string()
    }
}

/// Muted/secondary text (dim gray).
pub fn muted(s: &str, use_color: bool) -> String {
    if use_color {
        s.bright_black().to_string()
    } else {
        s.to_string()
    }
}

/// Error prefix (red).
pub fn error(s: &str, use_color: bool) -> String {
    if use_color {
        s.red().to_string()
    } else {
        s.to_string()
    }
}

/// Success (green).
pub fn success(s: &str, use_color: bool) -> String {
    if use_color {
        s.green().to_string()
    } else {
        s.to_string()
    }
}

/// Strip lines containing ANSI escape sequences from stdout output.
/// Used as fallback when `NO_COLOR=1` doesn't suppress hardcoded ANSI in xurl error paths.
/// Filters complete lines (not individual sequences) to avoid corrupting JSON structure.
pub fn strip_ansi_lines(s: &str) -> String {
    s.lines()
        .filter(|line| !line.contains('\x1b'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Sanitize untrusted text for stderr display: replace control chars with '?', truncate.
/// Prevents terminal escape injection from API response bodies.
pub fn sanitize_for_stderr(s: &str, max_chars: usize) -> String {
    s.chars()
        .take(max_chars)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Emoji for "available" when use_emoji; otherwise empty string.
pub fn emoji_available(use_emoji: bool) -> &'static str {
    if use_emoji {
        "✅ "
    } else {
        ""
    }
}

/// Emoji for "unavailable" when use_emoji; otherwise empty string.
pub fn emoji_unavailable(use_emoji: bool) -> &'static str {
    if use_emoji {
        "❌ "
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_lines_clean_json() {
        let input = "{\"data\":{\"id\":\"1\"}}\n";
        assert_eq!(strip_ansi_lines(input), "{\"data\":{\"id\":\"1\"}}");
    }

    #[test]
    fn strip_ansi_lines_removes_colored_error() {
        let input = "{\"data\":{\"id\":\"1\"}}\n\x1b[31mError: request failed\x1b[0m";
        assert_eq!(strip_ansi_lines(input), "{\"data\":{\"id\":\"1\"}}");
    }

    #[test]
    fn strip_ansi_lines_preserves_all_clean() {
        let input = "line one\nline two\nline three";
        assert_eq!(strip_ansi_lines(input), input);
    }

    #[test]
    fn strip_ansi_lines_empty() {
        assert_eq!(strip_ansi_lines(""), "");
    }

    #[test]
    fn sanitize_normal_text() {
        assert_eq!(sanitize_for_stderr("hello world", 100), "hello world");
    }

    #[test]
    fn sanitize_strips_escape() {
        assert_eq!(
            sanitize_for_stderr("a\x1b[31mred\x1b[0m", 100),
            "a?[31mred?[0m"
        );
    }

    #[test]
    fn sanitize_strips_bel() {
        assert_eq!(sanitize_for_stderr("a\x07b", 100), "a?b");
    }

    #[test]
    fn sanitize_strips_newlines() {
        assert_eq!(sanitize_for_stderr("line1\nline2", 100), "line1?line2");
    }

    #[test]
    fn sanitize_truncates() {
        assert_eq!(sanitize_for_stderr("abcdef", 3), "abc");
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(sanitize_for_stderr("", 100), "");
    }

    #[test]
    fn sanitize_at_exact_limit() {
        assert_eq!(sanitize_for_stderr("abc", 3), "abc");
    }
}
