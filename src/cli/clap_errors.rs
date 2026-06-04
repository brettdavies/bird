//! Map clap parse errors to `BirdError::Usage` (or `None` for help/version).
//!
//! Pure function; no I/O. Help/version display is left to the caller, which
//! routes to stdout and exits zero.

use crate::error::BirdError;

/// Convert a clap parse error to a `BirdError::Usage` (for non-help cases) or
/// route help/version to stdout directly. Returns `None` when the error was a
/// help/version display (program should exit 0).
pub fn clap_error_to_bird(err: &clap::Error) -> Option<BirdError> {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => None,
        _ => {
            let error_id = match err.kind() {
                ErrorKind::UnknownArgument => "unknown-argument",
                ErrorKind::MissingRequiredArgument => "missing-required-argument",
                ErrorKind::MissingSubcommand => "missing-subcommand",
                ErrorKind::InvalidSubcommand => "invalid-subcommand",
                ErrorKind::InvalidValue => "invalid-value",
                ErrorKind::TooManyValues => "too-many-values",
                ErrorKind::TooFewValues => "too-few-values",
                ErrorKind::ArgumentConflict => "argument-conflict",
                ErrorKind::NoEquals => "missing-equals",
                ErrorKind::ValueValidation => "invalid-value",
                _ => "invalid-arguments",
            };
            Some(BirdError::usage(error_id, err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn clap_error_unknown_argument_maps_to_usage() {
        let err = clap::Error::new(ErrorKind::UnknownArgument);
        let mapped = clap_error_to_bird(&err);
        assert!(mapped.is_some());
        let b = mapped.unwrap();
        assert_eq!(b.exit_code(), 2);
    }

    #[test]
    fn clap_error_display_help_returns_none() {
        let err = clap::Error::new(ErrorKind::DisplayHelp);
        assert!(clap_error_to_bird(&err).is_none());
    }
}
