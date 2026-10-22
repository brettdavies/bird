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
        /// `EXIT_NOT_FOUND (4)`, and `EXIT_NETWORK_ERROR (5)` (KTD-4).
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

    /// Translate an `xdk::Error` into the bird envelope per KTD-4.
    ///
    /// bird overrides two of xdk's classifications: `Validation` is a config
    /// error (78, where xdk exits 1), and `AuthMethodMismatch` is an auth
    /// error (77, where xdk's 2 collides with clap's usage exit). Every other
    /// error takes xdk's own `exit_code()`: 77 for a missing or refused
    /// credential, 3 for a rate limit, 4 for not found, 5 for a transport or
    /// I/O failure, 1 otherwise. `xdk::Error` is `#[non_exhaustive]`, so a
    /// variant xdk adds arrives already classified.
    fn from_xdk_error(name: &'static str, err: &xdk::Error) -> Self {
        let message = err.to_string();
        match err {
            xdk::Error::Validation(_) => BirdError::Config {
                error_id: "config-error",
                message,
            },
            xdk::Error::AuthMethodMismatch(_) => BirdError::Auth {
                error_id: "auth-method-mismatch",
                message,
            },
            _ => match err.exit_code() {
                xdk::error::EXIT_AUTH_REQUIRED => BirdError::Auth {
                    error_id: "auth-error",
                    message,
                },
                code => BirdError::General {
                    error_id: "command-error",
                    message,
                    command: Some(name),
                    status: api_status(err),
                    exit_code_override: u8::try_from(code)
                        .ok()
                        .filter(|&c| i32::from(c) != xdk::error::EXIT_GENERAL_ERROR),
                },
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

    #[test]
    fn exit_codes_match_envelope_kinds() {
        assert_eq!(BirdError::usage("u", "m").exit_code(), 2);
        assert_eq!(BirdError::config("m").exit_code(), 78);
        let general = BirdError::from_source("test", boxed(xdk::Error::Internal("boom".into())));
        assert_eq!(general.exit_code(), 1);
        let auth = BirdError::from_source("test", boxed(xdk::Error::auth("x")));
        assert_eq!(auth.exit_code(), 77);
    }

    #[test]
    fn kind_strings_match_envelope() {
        assert_eq!(BirdError::usage("u", "m").kind(), "usage");
        assert_eq!(BirdError::config("m").kind(), "config");
        let auth = BirdError::from_source("test", boxed(xdk::Error::auth("x")));
        assert_eq!(auth.kind(), "auth");
        let general = BirdError::from_source("test", boxed(xdk::Error::Internal("boom".into())));
        assert_eq!(general.kind(), "general");
    }

    /// Validation overrides xdk's exit 1 → bird's 78 (config error).
    #[test]
    fn xdk_validation_maps_to_config_78() {
        let mapped = BirdError::from_source("raw", boxed(xdk::Error::validation("missing field")));
        assert_eq!(mapped.exit_code(), 78);
        assert_eq!(mapped.kind(), "config");
    }

    #[test]
    fn xdk_auth_maps_to_auth_77() {
        let mapped = BirdError::from_source("me", boxed(xdk::Error::auth("token expired")));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
        assert_eq!(mapped.error_id(), "auth-error");
    }

    #[test]
    fn xdk_token_store_maps_to_auth_77() {
        let mapped = BirdError::from_source("me", boxed(xdk::Error::token_store("read failed")));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
    }

    /// AuthMethodMismatch overrides xdk's 2 → bird's 77.
    #[test]
    fn xdk_auth_method_mismatch_maps_to_auth_77_not_2() {
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
        let mapped = BirdError::from_source("post", boxed(mismatch.into()));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
        assert_eq!(mapped.error_id(), "auth-method-mismatch");
    }

    #[test]
    fn xdk_api_401_maps_to_auth_77() {
        let mapped = BirdError::from_source("me", boxed(xdk::Error::api(401, "Unauthorized")));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
    }

    /// 429 inherits xdk's `EXIT_RATE_LIMITED = 3` via `exit_code_override`.
    /// The contract is the load-bearing signal agents key on for back-off.
    #[test]
    fn xdk_api_429_maps_to_general_3() {
        let mapped = BirdError::from_source(
            "bookmarks",
            boxed(xdk::Error::api(429, "Too Many Requests")),
        );
        assert_eq!(mapped.exit_code(), 3);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(429));
    }

    /// 404 inherits xdk's `EXIT_NOT_FOUND = 4`.
    #[test]
    fn xdk_api_404_maps_to_general_4() {
        let mapped = BirdError::from_source("profile", boxed(xdk::Error::api(404, "Not Found")));
        assert_eq!(mapped.exit_code(), 4);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(404));
    }

    /// Transport and filesystem failures inherit xdk's `EXIT_NETWORK_ERROR = 5`.
    #[test]
    fn xdk_transport_and_io_map_to_general_5() {
        for err in [
            xdk::Error::http("connection reset"),
            xdk::Error::io("ECONNRESET"),
        ] {
            let label = format!("{err:?}");
            let mapped = BirdError::from_source("search", boxed(err));
            assert_eq!(mapped.exit_code(), 5, "{label} must map to exit 5");
            assert_eq!(mapped.kind(), "general");
            assert_eq!(mapped.status(), None);
        }
    }

    /// A transport failure carries no status; its message quotes the request
    /// URL, so a status-like number in it must not pick the exit code.
    #[test]
    fn xdk_transport_message_digits_do_not_pick_the_exit_code() {
        let mapped = BirdError::from_source(
            "profile",
            boxed(xdk::Error::http(
                "error sending request for url (https://api.x.com/2/users/404)",
            )),
        );
        assert_eq!(mapped.exit_code(), 5);
        assert_eq!(mapped.status(), None);
    }

    /// Other API status codes (e.g. 422, 500) inherit xdk's
    /// `EXIT_GENERAL_ERROR = 1` and preserve the status field on the envelope.
    #[test]
    fn xdk_api_other_status_maps_to_general_1() {
        let mapped =
            BirdError::from_source("post", boxed(xdk::Error::api(422, "Unprocessable Entity")));
        assert_eq!(mapped.exit_code(), 1);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(422));
    }

    /// `Internal`, `InvalidUrl`, `InvalidPathParam`, `Json`, `InvalidMethod`,
    /// and `ProcessingTimeout` all surface as `EXIT_GENERAL_ERROR = 1` with
    /// the command name preserved.
    #[test]
    fn xdk_structural_errors_map_to_general_1() {
        let cases = vec![
            xdk::Error::Internal("invariant violated".into()),
            xdk::Error::invalid_url("ftp://bad"),
            xdk::Error::InvalidPathParam {
                name: "id".into(),
                value: "/escape".into(),
            },
            xdk::Error::json("unexpected token"),
            xdk::Error::InvalidMethod("GLOMP".into()),
            xdk::Error::ProcessingTimeout {
                media_id: "1".into(),
                waited: std::time::Duration::from_secs(60),
            },
        ];
        for err in cases {
            let label = format!("{err:?}");
            let mapped = BirdError::from_source("raw", boxed(err));
            assert_eq!(mapped.exit_code(), 1, "{label} must map to exit 1");
            assert_eq!(mapped.kind(), "general", "{label} must surface as general");
            assert_eq!(mapped.command(), Some("raw"));
        }
    }

    /// A source that is not an `xdk::Error` is a general error with no status.
    #[test]
    fn non_xdk_source_maps_to_general_1() {
        let mapped = BirdError::from_source("cache", "disk full".into());
        assert_eq!(mapped.exit_code(), 1);
        assert_eq!(mapped.status(), None);
    }
}
