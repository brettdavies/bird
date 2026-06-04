//! Pre-clap argv pre-scanning helpers.
//!
//! These run before the main parser to determine the output format used for
//! clap's own error envelopes. Pure functions; no I/O beyond reading
//! `BIRD_OUTPUT` and stderr TTY status.

use crate::output::OutputFormat;
use std::io::IsTerminal;

/// Pre-scan argv for an EXPLICIT output flag (`--output json`, `--output=json`,
/// `-o json`, `--json`, `--jsonl`). Returns `None` if no explicit flag is set
/// (caller may then consult env vars or auto-detect from TTY).
pub fn explicit_output_from_argv(argv: &[String]) -> Option<OutputFormat> {
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].as_str();
        if a == "--json" {
            return Some(OutputFormat::Json);
        }
        if a == "--jsonl" {
            return Some(OutputFormat::Jsonl);
        }
        if (a == "-o" || a == "--output")
            && let Some(v) = argv.get(i + 1)
            && let Some(f) = parse_output_value(v)
        {
            return Some(f);
        }
        if let Some(rest) = a.strip_prefix("--output=")
            && let Some(f) = parse_output_value(rest)
        {
            return Some(f);
        }
        if let Some(rest) = a.strip_prefix("-o=")
            && let Some(f) = parse_output_value(rest)
        {
            return Some(f);
        }
        i += 1;
    }
    None
}

/// Pre-scan argv plus env for the format to use when emitting the envelope on
/// clap parse failures. Falls back to TTY auto-detection.
pub fn output_from_argv(argv: &[String]) -> OutputFormat {
    if let Some(f) = explicit_output_from_argv(argv) {
        return f;
    }
    if let Ok(env) = std::env::var("BIRD_OUTPUT")
        && let Some(f) = parse_output_value(&env)
    {
        return f;
    }
    if std::io::stderr().is_terminal() {
        OutputFormat::Text
    } else {
        OutputFormat::Json
    }
}

pub fn parse_output_value(v: &str) -> Option<OutputFormat> {
    match v {
        "json" => Some(OutputFormat::Json),
        "jsonl" => Some(OutputFormat::Jsonl),
        "ndjson" => Some(OutputFormat::Ndjson),
        "text" => Some(OutputFormat::Text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_from_argv_detects_json_flag() {
        let argv = vec!["bird".to_string(), "--json".to_string(), "me".to_string()];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }

    #[test]
    fn output_from_argv_detects_jsonl_flag() {
        let argv = vec![
            "bird".to_string(),
            "bookmarks".to_string(),
            "--jsonl".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Jsonl);
    }

    #[test]
    fn output_from_argv_detects_output_separate_value() {
        let argv = vec![
            "bird".to_string(),
            "--output".to_string(),
            "json".to_string(),
            "me".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }

    #[test]
    fn output_from_argv_detects_output_equals_value() {
        let argv = vec![
            "bird".to_string(),
            "--output=jsonl".to_string(),
            "bookmarks".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Jsonl);
    }

    #[test]
    fn output_from_argv_detects_short_o_value() {
        let argv = vec![
            "bird".to_string(),
            "-o".to_string(),
            "json".to_string(),
            "me".to_string(),
        ];
        assert_eq!(output_from_argv(&argv), OutputFormat::Json);
    }
}
