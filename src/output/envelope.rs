//! The JSON envelope every structured result takes, in `xr`'s shape.
//!
//! Field names, order, and serde attributes follow `xr`'s
//! `crates/xurl-cli/src/cli/envelope.rs` and `hints.rs`, so an agent that
//! parses `xr` parses bird the same way. `Action` carries xdk-rs's `NextAction` spellings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What the caller should do next; `xr`'s closed set, spelled as xdk-rs
/// spells `NextAction`.
///
/// xdk-rs 0.2.0 on crates.io declares eight of these thirteen actions, so
/// bird declares the full set and converts xdk's by its wire spelling
/// ([`Action::from_xdk`]); an action xdk-rs adds later converts the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// No app carries client credentials; register one.
    RegisterApp,
    /// The target app has credentials and no token; sign in.
    SignIn,
    /// Another app is the one to use; rerun naming it.
    SelectApp,
    /// The store could not be read or parsed; look at the file.
    InspectStore,
    /// X refused the app; enroll it in the developer portal.
    EnrollApp,
    /// The invocation was not one the tool takes; read the help it names.
    ShowHelp,
    /// Media was still processing when the wait's deadline passed.
    ResumeWait,
    /// X rate limited the request and said when the window resets.
    WaitAndRetry,
    /// Nothing is wrong with the request; send it again.
    Retry,
    /// The input, or the local state the command read, is what failed.
    FixInput,
    /// The tool or the library is at fault; report it.
    ReportIssue,
    /// The command acts on something and ran without its confirmation.
    Confirm,
    /// One other command repairs this; run it.
    RunCommand,
}

impl Action {
    /// The action xdk-rs named, by its wire spelling; `None` for one this set
    /// does not hold.
    #[must_use]
    pub fn from_xdk(action: xdk::error::NextAction) -> Option<Self> {
        serde_json::to_value(action)
            .ok()
            .and_then(|wire| serde_json::from_value(wire).ok())
    }
}

/// The three envelope variants, one per `status` discriminator value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Envelope {
    /// Success envelope; the payload's keys sit at the top level.
    Ok {
        #[serde(flatten)]
        payload: Value,
    },
    /// Dry-run envelope: what the command would have done, sending nothing.
    DryRun {
        /// True when every precondition checked clean.
        would_succeed: bool,
        /// Exit code the command would have returned on a real run.
        exit_code: i32,
        #[serde(flatten)]
        payload: Value,
    },
    /// Error envelope, written to stderr.
    Error(Box<ErrorBody>),
}

/// The closed set of `reason` values, spelled in kebab-case on the wire.
///
/// Every member `xr` also reports keeps `xr`'s spelling; a reason xdk-rs
/// classifies is spelled as `xdk::Error::kind()` returns it.
/// `config-error`, `cache-miss`, and `cancelled` are bird's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// No usable credential, or the API answered 401.
    AuthRequired,
    /// The endpoint does not accept the auth scheme the request would use.
    AuthMethodMismatch,
    /// The app holds no client credentials to sign in with.
    ClientCredentialsMissing,
    /// The API answered 429.
    RateLimited,
    /// The API answered 404.
    NotFound,
    /// The API answered 403.
    Forbidden,
    /// The API answered 400 or 422.
    InvalidRequest,
    /// The API answered with a 5xx status.
    ServerError,
    /// The API answered with any other error status.
    ApiError,
    /// The request got no response.
    NetworkError,
    /// The arguments did not parse.
    InvalidArgs,
    /// The first word names no command.
    UnknownCommand,
    /// The HTTP method is not one the library sends.
    InvalidMethod,
    /// The URL has a scheme other than `http` or `https`, or did not parse.
    InvalidUrl,
    /// A path parameter holds a character that would break the URL.
    InvalidPathParam,
    /// A value was refused without an HTTP error, before or after the request.
    Validation,
    /// A body could not be read or written as JSON.
    Serialization,
    /// A file or stream could not be read or written.
    Io,
    /// The token store could not be read or written.
    TokenStore,
    /// An invariant inside bird or the library did not hold.
    Internal,
    /// Media was still processing when the wait ended.
    ProcessingTimeout,
    /// A confirmable command ran under `--no-interactive` without `--force`.
    ConfirmationRequired,
    /// `bird schema` was given a name it does not know.
    UnknownSchema,
    /// `skill install` was given neither a host nor `--all`.
    MissingHost,
    /// bird's own config file or paths could not be used.
    ConfigError,
    /// `--cache-only` asked for an entry the local store does not hold.
    CacheMiss,
    /// SIGTERM or SIGINT dropped the X API call in flight.
    Cancelled,
}

impl Reason {
    /// The reason spelled as xdk-rs spells `kind`; `None` for a kind this
    /// set does not hold.
    #[must_use]
    pub fn from_kind(kind: &str) -> Option<Self> {
        let kind: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            serde::de::IntoDeserializer::into_deserializer(kind);
        Self::deserialize(kind).ok()
    }

    /// The kebab-case wire spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth-required",
            Self::AuthMethodMismatch => "auth-method-mismatch",
            Self::ClientCredentialsMissing => "client-credentials-missing",
            Self::RateLimited => "rate-limited",
            Self::NotFound => "not-found",
            Self::Forbidden => "forbidden",
            Self::InvalidRequest => "invalid-request",
            Self::ServerError => "server-error",
            Self::ApiError => "api-error",
            Self::NetworkError => "network-error",
            Self::InvalidArgs => "invalid-args",
            Self::UnknownCommand => "unknown-command",
            Self::InvalidMethod => "invalid-method",
            Self::InvalidUrl => "invalid-url",
            Self::InvalidPathParam => "invalid-path-param",
            Self::Validation => "validation",
            Self::Serialization => "serialization",
            Self::Io => "io",
            Self::TokenStore => "token-store",
            Self::Internal => "internal",
            Self::ProcessingTimeout => "processing-timeout",
            Self::ConfirmationRequired => "confirmation-required",
            Self::UnknownSchema => "unknown-schema",
            Self::MissingHost => "missing-host",
            Self::ConfigError => "config-error",
            Self::CacheMiss => "cache-miss",
            Self::Cancelled => "cancelled",
        }
    }
}

/// What the caller should do next, when a recovery step exists.
///
/// At most one of `command` and `template` is present: a command runs
/// verbatim from a non-TTY caller, a template holds placeholders the caller
/// fills or, under `confirm`, the invocation that acts once it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NextStep {
    /// The action to take.
    pub action: Action,
    /// A runnable invocation, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// A placeholder invocation, when the caller must supply values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Documentation URL, when a recipe exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
}

impl NextStep {
    fn new(action: Action) -> Self {
        Self {
            action,
            command: None,
            template: None,
            docs: None,
        }
    }

    /// Read the help `command` names.
    #[must_use]
    pub fn show_help(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::ShowHelp)
        }
    }

    /// Register an app with `xr`, filling the template's placeholders.
    #[must_use]
    pub fn register_app(template: String) -> Self {
        Self {
            template: Some(template),
            ..Self::new(Action::RegisterApp)
        }
    }

    /// Sign in by running `command`.
    #[must_use]
    pub fn sign_in(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::SignIn)
        }
    }

    /// Rerun against another app with `command`.
    #[must_use]
    pub fn select_app(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::SelectApp)
        }
    }

    /// Look at the store file through `command`.
    #[must_use]
    pub fn inspect_store(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::InspectStore)
        }
    }

    /// Enroll the app; the recipe is documentation, not a command.
    #[must_use]
    pub fn enroll_app(docs: String) -> Self {
        Self {
            docs: Some(docs),
            ..Self::new(Action::EnrollApp)
        }
    }

    /// Wait on media processing again with `command`.
    #[must_use]
    pub fn resume_wait(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::ResumeWait)
        }
    }

    /// Send the same request once the rate limit resets.
    #[must_use]
    pub fn wait_and_retry(docs: String) -> Self {
        Self {
            docs: Some(docs),
            ..Self::new(Action::WaitAndRetry)
        }
    }

    /// Send the same request again.
    #[must_use]
    pub fn retry(docs: Option<String>) -> Self {
        Self {
            docs,
            ..Self::new(Action::Retry)
        }
    }

    /// Change the input or local state before another try.
    #[must_use]
    pub fn fix_input(docs: Option<String>) -> Self {
        Self {
            docs,
            ..Self::new(Action::FixInput)
        }
    }

    /// Report a fault at `docs`.
    #[must_use]
    pub fn report_issue(docs: String) -> Self {
        Self {
            docs: Some(docs),
            ..Self::new(Action::ReportIssue)
        }
    }

    /// Run `template`, the invocation plus `--force`, once the caller decides.
    #[must_use]
    pub fn confirm(template: String) -> Self {
        Self {
            template: Some(template),
            ..Self::new(Action::Confirm)
        }
    }

    /// Run the one other command that repairs this.
    #[must_use]
    pub fn run_command(command: String) -> Self {
        Self {
            command: Some(command),
            ..Self::new(Action::RunCommand)
        }
    }

    /// The invocation to show a human: the command, else the template.
    #[must_use]
    pub fn display_invocation(&self) -> Option<&str> {
        self.command.as_deref().or(self.template.as_deref())
    }
}

/// Every field an error envelope can carry, beside the `status` tag.
///
/// Optional fields are skipped when absent, so a consumer feature-detects by
/// key presence.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorBody {
    /// Typed kebab-case kind; agents pattern-match on this. A newer release
    /// can add a reason, so treat one you do not recognize as your default
    /// branch.
    #[schemars(with = "String")]
    pub reason: Reason,
    /// Process exit code.
    pub exit_code: i32,
    /// Human-readable message, control characters sanitized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// What the caller should do next, when a recovery step exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<NextStep>,
    /// The offending value, echoed verbatim: the unknown word, or the
    /// command that needs confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The nearest real command, when one is close.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Endpoint template the request targeted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Endpoint path with path parameters substituted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_url: Option<String>,
    /// HTTP method the request would have used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Auth scheme the caller asked for; `null` when auto-detect ran.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub requested: Option<Value>,
    /// Auth schemes the endpoint accepts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported: Option<Vec<String>>,
    /// Auth schemes available for the active app.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_in_app: Option<Vec<String>>,
    /// The active app name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Other apps holding credentials, when the active one holds none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_apps_with_creds: Option<Vec<String>>,
    /// Seconds until the rate limit resets, zero once it has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
    /// The same reset as an RFC 3339 UTC timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<String>,
}

impl ErrorBody {
    /// A body carrying only its reason, exit code, and message.
    #[must_use]
    pub fn new(reason: Reason, exit_code: i32, message: Option<String>) -> Self {
        Self {
            reason,
            exit_code,
            message,
            next_step: None,
            command: None,
            suggestion: None,
            endpoint: None,
            rendered_url: None,
            method: None,
            requested: None,
            supported: None,
            available_in_app: None,
            app: None,
            other_apps_with_creds: None,
            retry_after_secs: None,
            retry_at: None,
        }
    }

    /// Serializes this body into the error envelope agents receive.
    #[must_use]
    pub fn into_value(self) -> Value {
        serde_json::to_value(Envelope::Error(Box::new(self)))
            .unwrap_or_else(|_| serde_json::json!({"status": "error"}))
    }
}

/// Reads a key that is present as `Some`, `null` included.
fn present<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

/// The JSON Schema (draft 2020-12) of every envelope bird prints, as
/// committed in `schema/envelope.schema.json` and printed by
/// `bird schema envelope`.
#[must_use]
pub fn envelope_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Envelope))
        .unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = schema.as_object_mut() {
        obj.insert("title".into(), Value::String("bird envelope".into()));
        obj.insert(
            "description".into(),
            Value::String(
                "Every structured document bird assembles: `ok` and `dry_run` on stdout, `error` on stderr. Agents branch on `status`.".into(),
            ),
        );
    }
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_spelling_round_trips_through_serde() {
        let all = [
            Reason::AuthRequired,
            Reason::AuthMethodMismatch,
            Reason::ClientCredentialsMissing,
            Reason::RateLimited,
            Reason::NotFound,
            Reason::Forbidden,
            Reason::InvalidRequest,
            Reason::ServerError,
            Reason::ApiError,
            Reason::NetworkError,
            Reason::InvalidArgs,
            Reason::UnknownCommand,
            Reason::InvalidMethod,
            Reason::InvalidUrl,
            Reason::InvalidPathParam,
            Reason::Validation,
            Reason::Serialization,
            Reason::Io,
            Reason::TokenStore,
            Reason::Internal,
            Reason::ProcessingTimeout,
            Reason::ConfirmationRequired,
            Reason::UnknownSchema,
            Reason::MissingHost,
            Reason::ConfigError,
            Reason::CacheMiss,
            Reason::Cancelled,
        ];
        for reason in all {
            let wire = serde_json::to_value(reason).unwrap();
            assert_eq!(wire, reason.as_str(), "{reason:?}");
            assert_eq!(Reason::from_kind(reason.as_str()), Some(reason));
        }
    }

    #[test]
    fn xdk_actions_convert_by_wire_spelling() {
        use xdk::error::NextAction;
        let pairs = [
            (NextAction::RegisterApp, Action::RegisterApp),
            (NextAction::SignIn, Action::SignIn),
            (NextAction::SelectApp, Action::SelectApp),
            (NextAction::InspectStore, Action::InspectStore),
            (NextAction::EnrollApp, Action::EnrollApp),
            (NextAction::ShowHelp, Action::ShowHelp),
            (NextAction::ResumeWait, Action::ResumeWait),
            (NextAction::WaitAndRetry, Action::WaitAndRetry),
        ];
        for (xdk_action, bird_action) in pairs {
            assert_eq!(Action::from_xdk(xdk_action), Some(bird_action));
        }
    }

    #[test]
    fn error_body_rejects_an_undeclared_key() {
        let doc = serde_json::json!({"reason": "internal", "exit_code": 1, "kind": "general"});
        assert!(serde_json::from_value::<ErrorBody>(doc).is_err());
    }

    #[test]
    fn error_envelope_carries_the_status_tag_first_class() {
        let value = ErrorBody::new(Reason::Internal, 1, Some("m".into())).into_value();
        assert_eq!(value["status"], "error");
        assert_eq!(value["reason"], "internal");
        assert_eq!(value["exit_code"], 1);
        assert!(value.get("next_step").is_none());
    }

    #[test]
    fn committed_envelope_schema_matches_the_types() {
        let generated = serde_json::to_string_pretty(&envelope_schema()).unwrap() + "\n";
        let committed = include_str!("../../schema/envelope.schema.json");
        assert_eq!(
            committed, generated,
            "schema/envelope.schema.json is stale; regenerate it with \
             `cargo test --lib bless_envelope_schema -- --ignored`"
        );
    }

    #[test]
    #[ignore = "writes schema/envelope.schema.json; run on purpose"]
    fn bless_envelope_schema() {
        let generated = serde_json::to_string_pretty(&envelope_schema()).unwrap() + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("schema")
            .join("envelope.schema.json");
        std::fs::write(path, generated).unwrap();
    }
}
