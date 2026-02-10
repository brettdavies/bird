//! Terminal output: color choice for clap, styled helpers, and hyperlinks.

use std::io::IsTerminal;
use owo_colors::OwoColorize;

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

/// Wrap URL in OSC 8 for clickable hyperlink. Sanitizes URL and display text (strips ASCII escape and BEL).
/// When use_hyperlinks is false, returns display_text or url unchanged.
pub fn hyperlink(url: &str, display_text: Option<&str>, use_hyperlinks: bool) -> String {
    if !use_hyperlinks {
        return display_text.unwrap_or(url).to_string();
    }
    let display = display_text.unwrap_or(url);
    let safe_url = url.replace('\x1b', "").replace('\x07', "");
    let safe_text = display.replace('\x1b', "").replace('\x07', "");
    format!("\x1b]8;;{}\x07{}\x1b]8;;\x07", safe_url, safe_text)
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
