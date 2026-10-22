//! Map clap parse errors to `BirdError` (or `None` for help/version).
//!
//! Pure function; no I/O. Help/version display is left to the caller, which
//! routes to stdout and exits zero.

use crate::error::BirdError;
use crate::output::envelope::Reason;

/// Convert a clap parse error to a `BirdError`. Returns `None` for a
/// help/version display, which the caller prints to stdout at exit 0.
///
/// A word that names no subcommand is `unknown-command`, echoing the word and
/// clap's nearest match; every other parse failure is `invalid-args`, as `xr`
/// reports them.
pub fn clap_error_to_bird(err: &clap::Error) -> Option<BirdError> {
    use clap::error::{ContextKind, ContextValue, ErrorKind};
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => None,
        ErrorKind::InvalidSubcommand => {
            let first = |kind| match err.get(kind) {
                Some(ContextValue::String(s)) => Some(s.clone()),
                Some(ContextValue::Strings(v)) => v.first().cloned(),
                _ => None,
            };
            let word = first(ContextKind::InvalidSubcommand).unwrap_or_default();
            let suggestion = first(ContextKind::SuggestedSubcommand);
            Some(BirdError::unknown_command(
                word,
                suggestion,
                err.to_string(),
            ))
        }
        _ => Some(BirdError::usage(Reason::InvalidArgs, err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn clap_error_unknown_argument_maps_to_invalid_args() {
        let err = clap::Error::new(ErrorKind::UnknownArgument);
        let b = clap_error_to_bird(&err).expect("a parse failure");
        assert_eq!(b.exit_code(), 2);
        assert_eq!(b.error_id(), "invalid-args");
    }

    #[test]
    fn clap_error_display_help_returns_none() {
        let err = clap::Error::new(ErrorKind::DisplayHelp);
        assert!(clap_error_to_bird(&err).is_none());
    }

    #[test]
    fn an_unknown_subcommand_echoes_the_word_and_claps_suggestion() {
        use clap::CommandFactory;
        let err = crate::cli::Cli::command()
            .try_get_matches_from(["bird", "thred"])
            .expect_err("thred is no command");
        let b = clap_error_to_bird(&err).expect("a parse failure");
        let body = b.body(String::new());
        assert_eq!(b.error_id(), "unknown-command");
        assert_eq!(body.command.as_deref(), Some("thred"));
        assert_eq!(body.suggestion.as_deref(), Some("thread"));
    }
}
