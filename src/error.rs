//! Structured error type and envelope mapping for the bird CLI.
//!
//! Variants map to distinct kinds and exit codes per the anc envelope contract:
//! - `Usage` -> kind "usage", exit 2 (clap parse failures, bad flag, bad input)
//! - `Auth` -> kind "auth", exit 77 (xurl auth failures)
//! - `Config` -> kind "config", exit 78 (missing config, invalid setup)
//! - `General` -> kind "general", exit 1 (command execution, network, API)

use crate::transport;

/// Structured error for the CLI. Each variant maps to a distinct exit code and
/// envelope `kind`. Constructors carry a stable kebab-case `error_id` plus a
/// human message.
pub enum BirdError {
    /// Usage / argument-parse error (exit 2).
    Usage {
        error_id: &'static str,
        message: String,
    },
    /// Authentication error (exit 77).
    Auth {
        error_id: &'static str,
        message: String,
    },
    /// Configuration error (exit 78).
    Config {
        error_id: &'static str,
        message: String,
    },
    /// Command execution error: API, network, I/O (exit 1).
    General {
        error_id: &'static str,
        message: String,
        /// Optional command name; included in the JSON envelope when present.
        command: Option<&'static str>,
        /// Optional HTTP status from upstream API; included when > 0.
        status: Option<u16>,
    },
}

impl BirdError {
    /// Construct a `Config` variant with the default `config-error` id.
    pub fn config<E: std::fmt::Display>(err: E) -> Self {
        BirdError::Config {
            error_id: "config-error",
            message: err.to_string(),
        }
    }

    /// Construct a `Usage` variant.
    pub fn usage(error_id: &'static str, message: impl Into<String>) -> Self {
        BirdError::Usage {
            error_id,
            message: message.into(),
        }
    }

    /// Construct a `General` variant from a command name and source error.
    pub fn general(name: &'static str, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        let status = source
            .downcast_ref::<transport::XurlError>()
            .and_then(|x| match x {
                transport::XurlError::Api { status, .. } if *status > 0 => Some(*status),
                _ => None,
            });
        BirdError::General {
            error_id: "command-error",
            message: source.to_string(),
            command: Some(name),
            status,
        }
    }

    /// Map a boxed source error to either `Auth` (if XurlError::Auth) or `General`.
    /// Centralizes the auth-vs-command-error decision for dispatch closures.
    pub fn from_source(
        name: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        if let Some(xurl_err) = source.downcast_ref::<transport::XurlError>()
            && matches!(xurl_err, transport::XurlError::Auth(_))
        {
            return BirdError::Auth {
                error_id: "auth-error",
                message: source.to_string(),
            };
        }
        BirdError::general(name, source)
    }

    /// Exit code for this variant.
    pub fn exit_code(&self) -> u8 {
        match self {
            BirdError::Usage { .. } => 2,
            BirdError::Auth { .. } => 77,
            BirdError::Config { .. } => 78,
            BirdError::General { .. } => 1,
        }
    }

    /// Envelope `kind` string (one of "usage" | "auth" | "config" | "general").
    pub fn kind(&self) -> &'static str {
        match self {
            BirdError::Usage { .. } => "usage",
            BirdError::Auth { .. } => "auth",
            BirdError::Config { .. } => "config",
            BirdError::General { .. } => "general",
        }
    }

    /// Stable machine-readable error id (kebab-case).
    pub fn error_id(&self) -> &'static str {
        match self {
            BirdError::Usage { error_id, .. } => error_id,
            BirdError::Auth { error_id, .. } => error_id,
            BirdError::Config { error_id, .. } => error_id,
            BirdError::General { error_id, .. } => error_id,
        }
    }

    /// Human-readable message.
    pub fn message(&self) -> &str {
        match self {
            BirdError::Usage { message, .. } => message,
            BirdError::Auth { message, .. } => message,
            BirdError::Config { message, .. } => message,
            BirdError::General { message, .. } => message,
        }
    }

    /// Optional command name (only `General` variant currently carries one).
    pub fn command(&self) -> Option<&'static str> {
        match self {
            BirdError::General { command, .. } => *command,
            _ => None,
        }
    }

    /// Optional upstream HTTP status (only `General` variant currently carries one).
    pub fn status(&self) -> Option<u16> {
        match self {
            BirdError::General { status, .. } => *status,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_envelope_kinds() {
        assert_eq!(
            BirdError::usage("u", "m").exit_code(),
            2,
            "usage variant should exit 2"
        );
        let auth = BirdError::from_source("test", Box::new(transport::XurlError::Auth("x".into())));
        assert_eq!(auth.exit_code(), 77, "auth variant should exit 77");
        assert_eq!(
            BirdError::config("m").exit_code(),
            78,
            "config variant should exit 78"
        );
        assert_eq!(
            BirdError::general("test", "m".into()).exit_code(),
            1,
            "general variant should exit 1"
        );
    }

    #[test]
    fn kind_strings_match_envelope() {
        assert_eq!(BirdError::usage("u", "m").kind(), "usage");
        let auth = BirdError::from_source("test", Box::new(transport::XurlError::Auth("x".into())));
        assert_eq!(auth.kind(), "auth");
        assert_eq!(BirdError::config("m").kind(), "config");
        assert_eq!(BirdError::general("t", "m".into()).kind(), "general");
    }

    #[test]
    fn from_source_detects_xurl_auth() {
        let err: Box<dyn std::error::Error + Send + Sync> =
            Box::new(transport::XurlError::Auth("unauthorized".to_string()));
        let mapped = BirdError::from_source("test", err);
        assert_eq!(mapped.exit_code(), 77, "XurlError::Auth -> exit 77");
        assert_eq!(mapped.kind(), "auth");
    }

    #[test]
    fn from_source_other_xurl_maps_to_general() {
        let err: Box<dyn std::error::Error + Send + Sync> = Box::new(
            transport::XurlError::Process("connection failed".to_string()),
        );
        let mapped = BirdError::from_source("profile", err);
        assert_eq!(mapped.exit_code(), 1, "non-auth XurlError -> exit 1");
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.command(), Some("profile"));
    }

    #[test]
    fn bird_error_exit_codes() {
        assert_eq!(
            BirdError::config("test").exit_code(),
            78,
            "Config errors should exit 78"
        );
        let auth = BirdError::from_source("test", Box::new(transport::XurlError::Auth("x".into())));
        assert_eq!(auth.exit_code(), 77, "Auth errors should exit 77");
        assert_eq!(
            BirdError::general("test", "test".into()).exit_code(),
            1,
            "Command errors should exit 1"
        );
        assert_eq!(
            BirdError::usage("bad", "test").exit_code(),
            2,
            "Usage errors should exit 2"
        );
    }

    #[test]
    fn map_cmd_error_detects_auth() {
        let auth_err: Box<dyn std::error::Error + Send + Sync> =
            Box::new(transport::XurlError::Auth("unauthorized".to_string()));
        let mapped = BirdError::from_source("test", auth_err);
        assert_eq!(
            mapped.exit_code(),
            77,
            "XurlError::Auth should map to exit 77"
        );
    }

    #[test]
    fn map_cmd_error_preserves_command_for_non_auth() {
        let api_err: Box<dyn std::error::Error + Send + Sync> = Box::new(
            transport::XurlError::Process("connection failed".to_string()),
        );
        let mapped = BirdError::from_source("profile", api_err);
        assert_eq!(
            mapped.exit_code(),
            1,
            "Non-auth XurlError should map to exit 1"
        );
    }
}
