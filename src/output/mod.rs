//! Terminal output: color mode, output format, success/error envelopes, and styled helpers.

pub mod envelope;
pub mod hints;

use crate::error::BirdError;
use clap::ValueEnum;
use hints::HintContext;
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

    /// Write the ok envelope around `payload` in the active structured format.
    pub fn print_ok(
        &self,
        out: &mut dyn std::io::Write,
        payload: serde_json::Value,
    ) -> std::io::Result<()> {
        self.write_structured(out, &ok_envelope(payload))
    }

    /// Write the ok envelope, indented when `pretty` asks for it whatever the
    /// output format.
    pub fn print_ok_pretty(
        &self,
        out: &mut dyn std::io::Write,
        payload: serde_json::Value,
        pretty: bool,
    ) -> std::io::Result<()> {
        if pretty {
            let body = serde_json::to_string_pretty(&ok_envelope(payload))
                .map_err(std::io::Error::other)?;
            writeln!(out, "{body}")
        } else {
            self.print_ok(out, payload)
        }
    }

    /// Write `{"status":"ok","message":...}`.
    pub fn print_ok_message(&self, out: &mut dyn std::io::Write, msg: &str) -> std::io::Result<()> {
        self.print_ok(out, serde_json::json!({ "message": msg }))
    }

    /// Write the dry-run envelope: what the command would have done, with
    /// `ctx`'s keys at the top level.
    pub fn print_dry_run(
        &self,
        out: &mut dyn std::io::Write,
        would_succeed: bool,
        exit_code: i32,
        ctx: serde_json::Value,
    ) -> std::io::Result<()> {
        self.write_structured(out, &dry_run_envelope(would_succeed, exit_code, ctx))
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
    ///
    /// Structured modes emit `xr`'s error envelope,
    /// `{"status":"error","reason","exit_code","message","next_step",...}`,
    /// with the recovery step `hints::for_error` chooses from `ctx`. Text mode
    /// renders a colored prefix, the message, and the same step as a hint
    /// line. `BirdError::print` is the fatal-only sibling that targets the
    /// process stderr directly when no `OutputConfig` is available yet.
    pub fn print_error(
        &self,
        stderr: &mut dyn std::io::Write,
        err: &BirdError,
        ctx: &HintContext,
    ) -> std::io::Result<()> {
        let hint = hints::for_error(err, ctx);
        if self.format.is_json() {
            let mut body = err.body(sanitize_for_stderr(err.message(), 1000));
            body.next_step = hint.map(|h| h.next_step);
            self.write_structured(stderr, &body.into_value())
        } else {
            writeln!(
                stderr,
                "{}{}",
                error(&err.text_prefix(), self.use_color),
                err.message()
            )?;
            for line in hint.iter().flat_map(|h| &h.text_lines) {
                writeln!(stderr, "{line}")?;
            }
            Ok(())
        }
    }

    /// Renders `value` in the active structured format: `json` indented,
    /// compact under `--raw`; `jsonl` and `ndjson` always on one line.
    pub fn write_structured(
        &self,
        w: &mut dyn std::io::Write,
        value: &serde_json::Value,
    ) -> std::io::Result<()> {
        let body = if self.format == OutputFormat::Json && !self.raw {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(std::io::Error::other)?;
        writeln!(w, "{body}")
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

/// Locks the shared diagnostic writer `BirdClient` and `BirdDb` hold. A panic
/// on another thread poisons the mutex but leaves the writer intact, so the
/// guard is recovered rather than turning a warning into a second panic.
pub(crate) fn lock_writer<'a>(
    writer: &'a std::sync::Mutex<dyn std::io::Write + Send + 'static>,
) -> std::sync::MutexGuard<'a, dyn std::io::Write + Send + 'static> {
    writer
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

/// The payload as an object: a value that is not one sits under `data`.
fn payload_object(payload: serde_json::Value) -> serde_json::Value {
    if payload.is_object() {
        payload
    } else {
        serde_json::json!({ "data": payload })
    }
}

/// `{"status":"ok", ...payload}`.
pub fn ok_envelope(payload: serde_json::Value) -> serde_json::Value {
    serde_json::to_value(envelope::Envelope::Ok {
        payload: payload_object(payload),
    })
    .unwrap_or_else(|_| serde_json::json!({"status": "ok"}))
}

/// `{"status":"dry_run","would_succeed","exit_code", ...ctx}`.
pub fn dry_run_envelope(
    would_succeed: bool,
    exit_code: i32,
    ctx: serde_json::Value,
) -> serde_json::Value {
    serde_json::to_value(envelope::Envelope::DryRun {
        would_succeed,
        exit_code,
        payload: payload_object(ctx),
    })
    .unwrap_or_else(|_| serde_json::json!({"status": "dry_run"}))
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
    fn print_ok_flattens_the_payload_beside_status() {
        let cfg = json_cfg();
        let mut buf = Vec::new();
        cfg.print_ok(&mut buf, json!({"path": "/x", "healthy": true}))
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(
            parsed,
            json!({"status": "ok", "path": "/x", "healthy": true})
        );

        let mut buf = Vec::new();
        cfg.print_ok(&mut buf, json!(["a", "b"])).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed, json!({"status": "ok", "data": ["a", "b"]}));
    }

    #[test]
    fn print_dry_run_carries_would_succeed_and_exit_code() {
        let mut buf = Vec::new();
        json_cfg()
            .print_dry_run(&mut buf, true, 0, json!({"command": "like"}))
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(
            parsed,
            json!({"status": "dry_run", "would_succeed": true, "exit_code": 0, "command": "like"})
        );
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

    fn error_doc(cfg: &OutputConfig, err: &BirdError, ctx: &HintContext) -> serde_json::Value {
        let mut buf = Vec::new();
        cfg.print_error(&mut buf, err, ctx).unwrap();
        serde_json::from_slice(&buf).expect("error envelope is JSON")
    }

    #[test]
    fn print_error_json_writes_xr_envelope() {
        let doc = error_doc(
            &json_cfg(),
            &BirdError::config("missing"),
            &HintContext::default(),
        );
        assert_eq!(doc["status"], "error");
        assert_eq!(doc["reason"], "config-error");
        assert_eq!(doc["exit_code"], 78);
        assert_eq!(doc["message"], "missing");
        for retired in ["error", "kind", "meta"] {
            assert!(doc.get(retired).is_none(), "{retired} must be gone: {doc}");
        }
    }

    #[test]
    fn print_error_json_is_indented_and_compact_under_raw() {
        let err = BirdError::config("missing");
        let mut pretty = Vec::new();
        json_cfg()
            .print_error(&mut pretty, &err, &HintContext::default())
            .unwrap();
        assert!(String::from_utf8(pretty).unwrap().lines().count() > 1);

        let raw = OutputConfig {
            raw: true,
            ..json_cfg()
        };
        let mut compact = Vec::new();
        raw.print_error(&mut compact, &err, &HintContext::default())
            .unwrap();
        assert_eq!(String::from_utf8(compact).unwrap().lines().count(), 1);
    }

    #[test]
    fn a_usage_error_points_at_the_commands_help() {
        let ctx = HintContext {
            help: "bird thread --help".into(),
            ..HintContext::default()
        };
        let doc = error_doc(
            &json_cfg(),
            &BirdError::usage(envelope::Reason::InvalidArgs, "bad flag"),
            &ctx,
        );
        assert_eq!(doc["next_step"]["action"], "show-help");
        assert_eq!(doc["next_step"]["command"], "bird thread --help");
    }

    #[test]
    fn a_refused_confirmation_carries_the_forced_invocation() {
        let ctx = HintContext {
            invocation: vec!["bird".into(), "like".into(), "1".into()],
            ..HintContext::default()
        };
        let doc = error_doc(&json_cfg(), &BirdError::confirmation_required("like"), &ctx);
        assert_eq!(doc["reason"], "confirmation-required");
        assert_eq!(doc["exit_code"], 1);
        assert_eq!(doc["command"], "like");
        assert_eq!(doc["next_step"]["action"], "confirm");
        assert_eq!(doc["next_step"]["template"], "bird like 1 --force");
    }

    #[test]
    fn a_library_error_takes_xdks_step_and_docs() {
        let err = BirdError::from_source("thread", Box::new(xdk::Error::api(503, "Unavailable")));
        let doc = error_doc(&json_cfg(), &err, &HintContext::default());
        assert_eq!(doc["reason"], "server-error");
        assert_eq!(doc["next_step"]["action"], "retry");
        assert_eq!(
            doc["next_step"]["docs"],
            "https://docs.x.com/x-api/fundamentals/response-codes-and-errors"
        );
    }

    #[test]
    fn text_mode_prints_the_hint_line_after_the_error() {
        let ctx = HintContext {
            help: "bird thread --help".into(),
            ..HintContext::default()
        };
        let mut buf = Vec::new();
        text_cfg()
            .print_error(
                &mut buf,
                &BirdError::usage(envelope::Reason::InvalidArgs, "bad flag"),
                &ctx,
            )
            .unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "usage error: bad flag\nTry 'bird thread --help'.\n"
        );
    }

    /// Every error envelope the emitter writes uses only keys the published
    /// envelope schema declares for its error variant, which closes the object.
    #[test]
    fn print_error_json_keys_are_declared_by_the_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../schema/envelope.schema.json"))
                .expect("envelope schema is JSON");
        let declared = schema["oneOf"]
            .as_array()
            .expect("schema lists the variants")
            .iter()
            .find(|v| v["properties"]["status"]["const"] == "error")
            .and_then(|v| v["properties"].as_object())
            .expect("schema declares the error variant");
        let errors = [
            BirdError::usage(envelope::Reason::InvalidArgs, "m"),
            BirdError::unknown_command("tweeet", Some("tweet".into()), "m"),
            BirdError::confirmation_required("like"),
            BirdError::config("m"),
            BirdError::from_source("search", Box::new(xdk::Error::api(429, "slow down"))),
            BirdError::from_source("search", Box::new(xdk::Error::http("reset"))),
        ];
        for err in &errors {
            let doc = error_doc(&json_cfg(), err, &HintContext::default());
            for key in doc.as_object().unwrap().keys() {
                assert!(
                    key == "status" || declared.contains_key(key),
                    "{key} undeclared: {doc}"
                );
            }
        }
    }

    // U11 edge-case coverage --------------------------------------------------

    /// Multi-line input must arrive at the writer with every embedded `\n`
    /// preserved; `print_message` adds exactly one trailing `\n`.
    #[test]
    fn print_message_preserves_embedded_newlines() {
        let cfg = text_cfg();
        let mut buf = Vec::new();
        cfg.print_message(&mut buf, "line one\nline two\nline three")
            .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "line one\nline two\nline three\n");
        // Three embedded `\n` (between lines) + one terminator.
        assert_eq!(s.matches('\n').count(), 3);
    }

    /// Under `jsonl` a nested ok document serializes to a single compact line
    /// with inner structure preserved.
    #[test]
    fn print_ok_jsonl_nested_object_serializes_compactly() {
        let cfg = OutputConfig {
            format: OutputFormat::Jsonl,
            ..json_cfg()
        };
        let mut buf = Vec::new();
        let env = json!({
            "data": {
                "user": {"id": "1", "name": "alice"},
                "items": [1, 2, 3],
            },
            "meta": {"count": 3, "nested": {"deep": true}},
        });
        cfg.print_ok(&mut buf, env).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'), "envelope ends with newline");
        // Exactly one terminating newline — no embedded ones.
        assert_eq!(s.matches('\n').count(), 1);
        let parsed: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(parsed["data"]["user"]["name"], "alice");
        assert_eq!(parsed["meta"]["nested"]["deep"], true);
    }

    /// With `use_color = true` in text mode the error prefix must carry an
    /// ANSI escape sequence (`\x1b[`). With color disabled it must not.
    #[test]
    fn print_error_text_color_emits_ansi() {
        let cfg = OutputConfig {
            format: OutputFormat::Text,
            use_color: true,
            quiet: false,
            raw: false,
        };
        let mut buf = Vec::new();
        let err = BirdError::config("missing");
        cfg.print_error(&mut buf, &err, &HintContext::default())
            .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains('\x1b'),
            "use_color=true should embed ANSI escape, got: {:?}",
            s
        );
        // The same call with color disabled must be plain.
        let plain = text_cfg();
        let mut plain_buf = Vec::new();
        plain
            .print_error(&mut plain_buf, &err, &HintContext::default())
            .unwrap();
        let plain_s = String::from_utf8(plain_buf).unwrap();
        assert!(
            !plain_s.contains('\x1b'),
            "use_color=false must not embed ANSI escape, got: {:?}",
            plain_s
        );
    }

    /// Empty message strings produce just the terminator — no panic, no double
    /// newline.
    #[test]
    fn print_message_empty_writes_single_newline() {
        let cfg = text_cfg();
        let mut buf = Vec::new();
        cfg.print_message(&mut buf, "").unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "\n");
    }

    /// Large input (~64 KiB) must round-trip through the writer without loss
    /// or truncation.
    #[test]
    fn print_message_large_input_round_trips() {
        let cfg = text_cfg();
        let big = "x".repeat(64 * 1024);
        let mut buf = Vec::new();
        cfg.print_message(&mut buf, &big).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s.len(), big.len() + 1, "preserve content + 1 newline");
        assert!(s.starts_with("xxxx"));
        assert!(s.ends_with("x\n"));
    }
}
