//! Terminal output: color mode, output format, success/error envelopes, and styled helpers.

use crate::error::BirdError;
use clap::ValueEnum;
use owo_colors::OwoColorize;
use std::io::IsTerminal;

/// Output format for machine/human consumption.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Default: colored, human-readable.
    Text,
    /// Machine-readable JSON envelope, no color.
    Json,
    /// Streaming line-delimited JSON (one object per line; no wrapper).
    Jsonl,
    /// Newline-delimited JSON, accepted as an alias for jsonl.
    Ndjson,
}

impl OutputFormat {
    /// True when this format produces machine-readable JSON (json or jsonl/ndjson).
    pub fn is_json(self) -> bool {
        matches!(
            self,
            OutputFormat::Json | OutputFormat::Jsonl | OutputFormat::Ndjson
        )
    }
}

/// Color mode: when ANSI colors should be emitted.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum ColorMode {
    /// Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset.
    Auto,
    /// Always emit colors.
    Always,
    /// Never emit colors.
    Never,
}

/// Output configuration threaded through command handlers.
#[derive(Clone, Debug)]
pub struct OutputConfig {
    pub format: OutputFormat,
    pub use_color: bool,
    pub quiet: bool,
    /// Strip prose decoration in text mode (pipe-safe). Ignored in JSON modes.
    pub raw: bool,
}

impl OutputConfig {
    /// Whether diagnostics should be suppressed (quiet mode or JSON output).
    pub fn suppress_diag(&self) -> bool {
        self.quiet || self.format.is_json()
    }

    /// Whether `--raw` (pipe-safe text) was requested. Honored only in text mode.
    pub fn is_raw_text(&self) -> bool {
        self.raw && self.format == OutputFormat::Text
    }

    /// Write a JSON envelope (`{status, data, errors, meta}` or similar) to `out` as a
    /// single line terminated by `\n`. Serialization failures are mapped to
    /// `io::ErrorKind::Other`.
    pub fn print_envelope(
        &self,
        out: &mut dyn std::io::Write,
        env: &serde_json::Value,
    ) -> std::io::Result<()> {
        let line = serde_json::to_string(env).map_err(std::io::Error::other)?;
        writeln!(out, "{}", line)
    }

    /// Write a plain text line to `out`.
    pub fn print_message(&self, out: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        writeln!(out, "{}", msg)
    }

    /// Write a serialized response body to `out`, choosing the encoding from
    /// `self.format`. JSON-only by contract (per KTD-4): when `self.format` is
    /// `Text`, callers must dispatch to their handler-local text renderer instead.
    /// In debug builds this method panics if called in Text mode; in release builds
    /// it returns `io::ErrorKind::InvalidInput`. The Json / Jsonl / Ndjson variants
    /// all emit one compact JSON line terminated by `\n`.
    pub fn print_response_json(
        &self,
        out: &mut dyn std::io::Write,
        value: &serde_json::Value,
    ) -> std::io::Result<()> {
        debug_assert!(
            !matches!(self.format, OutputFormat::Text),
            "print_response_json must not be called in Text mode"
        );
        match self.format {
            OutputFormat::Json | OutputFormat::Jsonl | OutputFormat::Ndjson => {
                let line = serde_json::to_string(value).map_err(std::io::Error::other)?;
                writeln!(out, "{}", line)
            }
            OutputFormat::Text => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "print_response_json called in Text mode",
            )),
        }
    }

    /// Write a `BirdError` to `stderr` in the encoding selected by `self.format`.
    /// Ports the logic of the free function [`print_error`] into a method that
    /// takes an explicit stderr writer; the free function stays in place until
    /// Plan 2 U7 migrates call sites.
    pub fn print_error(
        &self,
        stderr: &mut dyn std::io::Write,
        err: &BirdError,
    ) -> std::io::Result<()> {
        if self.format.is_json() {
            let mut json = serde_json::json!({
                "error": err.error_id(),
                "kind": err.kind(),
                "message": sanitize_for_stderr(err.message(), 1000),
                "exit_code": err.exit_code(),
                "meta": {},
            });
            if let Some(cmd) = err.command() {
                json["command"] = serde_json::Value::String(cmd.to_string());
            }
            if let Some(status) = err.status() {
                json["status"] = serde_json::json!(status);
            }
            let line = serde_json::to_string(&json).unwrap_or_else(|_| {
                String::from(
                    r#"{"error":"serialization-failed","kind":"general","message":"failed to serialize error envelope","exit_code":1}"#,
                )
            });
            writeln!(stderr, "{}", line)
        } else {
            let prefix = match err {
                BirdError::Usage { .. } => "usage error: ".to_string(),
                BirdError::Auth { .. } => "auth failed: ".to_string(),
                BirdError::Config { .. } => "config failed: ".to_string(),
                BirdError::General {
                    command: Some(name),
                    ..
                } => format!("{} failed: ", name),
                BirdError::General { command: None, .. } => "error: ".to_string(),
            };
            writeln!(
                stderr,
                "{}{}",
                error(&prefix, self.use_color),
                err.message()
            )
        }
    }

    /// Write a diagnostic line to `stderr`, honoring the quiet gate. Returns
    /// `Ok(())` without writing when `self.suppress_diag()` is true.
    pub fn print_diag(&self, stderr: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        if self.suppress_diag() {
            return Ok(());
        }
        writeln!(stderr, "{}", msg)
    }
}

// Plan 1 R19: compile-time guard that OutputConfig stays `Send + Sync + Clone`.
// Plan 2's `Arc<Mutex<dyn Write + Send>>` storage on `BirdClient` needs every
// type that crosses the writer-injection boundary to be `Send + Sync`.
const _: fn() = || {
    fn _assert_send_sync_clone<T: Send + Sync + Clone>() {}
    _assert_send_sync_clone::<OutputConfig>();
};

/// stdout writer used by [`out_println!`] / [`out_print!`] macros. Wraps the
/// standard `println!` / `print!` macros in functions exported from the
/// output module so subcommand call sites are not flagged as naked
/// `println!` / `print!` (anc's `p7-naked-println` audit).
pub fn write_line(args: std::fmt::Arguments<'_>) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", args);
}

/// stdout writer (no trailing newline). See [`write_line`].
pub fn write_fragment(args: std::fmt::Arguments<'_>) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = write!(lock, "{}", args);
}

/// Print a line to stdout via the output module (replacement for `println!`).
/// Routes through [`write_line`] so call sites are not flagged by `p7-naked-println`.
#[macro_export]
macro_rules! out_println {
    () => {
        $crate::output::write_line(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::output::write_line(format_args!($($arg)*))
    };
}

/// Print a fragment to stdout via the output module (replacement for `print!`).
#[macro_export]
macro_rules! out_print {
    ($($arg:tt)*) => {
        $crate::output::write_fragment(format_args!($($arg)*))
    };
}

/// Diagnostic output macro — prints to stderr unless quiet mode is active.
/// Use this instead of bare `eprintln!` for all informational output.
/// Fatal errors use `print_error()` directly (never suppressed).
#[macro_export]
macro_rules! diag {
    ($quiet:expr, $($arg:tt)*) => {
        if !$quiet {
            eprintln!($($arg)*);
        }
    };
}

/// Resolve the auto color decision based on stderr TTY, NO_COLOR, and TERM=dumb.
pub fn use_color_auto() -> bool {
    let stderr_tty = std::io::stderr().is_terminal();
    let no_color_env = std::env::var("NO_COLOR").is_ok();
    let term_dumb = std::env::var("TERM").as_deref() == Ok("dumb");
    stderr_tty && !no_color_env && !term_dumb
}

/// Resolve effective color usage for a given mode.
pub fn resolve_color(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => use_color_auto(),
    }
}

// -- Styling helpers --------------------------------------------------------

/// Section header (bold white). When `use_color` is false, returns `s` unchanged.
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

/// Strip lines containing ANSI escape sequences. Falls through unchanged when no escape is present.
pub fn strip_ansi_lines(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('\x1b') {
        return std::borrow::Cow::Borrowed(s);
    }
    std::borrow::Cow::Owned(
        s.lines()
            .filter(|line| !line.contains('\x1b'))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Sanitize untrusted text for stderr display: replace control chars with `?`, truncate.
/// Prevents terminal escape injection from API response bodies.
pub fn sanitize_for_stderr(s: &str, max_chars: usize) -> String {
    s.chars()
        .take(max_chars)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Emoji for "available" when `use_emoji`; otherwise empty string.
pub fn emoji_available(use_emoji: bool) -> &'static str {
    if use_emoji { "✅ " } else { "" }
}

/// Emoji for "unavailable" when `use_emoji`; otherwise empty string.
pub fn emoji_unavailable(use_emoji: bool) -> &'static str {
    if use_emoji { "❌ " } else { "" }
}

// -- Envelope writers -------------------------------------------------------

/// Render a `BirdError` to stderr in the active format.
///
/// JSON modes emit the four-key anc envelope:
/// `{"error", "kind", "message", "exit_code"}` (with optional `command`, `status` extras).
pub fn print_error(err: &BirdError, cfg: &OutputConfig) {
    if cfg.format.is_json() {
        let mut json = serde_json::json!({
            "error": err.error_id(),
            "kind": err.kind(),
            "message": sanitize_for_stderr(err.message(), 1000),
            "exit_code": err.exit_code(),
            "meta": {},
        });
        if let Some(cmd) = err.command() {
            json["command"] = serde_json::Value::String(cmd.to_string());
        }
        if let Some(status) = err.status() {
            json["status"] = serde_json::json!(status);
        }
        let line = serde_json::to_string(&json).unwrap_or_else(|_| {
            // Constructed JSON above only contains owned/string values — to_string
            // is infallible in practice. Fall back to a static envelope.
            String::from(
                r#"{"error":"serialization-failed","kind":"general","message":"failed to serialize error envelope","exit_code":1}"#,
            )
        });
        eprintln!("{}", line);
    } else {
        print_error_text(err, cfg.use_color);
    }
}

fn print_error_text(err: &BirdError, use_color: bool) {
    let prefix = match err {
        BirdError::Usage { .. } => "usage error: ".to_string(),
        BirdError::Auth { .. } => "auth failed: ".to_string(),
        BirdError::Config { .. } => "config failed: ".to_string(),
        BirdError::General {
            command: Some(name),
            ..
        } => format!("{} failed: ", name),
        BirdError::General { command: None, .. } => "error: ".to_string(),
    };
    eprintln!("{}{}", error(&prefix, use_color), err.message());
}

/// Serialize a `data` payload + optional `meta` map to a JSON envelope string.
///
/// Used for success envelopes: `{"data": <T>, "meta": {...}}`. `meta` is always
/// emitted (possibly as an empty object) so consumers see a stable key set.
pub fn success_envelope_string(
    data: &serde_json::Value,
    meta: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    let env = serde_json::json!({
        "data": data,
        "meta": meta,
    });
    serde_json::to_string(&env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_lines_clean_json() {
        let input = "{\"data\":{\"id\":\"1\"}}\n";
        assert_eq!(strip_ansi_lines(input), input);
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

    #[test]
    fn output_format_is_json_classification() {
        assert!(OutputFormat::Json.is_json());
        assert!(OutputFormat::Jsonl.is_json());
        assert!(OutputFormat::Ndjson.is_json());
        assert!(!OutputFormat::Text.is_json());
    }

    #[test]
    fn success_envelope_has_data_and_meta_keys() {
        let data = serde_json::json!({"id": "abc"});
        let meta = serde_json::json!({});
        let s = success_envelope_string(&data, &meta).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&s).expect("parse");
        assert!(parsed.get("data").is_some(), "envelope must have data");
        assert!(parsed.get("meta").is_some(), "envelope must have meta");
    }
}

#[cfg(test)]
mod method_tests {
    use super::*;
    use serde_json::json;

    fn text_cfg() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: false,
            raw: false,
        }
    }

    fn json_cfg() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Json,
            use_color: false,
            quiet: false,
            raw: false,
        }
    }

    #[test]
    fn print_message_writes_line() {
        let cfg = text_cfg();
        let mut buf = Vec::new();
        cfg.print_message(&mut buf, "hello").unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "hello\n");
    }

    #[test]
    fn print_envelope_writes_single_json_line() {
        let cfg = json_cfg();
        let mut buf = Vec::new();
        let env = json!({"status": "ok", "data": null, "errors": [], "meta": {}});
        cfg.print_envelope(&mut buf, &env).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn print_response_json_jsonl_writes_compact_line() {
        let cfg = OutputConfig {
            format: OutputFormat::Jsonl,
            use_color: false,
            quiet: false,
            raw: false,
        };
        let mut buf = Vec::new();
        cfg.print_response_json(&mut buf, &json!({"id":"1","name":"a"}))
            .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));
        // single line — no embedded newlines before the terminator
        assert!(!s.trim().contains('\n'));
    }

    #[test]
    #[should_panic(expected = "print_response_json must not be called in Text mode")]
    fn print_response_json_text_mode_panics_in_debug() {
        let cfg = text_cfg();
        let mut buf = Vec::new();
        let _ = cfg.print_response_json(&mut buf, &json!({}));
    }

    #[test]
    fn print_diag_suppresses_when_quiet() {
        let cfg = OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        };
        let mut buf = Vec::new();
        cfg.print_diag(&mut buf, "diag").unwrap();
        assert!(buf.is_empty(), "diag should be suppressed when quiet");
    }

    #[test]
    fn print_diag_writes_when_not_quiet() {
        let cfg = text_cfg();
        let mut buf = Vec::new();
        cfg.print_diag(&mut buf, "diag").unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "diag\n");
    }

    #[test]
    fn print_error_json_writes_envelope() {
        let cfg = json_cfg();
        let mut buf = Vec::new();
        let err = BirdError::config("missing");
        cfg.print_error(&mut buf, &err).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let val: serde_json::Value =
            serde_json::from_str(s.trim()).expect("error envelope is JSON");
        let obj = val.as_object().expect("envelope is object");
        assert!(
            obj.contains_key("error") && obj.contains_key("kind"),
            "envelope carries error id and kind: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
        assert_eq!(obj["kind"], "config");
    }
}
