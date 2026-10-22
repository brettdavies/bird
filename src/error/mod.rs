//! Structured error type and envelope mapping for the bird CLI.
//!
//! Variants map to distinct kinds and exit codes per the anc envelope contract:
//! - `Usage` -> kind "usage", exit 2 (clap parse failures, bad flag, bad input)
//! - `Auth` -> kind "auth", exit 77 (missing or refused credentials)
//! - `Config` -> kind "config", exit 78 (missing config, invalid setup)
//! - `General` -> kind "general", exit 1 (command execution, network, API)

pub mod fatal;

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
        /// Optional override that bypasses the default `General → 1`
        /// exit-code rule. Carries xdk's `EXIT_RATE_LIMITED (3)`,
        /// `EXIT_NOT_FOUND (4)`, and `EXIT_NETWORK_ERROR (5)`.
        exit_code_override: Option<u8>,
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
        BirdError::General {
            error_id: "command-error",
            message: source.to_string(),
            command: Some(name),
            status: source.downcast_ref::<xdk::Error>().and_then(api_status),
            exit_code_override: None,
        }
    }

    /// A command whose X API call was dropped by SIGTERM or SIGINT: exit code
    /// 128 plus the signal number.
    pub fn cancelled(name: &'static str, cancelled: crate::shutdown::Cancelled) -> Self {
        BirdError::General {
            error_id: "cancelled",
            message: cancelled.to_string(),
            command: Some(name),
            status: None,
            exit_code_override: Some(cancelled.exit_code()),
        }
    }

    /// Map a boxed source error to either a structured bird envelope (when
    /// the source is an `xdk::Error`) or `General` otherwise.
    /// Centralizes the auth-vs-command-error decision for dispatch closures.
    pub fn from_source(
        name: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        if let Some(err) = source.downcast_ref::<xdk::Error>() {
            return BirdError::from_xdk_error(name, err);
        }
        BirdError::general(name, source)
    }

    /// Translate an `xdk::Error` into the bird envelope. xdk-rs owns the
    /// classification: the envelope's `error` id is `xdk::Error::kind` (the
    /// `reason` `xr` reports) and the exit code is `xdk::Error::exit_code`.
    /// bird only files the result under its envelope `kind`: 77 is `auth`,
    /// 2 is `usage`, and every other code is `general`.
    fn from_xdk_error(name: &'static str, err: &xdk::Error) -> Self {
        if let Some(cancelled) = crate::shutdown::Cancelled::find(err) {
            return BirdError::cancelled(name, cancelled);
        }
        let error_id = err.kind();
        let message = err.to_string();
        match err.exit_code() {
            xdk::error::EXIT_AUTH_REQUIRED => BirdError::Auth { error_id, message },
            xdk::error::EXIT_USAGE_ERROR => BirdError::Usage { error_id, message },
            code => BirdError::General {
                error_id,
                message,
                command: Some(name),
                status: api_status(err),
                exit_code_override: u8::try_from(code)
                    .ok()
                    .filter(|&c| i32::from(c) != xdk::error::EXIT_GENERAL_ERROR),
            },
        }
    }

    /// Exit code for this variant.
    pub fn exit_code(&self) -> u8 {
        match self {
            BirdError::Usage { .. } => 2,
            BirdError::Auth { .. } => 77,
            BirdError::Config { .. } => 78,
            BirdError::General {
                exit_code_override: Some(code),
                ..
            } => *code,
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

    /// Render this error to the process's real stderr in a plain-text format.
    ///
    /// Fatal-only chokepoint: called from the runner's bare-error path that
    /// fires before an `OutputConfig` and injected writer are available
    /// (e.g. when `ResolvedPaths::from_env` fails at startup). Every other
    /// error rendering goes through `OutputConfig::print_error` against an
    /// injected writer so tests can capture the output.
    pub fn print(&self) {
        let line = match self {
            BirdError::Usage { message, .. } => format!("usage error: {}", message),
            BirdError::Auth { message, .. } => format!("auth failed: {}", message),
            BirdError::Config { message, .. } => format!("config failed: {}", message),
            BirdError::General {
                command: Some(name),
                message,
                ..
            } => format!("{} failed: {}", name, message),
            BirdError::General {
                command: None,
                message,
                ..
            } => format!("error: {}", message),
        };
        crate::error::fatal::fatal_eprintln(&line);
    }
}

/// The upstream HTTP status an `xdk::Error::Api` carries.
fn api_status(err: &xdk::Error) -> Option<u16> {
    match err {
        xdk::Error::Api { status, .. } if *status > 0 => Some(*status),
        _ => None,
    }
}

// Plan 1 R19: compile-time guard that BirdError stays `Send + Sync`. Plan 2's
// writer-injection requires every public type that may cross thread/async
// boundaries to be `Send + Sync`; this assertion catches a future field that
// would break it (e.g. a non-`Send` source error reference).
const _: fn() = || {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<BirdError>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use xdk::error::AuthMismatch;

    fn boxed(err: xdk::Error) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(err)
    }

    fn mapped(err: xdk::Error) -> BirdError {
        BirdError::from_source("search", boxed(err))
    }

    #[test]
    fn a_cancelled_call_exits_128_plus_the_signal() {
        let term = mapped(
            crate::shutdown::Cancelled {
                signal: libc::SIGTERM,
            }
            .into(),
        );
        assert_eq!(term.error_id(), "cancelled");
        assert_eq!(term.kind(), "general");
        assert_eq!(term.exit_code(), 143);

        let int = mapped(
            crate::shutdown::Cancelled {
                signal: libc::SIGINT,
            }
            .into(),
        );
        assert_eq!(int.exit_code(), 130);
    }

    #[test]
    fn exit_codes_match_envelope_kinds() {
        assert_eq!(BirdError::usage("u", "m").exit_code(), 2);
        assert_eq!(BirdError::config("m").exit_code(), 78);
        assert_eq!(mapped(xdk::Error::Internal("boom".into())).exit_code(), 1);
        assert_eq!(mapped(xdk::Error::auth("x")).exit_code(), 77);
    }

    #[test]
    fn kind_strings_match_envelope() {
        assert_eq!(BirdError::usage("u", "m").kind(), "usage");
        assert_eq!(BirdError::config("m").kind(), "config");
        assert_eq!(mapped(xdk::Error::auth("x")).kind(), "auth");
        assert_eq!(
            mapped(xdk::Error::Internal("boom".into())).kind(),
            "general"
        );
    }

    /// Every xdk error takes xdk's own exit code and its `kind()` as the
    /// envelope id; bird adds only its envelope `kind` and, on `general`
    /// errors, the command name and upstream status.
    #[test]
    fn xdk_errors_keep_xdk_reason_and_exit_code() {
        let mismatch = AuthMismatch {
            endpoint: "/2/users/{id}/likes".into(),
            rendered_url: Some("/2/users/12345/likes".into()),
            method: "POST".into(),
            requested: Some("oauth1".into()),
            supported: vec!["oauth2".into()],
            available_in_app: None,
            app: Some("default".into()),
            other_apps_with_creds: None,
        };
        let cases: Vec<(xdk::Error, &str, u8, &str, Option<u16>)> = vec![
            (
                xdk::Error::auth("token expired"),
                "auth-required",
                77,
                "auth",
                None,
            ),
            (
                xdk::Error::token_store("read failed"),
                "token-store",
                77,
                "auth",
                None,
            ),
            (
                xdk::Error::api(401, "Unauthorized"),
                "auth-required",
                77,
                "auth",
                None,
            ),
            (mismatch.into(), "auth-method-mismatch", 2, "usage", None),
            (
                xdk::Error::api(429, "Too Many Requests"),
                "rate-limited",
                3,
                "general",
                Some(429),
            ),
            (
                xdk::Error::api(404, "Not Found"),
                "not-found",
                4,
                "general",
                Some(404),
            ),
            (
                xdk::Error::http("connection reset"),
                "network-error",
                5,
                "general",
                None,
            ),
            (xdk::Error::io("ECONNRESET"), "io", 5, "general", None),
            (
                xdk::Error::api(403, "Forbidden"),
                "forbidden",
                1,
                "general",
                Some(403),
            ),
            (
                xdk::Error::api(422, "Unprocessable"),
                "invalid-request",
                1,
                "general",
                Some(422),
            ),
            (
                xdk::Error::api(503, "Unavailable"),
                "server-error",
                1,
                "general",
                Some(503),
            ),
            (
                xdk::Error::validation("missing field"),
                "validation",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::Internal("invariant".into()),
                "internal",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::invalid_url("ftp://bad"),
                "invalid-url",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::InvalidPathParam {
                    name: "id".into(),
                    value: "/escape".into(),
                },
                "invalid-path-param",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::json("unexpected token"),
                "serialization",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::InvalidMethod("GLOMP".into()),
                "invalid-method",
                1,
                "general",
                None,
            ),
            (
                xdk::Error::ProcessingTimeout {
                    media_id: "1".into(),
                    waited: std::time::Duration::from_secs(60),
                },
                "processing-timeout",
                1,
                "general",
                None,
            ),
        ];
        for (err, error_id, exit_code, kind, status) in cases {
            let label = format!("{err:?}");
            let bird = mapped(err);
            assert_eq!(bird.error_id(), error_id, "{label}");
            assert_eq!(bird.exit_code(), exit_code, "{label}");
            assert_eq!(bird.kind(), kind, "{label}");
            assert_eq!(bird.status(), status, "{label}");
            if kind == "general" {
                assert_eq!(bird.command(), Some("search"), "{label}");
            }
        }
    }

    /// A transport failure carries no status; its message quotes the request
    /// URL, so a status-like number in it must not pick the exit code.
    #[test]
    fn xdk_transport_message_digits_do_not_pick_the_exit_code() {
        let bird = mapped(xdk::Error::http(
            "error sending request for url (https://api.x.com/2/users/404)",
        ));
        assert_eq!(bird.exit_code(), 5);
        assert_eq!(bird.status(), None);
    }

    /// A source that is not an `xdk::Error` is a general error with no status.
    #[test]
    fn non_xdk_source_maps_to_general_1() {
        let bird = BirdError::from_source("cache", "disk full".into());
        assert_eq!(bird.error_id(), "command-error");
        assert_eq!(bird.exit_code(), 1);
        assert_eq!(bird.status(), None);
    }
}
