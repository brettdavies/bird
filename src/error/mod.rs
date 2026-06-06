//! Structured error type and envelope mapping for the bird CLI.
//!
//! Variants map to distinct kinds and exit codes per the anc envelope contract:
//! - `Usage` -> kind "usage", exit 2 (clap parse failures, bad flag, bad input)
//! - `Auth` -> kind "auth", exit 77 (xurl auth failures)
//! - `Config` -> kind "config", exit 78 (missing config, invalid setup)
//! - `General` -> kind "general", exit 1 (command execution, network, API)

pub mod fatal;

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
        /// Optional override that bypasses the default `General → 1`
        /// exit-code rule. Embedded-xurl uses it to inherit xurl's
        /// `EXIT_RATE_LIMITED (3)`, `EXIT_NOT_FOUND (4)`, and
        /// `EXIT_NETWORK_ERROR (5)` mappings (KTD-4). Subprocess paths
        /// leave it `None` so existing behavior is preserved.
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
            exit_code_override: None,
        }
    }

    /// Map a boxed source error to either `Auth` (if XurlError::Auth) or `General`.
    /// Centralizes the auth-vs-command-error decision for dispatch closures.
    ///
    /// Under `embedded-xurl`, the embedded transport surfaces
    /// `xurl::error::XurlError`; we run that through the exhaustive
    /// `From<XurlError> for BirdError` mapping (KTD-4) before falling back
    /// to the existing subprocess `transport::XurlError` downcast. The
    /// embedded path is tried first so feature-on builds prefer xurl-rs's
    /// structured errors over the subprocess classifier (which never fires
    /// when the embedded path is the one producing the error in the first
    /// place).
    pub fn from_source(
        name: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        #[cfg(feature = "embedded-xurl")]
        if let Some(xurl_err) = source.downcast_ref::<xurl::error::XurlError>() {
            return BirdError::from_xurl_error(name, xurl_err);
        }
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

    /// Translate a `xurl::error::XurlError` into the bird envelope per KTD-4.
    /// Bird inherits xurl's `exit_code_for_error` for codes 3 / 4 / 5; overrides
    /// `Validation → 78` (config) and `AuthMethodMismatch → 77` (avoiding clap's
    /// `EX_USAGE = 2` collision); maps everything else through the variant the
    /// envelope already documents.
    ///
    /// The match is exhaustive: rustc fails the build when xurl-rs ships a new
    /// `XurlError` variant, forcing the consumer to decide where it lands
    /// before the upgrade ships.
    #[cfg(feature = "embedded-xurl")]
    fn from_xurl_error(name: &'static str, err: &xurl::error::XurlError) -> Self {
        use xurl::error::XurlError;

        match err {
            // Validation overrides xurl's exit 1 → bird's 78 (config).
            XurlError::Validation(_) => BirdError::Config {
                error_id: "config-error",
                message: err.to_string(),
            },

            // Auth + tokenstore + 401 → 77.
            XurlError::Auth(_) | XurlError::TokenStore(_) => BirdError::Auth {
                error_id: "auth-error",
                message: err.to_string(),
            },
            XurlError::Api { status: 401, .. } => BirdError::Auth {
                error_id: "auth-error",
                message: err.to_string(),
            },
            // AuthMethodMismatch overrides xurl's 2 → bird's 77 to avoid the
            // clap usage-error code collision.
            XurlError::AuthMethodMismatch { .. } => BirdError::Auth {
                error_id: "auth-method-mismatch",
                message: err.to_string(),
            },

            // Api rate-limit / not-found / network — inherit xurl's 3 / 4 / 5.
            XurlError::Api { status: 429, .. } => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: Some(429),
                exit_code_override: Some(3),
            },
            XurlError::Api { status: 404, .. } => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: Some(404),
                exit_code_override: Some(4),
            },
            XurlError::Io(_) => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: None,
                exit_code_override: Some(5),
            },

            // Http (string-typed transport errors) — pattern-match on the
            // substring xurl itself scans for so bird picks up the same
            // inferred codes.
            XurlError::Http(msg) if msg.contains("401") || msg.contains("Unauthorized") => {
                BirdError::Auth {
                    error_id: "auth-error",
                    message: err.to_string(),
                }
            }
            XurlError::Http(msg) if msg.contains("429") => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: None,
                exit_code_override: Some(3),
            },
            XurlError::Http(msg) if msg.contains("404") => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: None,
                exit_code_override: Some(4),
            },

            // EnvelopeAlreadyEmitted carries its own structured exit code
            // because the call site already printed the canonical envelope.
            // Honor that exit code; bird's renderer short-circuits on the
            // sentinel kind below.
            XurlError::EnvelopeAlreadyEmitted { exit_code } => BirdError::General {
                error_id: "envelope-already-emitted",
                message: String::new(),
                command: Some(name),
                status: None,
                exit_code_override: Some((*exit_code).clamp(0, i32::from(u8::MAX)) as u8),
            },

            // Other Api status codes + structural errors fall through to
            // exit 1 (general).
            XurlError::Api { status, .. } => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: Some(*status),
                exit_code_override: None,
            },
            XurlError::Http(_)
            | XurlError::InvalidMethod(_)
            | XurlError::InvalidUrl(_)
            | XurlError::InvalidPathParam { .. }
            | XurlError::Internal(_)
            | XurlError::Json(_) => BirdError::General {
                error_id: "command-error",
                message: err.to_string(),
                command: Some(name),
                status: None,
                exit_code_override: None,
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

#[cfg(all(test, feature = "embedded-xurl"))]
mod embedded_tests {
    use super::*;
    use xurl::error::XurlError;

    fn boxed(err: XurlError) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(err)
    }

    /// Validation overrides xurl's exit 1 → bird's 78 (config error). The
    /// override preserves bird's existing contract that config-shaped
    /// failures (missing/invalid env, bad URL template, etc.) surface
    /// distinct from generic command errors.
    #[test]
    fn xurl_validation_maps_to_config_78() {
        let mapped =
            BirdError::from_source("raw", boxed(XurlError::Validation("missing field".into())));
        assert_eq!(mapped.exit_code(), 78);
        assert_eq!(mapped.kind(), "config");
    }

    #[test]
    fn xurl_auth_maps_to_auth_77() {
        let mapped = BirdError::from_source("me", boxed(XurlError::Auth("token expired".into())));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
    }

    #[test]
    fn xurl_token_store_maps_to_auth_77() {
        let mapped =
            BirdError::from_source("me", boxed(XurlError::TokenStore("read failed".into())));
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
    }

    /// AuthMethodMismatch overrides xurl's 2 → bird's 77. Without the
    /// override the code would collide with clap's `EX_USAGE = 2` and
    /// agents that key on the exit code couldn't disambiguate.
    #[test]
    fn xurl_auth_method_mismatch_maps_to_auth_77_not_2() {
        let mapped = BirdError::from_source(
            "tweet",
            boxed(XurlError::AuthMethodMismatch {
                endpoint: "/2/users/{id}/likes".into(),
                rendered_url: Some("/2/users/12345/likes".into()),
                method: "POST".into(),
                requested: Some("oauth1".into()),
                supported: vec!["oauth2".into()],
                available_in_app: None,
                app: Some("default".into()),
                other_apps_with_creds: None,
            }),
        );
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
        assert_eq!(mapped.error_id(), "auth-method-mismatch");
    }

    #[test]
    fn xurl_api_401_maps_to_auth_77() {
        let mapped = BirdError::from_source(
            "me",
            boxed(XurlError::Api {
                status: 401,
                body: "Unauthorized".into(),
            }),
        );
        assert_eq!(mapped.exit_code(), 77);
        assert_eq!(mapped.kind(), "auth");
    }

    /// 429 inherits xurl's `EXIT_RATE_LIMITED = 3` via `exit_code_override`.
    /// The contract is the load-bearing signal agents key on for back-off.
    #[test]
    fn xurl_api_429_maps_to_general_3() {
        let mapped = BirdError::from_source(
            "bookmarks",
            boxed(XurlError::Api {
                status: 429,
                body: "Too Many Requests".into(),
            }),
        );
        assert_eq!(mapped.exit_code(), 3);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(429));
    }

    /// 404 inherits xurl's `EXIT_NOT_FOUND = 4`.
    #[test]
    fn xurl_api_404_maps_to_general_4() {
        let mapped = BirdError::from_source(
            "profile",
            boxed(XurlError::Api {
                status: 404,
                body: "Not Found".into(),
            }),
        );
        assert_eq!(mapped.exit_code(), 4);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(404));
    }

    /// `Io` inherits xurl's `EXIT_NETWORK_ERROR = 5` so transient network
    /// failures surface distinct from logic errors.
    #[test]
    fn xurl_io_maps_to_general_5() {
        let mapped = BirdError::from_source("search", boxed(XurlError::Io("ECONNRESET".into())));
        assert_eq!(mapped.exit_code(), 5);
        assert_eq!(mapped.kind(), "general");
    }

    /// Other API status codes (e.g. 422, 500) inherit xurl's
    /// `EXIT_GENERAL_ERROR = 1` and preserve the status field on the
    /// envelope so callers can still report the upstream HTTP code.
    #[test]
    fn xurl_api_other_status_maps_to_general_1() {
        let mapped = BirdError::from_source(
            "tweet",
            boxed(XurlError::Api {
                status: 422,
                body: "Unprocessable Entity".into(),
            }),
        );
        assert_eq!(mapped.exit_code(), 1);
        assert_eq!(mapped.kind(), "general");
        assert_eq!(mapped.status(), Some(422));
    }

    #[test]
    fn xurl_http_substring_401_maps_to_auth_77() {
        let mapped =
            BirdError::from_source("me", boxed(XurlError::Http("401 Unauthorized".into())));
        assert_eq!(mapped.exit_code(), 77);
    }

    #[test]
    fn xurl_http_substring_429_maps_to_general_3() {
        let mapped = BirdError::from_source("me", boxed(XurlError::Http("429 throttled".into())));
        assert_eq!(mapped.exit_code(), 3);
    }

    #[test]
    fn xurl_http_substring_404_maps_to_general_4() {
        let mapped = BirdError::from_source("me", boxed(XurlError::Http("404 missing".into())));
        assert_eq!(mapped.exit_code(), 4);
    }

    /// `Internal`, `InvalidUrl`, `InvalidPathParam`, `Json`,
    /// `InvalidMethod`, and unclassified `Http` all surface as
    /// `EXIT_GENERAL_ERROR = 1` with the command name preserved.
    #[test]
    fn xurl_structural_errors_map_to_general_1() {
        let cases: Vec<XurlError> = vec![
            XurlError::Internal("invariant violated".into()),
            XurlError::InvalidUrl("ftp://bad".into()),
            XurlError::InvalidPathParam {
                name: "id".into(),
                value: "/escape".into(),
            },
            XurlError::Json("unexpected token".into()),
            XurlError::InvalidMethod("GLOMP".into()),
            XurlError::Http("connection reset".into()),
        ];
        for err in cases {
            let label = format!("{err:?}");
            let mapped = BirdError::from_source("raw", boxed(err));
            assert_eq!(mapped.exit_code(), 1, "{label} must map to exit 1",);
            assert_eq!(mapped.kind(), "general", "{label} must surface as general");
        }
    }

    /// `EnvelopeAlreadyEmitted` carries its own exit code because the call
    /// site already printed the canonical envelope; bird honors it.
    #[test]
    fn xurl_envelope_already_emitted_passes_through_exit_code() {
        let mapped = BirdError::from_source(
            "raw",
            boxed(XurlError::EnvelopeAlreadyEmitted { exit_code: 7 }),
        );
        assert_eq!(mapped.exit_code(), 7);
    }
}
