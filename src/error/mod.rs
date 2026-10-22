//! Structured error type for the bird CLI.
//!
//! A `BirdError` carries the envelope `reason` and exit code `xr` reports for
//! the same failure. An X API or library failure takes both from xdk-rs
//! (`xdk::Error::kind`, `xdk::Error::exit_code`); bird's own refusals use
//! `xr`'s spelling of the same refusal. Exit codes: 2 for an argument error,
//! 77 for a missing or refused credential, 78 for bird's own config, 3, 4,
//! and 5 for a rate limit, a 404, and no response, and 1 otherwise.

pub mod fatal;

use crate::output::envelope::{ErrorBody, Reason};

/// Which family a failure belongs to; decides the text-mode prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Usage,
    Auth,
    Config,
    General,
}

/// The `--cache-only` miss: the local store holds no entry for the request.
#[derive(Debug)]
pub struct CacheMiss;

impl std::fmt::Display for CacheMiss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("entity not in local store; run without --cache-only to fetch")
    }
}

impl std::error::Error for CacheMiss {}

/// Structured error for the CLI.
#[derive(Debug)]
pub struct BirdError {
    class: Class,
    reason: Reason,
    message: String,
    exit_code: u8,
    /// Subcommand name, for the text-mode `<name> failed:` prefix.
    label: Option<&'static str>,
    /// The offending value: the unknown word, or the command that needs
    /// confirmation.
    command: Option<String>,
    /// The nearest real command, for an unknown one.
    suggestion: Option<String>,
    /// The library error this came from, when it came from one.
    source: Option<Box<xdk::Error>>,
}

impl BirdError {
    fn new(class: Class, reason: Reason, exit_code: u8, message: String) -> Self {
        Self {
            class,
            reason,
            message,
            exit_code,
            label: None,
            command: None,
            suggestion: None,
            source: None,
        }
    }

    /// bird's own config file or paths could not be used (exit 78).
    pub fn config<E: std::fmt::Display>(err: E) -> Self {
        Self::new(Class::Config, Reason::ConfigError, 78, err.to_string())
    }

    /// An argument or input refusal at exit 2.
    pub fn usage(reason: Reason, message: impl Into<String>) -> Self {
        Self::new(Class::Usage, reason, 2, message.into())
    }

    /// The first word names no command.
    pub fn unknown_command(
        word: impl Into<String>,
        suggestion: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            command: Some(word.into()),
            suggestion,
            ..Self::usage(Reason::UnknownCommand, message)
        }
    }

    /// A confirmable command ran without `--force` and could not prompt
    /// (exit 1, as `xr` reports it).
    pub fn confirmation_required(verb: &str) -> Self {
        Self {
            command: Some(verb.to_string()),
            ..Self::new(
                Class::General,
                Reason::ConfirmationRequired,
                1,
                format!("confirmation required for {verb}; pass --force or run interactively"),
            )
        }
    }

    /// A command whose X API call was dropped by SIGTERM or SIGINT: exit code
    /// 128 plus the signal number.
    pub fn cancelled(name: &'static str, cancelled: crate::shutdown::Cancelled) -> Self {
        Self {
            label: Some(name),
            ..Self::new(
                Class::General,
                Reason::Cancelled,
                cancelled.exit_code(),
                cancelled.to_string(),
            )
        }
    }

    /// A command failure from any source; same as [`Self::from_source`].
    pub fn general(name: &'static str, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::from_source(name, source)
    }

    /// Files a boxed source error under its reason. An `xdk::Error` keeps
    /// xdk-rs's kind and exit code. A source bird recognizes takes the reason
    /// `xr` gives the same failure, and a plain message is a refused value.
    pub fn from_source(
        name: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        if let Some(cancelled) = crate::shutdown::Cancelled::find(source.as_ref()) {
            return Self::cancelled(name, cancelled);
        }
        let source = match source.downcast::<xdk::Error>() {
            Ok(err) => return Self::from_xdk_error(name, err),
            Err(other) => other,
        };
        let message = source.to_string();
        let (reason, exit_code) = if source.is::<CacheMiss>() {
            (Reason::CacheMiss, 1)
        } else if source.is::<std::io::Error>() {
            (Reason::Io, 5)
        } else if source.is::<serde_json::Error>() {
            (Reason::Serialization, 1)
        } else if source.is::<rusqlite::Error>() {
            (Reason::Internal, 1)
        } else {
            (Reason::Validation, 1)
        };
        Self {
            label: Some(name),
            ..Self::new(Class::General, reason, exit_code, message)
        }
    }

    /// Translates an `xdk::Error`: its `kind` is the reason and its exit code
    /// is the exit code.
    fn from_xdk_error(name: &'static str, err: Box<xdk::Error>) -> Self {
        let reason = Reason::from_kind(err.kind()).unwrap_or(Reason::Internal);
        let exit_code = u8::try_from(err.exit_code()).unwrap_or(1);
        let class = match err.exit_code() {
            xdk::error::EXIT_AUTH_REQUIRED => Class::Auth,
            xdk::error::EXIT_USAGE_ERROR => Class::Usage,
            _ => Class::General,
        };
        let message = err.to_string();
        Self {
            label: Some(name),
            source: Some(err),
            ..Self::new(class, reason, exit_code, message)
        }
    }

    /// Process exit code.
    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }

    /// Envelope `reason`.
    pub fn reason(&self) -> Reason {
        self.reason
    }

    /// Stable kebab-case reason string.
    pub fn error_id(&self) -> &'static str {
        self.reason.as_str()
    }

    /// Human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The library error this came from, when it came from one.
    pub fn xdk_source(&self) -> Option<&xdk::Error> {
        self.source.as_deref()
    }

    /// The text-mode prefix for this error.
    pub fn text_prefix(&self) -> String {
        match (self.class, self.label) {
            (Class::Usage, _) => "usage error: ".to_string(),
            (Class::Auth, _) => "auth failed: ".to_string(),
            (Class::Config, _) => "config failed: ".to_string(),
            (Class::General, Some(name)) => format!("{name} failed: "),
            (Class::General, None) => "error: ".to_string(),
        }
    }

    /// The envelope body for this error, without a `next_step`; the caller
    /// adds the step it chose. `message` is the sanitized message.
    pub fn body(&self, message: String) -> ErrorBody {
        let mut body = ErrorBody::new(self.reason, i32::from(self.exit_code), Some(message));
        body.command = self.command.clone();
        body.suggestion = self.suggestion.clone();
        match self.source.as_deref() {
            Some(xdk::Error::AuthMethodMismatch(mismatch)) => {
                body.endpoint = Some(mismatch.endpoint.clone());
                body.rendered_url = mismatch.rendered_url.clone();
                body.method = Some(mismatch.method.clone());
                body.requested = Some(match &mismatch.requested {
                    Some(s) => serde_json::Value::String(s.clone()),
                    None => serde_json::Value::Null,
                });
                body.supported = Some(mismatch.supported.clone());
                body.available_in_app = mismatch.available_in_app.clone();
                body.app = mismatch.app.clone();
                body.other_apps_with_creds = mismatch.other_apps_with_creds.clone();
            }
            Some(xdk::Error::Api {
                status: 429,
                reset_at: Some(reset_at),
                ..
            }) => {
                let (secs, at) = crate::output::hints::retry_times(*reset_at);
                body.retry_after_secs = Some(secs);
                body.retry_at = Some(at);
            }
            _ => {}
        }
        body
    }

    /// Render this error to the process's real stderr in a plain-text format.
    ///
    /// Fatal-only chokepoint: called from the runner's bare-error path that
    /// fires before an `OutputConfig` and injected writer are available
    /// (e.g. when `ResolvedPaths::from_env` fails at startup). Every other
    /// error rendering goes through `OutputConfig::print_error` against an
    /// injected writer so tests can capture the output.
    pub fn print(&self) {
        let line = format!("{}{}", self.text_prefix(), self.message);
        crate::error::fatal::fatal_eprintln(&line);
    }
}

// Compile-time guard that BirdError stays `Send + Sync`: every type that may
// cross the writer-injection boundary has to be.
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

    fn mismatch() -> AuthMismatch {
        AuthMismatch {
            endpoint: "/2/users/{id}/likes".into(),
            rendered_url: Some("/2/users/12345/likes".into()),
            method: "POST".into(),
            requested: Some("oauth1".into()),
            supported: vec!["oauth2".into()],
            available_in_app: None,
            app: Some("default".into()),
            other_apps_with_creds: None,
        }
    }

    #[test]
    fn bird_own_errors_take_xr_reasons_and_exit_codes() {
        assert_eq!(BirdError::config("m").error_id(), "config-error");
        assert_eq!(BirdError::config("m").exit_code(), 78);
        let usage = BirdError::usage(Reason::InvalidArgs, "m");
        assert_eq!((usage.error_id(), usage.exit_code()), ("invalid-args", 2));
        let confirm = BirdError::confirmation_required("like");
        assert_eq!(
            (confirm.error_id(), confirm.exit_code()),
            ("confirmation-required", 1)
        );
        assert_eq!(confirm.body("m".into()).command.as_deref(), Some("like"));
        let unknown = BirdError::unknown_command("tweeet", Some("tweet".into()), "m");
        let body = unknown.body("m".into());
        assert_eq!(body.reason, Reason::UnknownCommand);
        assert_eq!(body.command.as_deref(), Some("tweeet"));
        assert_eq!(body.suggestion.as_deref(), Some("tweet"));
    }

    /// Every xdk error takes xdk's own exit code and its `kind()` as the
    /// reason; the kinds below are every one xdk-rs's `Error::kind` returns.
    #[test]
    fn xdk_errors_keep_xdk_reason_and_exit_code() {
        let cases: Vec<(xdk::Error, &str, u8)> = vec![
            (xdk::Error::auth("token expired"), "auth-required", 77),
            (xdk::Error::token_store("read failed"), "token-store", 77),
            (xdk::Error::api(401, "Unauthorized"), "auth-required", 77),
            (mismatch().into(), "auth-method-mismatch", 2),
            (xdk::Error::api(429, "Too Many Requests"), "rate-limited", 3),
            (xdk::Error::api(404, "Not Found"), "not-found", 4),
            (xdk::Error::http("connection reset"), "network-error", 5),
            (xdk::Error::io("ECONNRESET"), "io", 5),
            (xdk::Error::api(403, "Forbidden"), "forbidden", 1),
            (xdk::Error::api(422, "Unprocessable"), "invalid-request", 1),
            (xdk::Error::api(503, "Unavailable"), "server-error", 1),
            (xdk::Error::api(418, "Teapot"), "api-error", 1),
            (xdk::Error::validation("missing field"), "validation", 1),
            (xdk::Error::Internal("invariant".into()), "internal", 1),
            (xdk::Error::invalid_url("ftp://bad"), "invalid-url", 1),
            (
                xdk::Error::InvalidPathParam {
                    name: "id".into(),
                    value: "/escape".into(),
                },
                "invalid-path-param",
                1,
            ),
            (xdk::Error::json("unexpected token"), "serialization", 1),
            (
                xdk::Error::InvalidMethod("GLOMP".into()),
                "invalid-method",
                1,
            ),
            (
                xdk::Error::ProcessingTimeout {
                    media_id: "1".into(),
                    waited: std::time::Duration::from_secs(60),
                },
                "processing-timeout",
                1,
            ),
        ];
        for (err, reason, exit_code) in cases {
            let label = format!("{err:?}");
            assert_eq!(
                Reason::from_kind(err.kind()).map(Reason::as_str),
                Some(err.kind()),
                "bird's reason set lacks xdk kind {}",
                err.kind()
            );
            let bird = mapped(err);
            assert_eq!(bird.error_id(), reason, "{label}");
            assert_eq!(bird.exit_code(), exit_code, "{label}");
            assert!(bird.xdk_source().is_some(), "{label}");
        }
    }

    #[test]
    fn a_mismatch_carries_xrs_mismatch_fields() {
        let body = mapped(mismatch().into()).body("m".into());
        assert_eq!(body.endpoint.as_deref(), Some("/2/users/{id}/likes"));
        assert_eq!(body.method.as_deref(), Some("POST"));
        assert_eq!(body.requested, Some(serde_json::json!("oauth1")));
        assert_eq!(body.supported, Some(vec!["oauth2".to_string()]));
        assert_eq!(body.app.as_deref(), Some("default"));
    }

    #[test]
    fn a_rate_limit_with_a_reset_carries_retry_times() {
        let reset = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        let err = xdk::Error::Api {
            status: 429,
            body: String::new(),
            reset_at: Some(reset),
        };
        let body = mapped(err).body("m".into());
        assert!(body.retry_after_secs.is_some_and(|s| s <= 60));
        assert!(body.retry_at.is_some_and(|at| at.ends_with('Z')));

        let body = mapped(xdk::Error::api(429, "Too Many Requests")).body("m".into());
        assert!(body.retry_after_secs.is_none() && body.retry_at.is_none());
    }

    /// A transport failure carries no status; its message quotes the request
    /// URL, so a status-like number in it must not pick the exit code.
    #[test]
    fn xdk_transport_message_digits_do_not_pick_the_exit_code() {
        let bird = mapped(xdk::Error::http(
            "error sending request for url (https://api.x.com/2/users/404)",
        ));
        assert_eq!(bird.exit_code(), 5);
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
    fn non_xdk_sources_take_the_reason_of_the_failure() {
        let message = BirdError::from_source("cache", "post ID must be numeric".into());
        assert_eq!((message.error_id(), message.exit_code()), ("validation", 1));
        let miss = BirdError::from_source("thread", Box::new(CacheMiss));
        assert_eq!((miss.error_id(), miss.exit_code()), ("cache-miss", 1));
        let io = BirdError::from_source("cache", Box::new(std::io::Error::other("disk full")));
        assert_eq!((io.error_id(), io.exit_code()), ("io", 5));
        let json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let json = BirdError::from_source("thread", Box::new(json));
        assert_eq!((json.error_id(), json.exit_code()), ("serialization", 1));
    }
}
