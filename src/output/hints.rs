//! Recovery hints: the `next_step` an error envelope carries and the text
//! line beside it.
//!
//! One built value feeds both renderings, so the line a person reads and the
//! object an agent runs cannot name different commands. The choice follows
//! `xr`'s `crates/xurl-cli/src/cli/hints.rs`, with bird's own commands where
//! bird has one (`bird login`, `bird doctor`).

use xdk::error::Error;
use xdk::store::snapshot::StoreSnapshot;

use super::envelope::{Action, NextStep, Reason};
use crate::error::BirdError;

/// The registration invocation. bird has no registration command, so the
/// step names `xr`'s; `<secret-command>` is whatever prints the secret.
pub const REGISTER_APP_TEMPLATE: &str =
    "<secret-command> | xr auth apps add <name> --client-id <client-id> --client-secret-file -";

/// A recovery hint: prose for a human, a typed step for an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// Lines printed after the error line in text mode.
    pub text_lines: Vec<String>,
    /// The step folded into the envelope in structured modes.
    pub next_step: NextStep,
}

/// What the runner knows when it renders an error: the store it loaded, the
/// invocation, and the help page of the command that invocation names.
#[derive(Debug, Default)]
pub struct HintContext {
    /// The token store as loaded before dispatch; `None` before it opened.
    pub snapshot: Option<StoreSnapshot>,
    /// argv as the runner received it.
    pub invocation: Vec<String>,
    /// `bird <command> --help` for the command the invocation names.
    pub help: String,
    /// Whether the caller asked for structured output.
    pub structured: bool,
}

/// Chooses the hint for `error`, when a recovery step exists.
#[must_use]
pub fn for_error(error: &BirdError, ctx: &HintContext) -> Option<Hint> {
    if let Some(fix) = error.fix() {
        return Some(Hint {
            text_lines: vec![fix.to_string()],
            next_step: NextStep::fix_input(None),
        });
    }
    match error.reason() {
        Reason::ConfirmationRequired => return Some(confirm_hint(&ctx.invocation)),
        Reason::InvalidArgs | Reason::UnknownCommand => return Some(show_help_hint(&ctx.help)),
        Reason::CacheMiss => return Some(cache_miss_hint(&ctx.invocation)),
        _ => {}
    }
    let source = error.xdk_source()?;
    let snapshot = ctx.snapshot.as_ref();
    if carries_no_auth_method(source) {
        return snapshot.map(|s| choose_hint(s, &ctx.invocation, ctx.structured));
    }
    snapshot
        .and_then(|s| unloadable_store_hint(s, source))
        .or_else(|| enrollment_hint(source))
        .or_else(|| resume_wait_hint(source))
        .or_else(|| wait_and_retry_hint(source))
        .or_else(|| {
            snapshot.and_then(|s| mismatch_hint(s, source, &ctx.invocation, ctx.structured))
        })
        .or_else(|| general_hint(source, &ctx.help))
}

/// Whether `error` is a missing-credential failure a recovery hint answers.
fn action(error: &Error) -> Option<Action> {
    error.next_action().and_then(Action::from_xdk)
}

fn carries_no_auth_method(error: &Error) -> bool {
    matches!(
        error,
        Error::Auth { message, .. }
            if message == xdk::error::NO_AUTH_METHOD || message == xdk::error::NO_OAUTH2_TOKEN
    )
}

/// The sign-in command, in the form a non-TTY caller can run when
/// `headless`.
fn sign_in_command(app: Option<&str>, headless: bool) -> String {
    let mut command = if headless {
        "bird login --no-browser".to_string()
    } else {
        "bird login".to_string()
    };
    if let Some(name) = app {
        command = format!(
            "bird --app {} {}",
            shell_word(name),
            &command["bird ".len()..]
        );
    }
    command
}

/// Chooses the hint for a no-credentials failure.
///
/// ```text
/// load failed               -> inspect-store, through bird doctor
/// another app holds tokens  -> select-app, rerunning this invocation
/// target can sign in        -> sign-in
/// another app has a client  -> select-app, signing in against it
/// nothing anywhere          -> register-app, with a template
/// ```
#[must_use]
pub fn choose_hint(snapshot: &StoreSnapshot, invocation: &[String], headless: bool) -> Hint {
    let target = snapshot.active_app.as_str();

    if snapshot.load_failed() {
        return Hint {
            text_lines: vec![
                format!("The token store could not be read: {}", snapshot.store_path),
                "Inspect or move that file, then retry. Run: bird doctor".to_string(),
            ],
            next_step: NextStep::inspect_store("bird doctor".to_string()),
        };
    }

    if let Some(app) = snapshot
        .apps_with_tokens()
        .into_iter()
        .find(|name| name != target)
    {
        let command = rerun_with_app(invocation, &app);
        return Hint {
            text_lines: vec![format!("App {app:?} is already signed in. Run: {command}")],
            next_step: NextStep::select_app(command),
        };
    }

    let target_has_client_id = snapshot.env_client_id_present
        || snapshot.apps.get(target).is_some_and(|f| f.has_client_id);
    if target_has_client_id {
        let command = sign_in_command(None, headless);
        return Hint {
            text_lines: vec![format!(
                "Sign in first. Run: {}",
                sign_in_command(None, false)
            )],
            next_step: NextStep::sign_in(command),
        };
    }

    if let Some(app) = snapshot
        .apps_ready_to_sign_in()
        .into_iter()
        .find(|name| name != target)
    {
        let command = sign_in_command(Some(&app), headless);
        return Hint {
            text_lines: vec![format!(
                "App {app:?} has client credentials. Run: {command}"
            )],
            next_step: NextStep::select_app(command),
        };
    }

    Hint {
        text_lines: vec![format!(
            "Register an app first. Run: {REGISTER_APP_TEMPLATE}"
        )],
        next_step: NextStep::register_app(REGISTER_APP_TEMPLATE.to_string()),
    }
}

/// The hint for a store that failed to load. The load state decides, not
/// the error's wording.
fn unloadable_store_hint(snapshot: &StoreSnapshot, error: &Error) -> Option<Hint> {
    if !(matches!(error, Error::TokenStore { .. }) && snapshot.load_failed()) {
        return None;
    }
    Some(Hint {
        text_lines: vec!["Inspect the store file it names. Run: bird doctor".to_string()],
        next_step: NextStep::inspect_store("bird doctor".to_string()),
    })
}

/// The enrollment hint when X refused the app.
fn enrollment_hint(error: &Error) -> Option<Hint> {
    let (Error::Api { body, .. }, Some(Action::EnrollApp), Some(docs)) =
        (error, action(error), error.docs_url())
    else {
        return None;
    };
    let mut text_lines = Vec::with_capacity(2);
    match detail_line(body) {
        Some(detail) => text_lines.push(format!("X refused the app: {detail}")),
        None => text_lines.push("X refused the app.".to_string()),
    }
    text_lines.push(format!(
        "Move it to the Pay-per-use package and the Production environment. See: {docs}"
    ));
    Some(Hint {
        text_lines,
        next_step: NextStep::enroll_app(docs.to_string()),
    })
}

/// The resume hint when a media processing wait reached its deadline. bird
/// has no media command, so the step names `xr`'s.
fn resume_wait_hint(error: &Error) -> Option<Hint> {
    let (Error::ProcessingTimeout { media_id, waited }, Some(Action::ResumeWait)) =
        (error, action(error))
    else {
        return None;
    };
    let command = format!(
        "xr media status {media_id} --wait={}",
        waited.as_secs().saturating_mul(2)
    );
    Some(Hint {
        text_lines: vec![format!("Resume the wait. Run: {command}")],
        next_step: NextStep::resume_wait(command),
    })
}

/// The retry hint for a 429 that named its reset.
fn wait_and_retry_hint(error: &Error) -> Option<Hint> {
    let (
        Error::Api {
            reset_at: Some(reset_at),
            ..
        },
        Some(Action::WaitAndRetry),
        Some(docs),
    ) = (error, action(error), error.docs_url())
    else {
        return None;
    };
    let (retry_after_secs, retry_at) = retry_times(*reset_at);
    Some(Hint {
        text_lines: vec![format!(
            "Rate limited. Retry in {retry_after_secs} seconds, at {retry_at}. See: {docs}"
        )],
        next_step: NextStep::wait_and_retry(docs.to_string()),
    })
}

/// The hint for an auth-scheme mismatch the store can answer.
fn mismatch_hint(
    snapshot: &StoreSnapshot,
    error: &Error,
    invocation: &[String],
    headless: bool,
) -> Option<Hint> {
    let Error::AuthMethodMismatch(mismatch) = error else {
        return None;
    };
    match mismatch.shape() {
        xdk::error::MismatchShape::WrongApp { others } => {
            let app = others.first()?;
            let command = rerun_with_app(invocation, app);
            Some(Hint {
                text_lines: vec![format!("App {app:?} is already signed in. Run: {command}")],
                next_step: NextStep::select_app(command),
            })
        }
        xdk::error::MismatchShape::EmptyIntersection { available }
            if mismatch.supported.iter().any(|scheme| scheme == "oauth2")
                && !available.iter().any(|scheme| scheme == "oauth2") =>
        {
            Some(choose_hint(snapshot, invocation, headless))
        }
        _ => None,
    }
}

/// The hint for an argument the command refused: its help page.
#[must_use]
pub fn show_help_hint(help: &str) -> Hint {
    Hint {
        text_lines: vec![format!("Try '{help}'.")],
        next_step: NextStep::show_help(help.to_string()),
    }
}

/// The invocation as shell words, run as `bird`.
fn invocation_words(invocation: &[String]) -> Vec<String> {
    let mut words: Vec<String> = invocation.iter().map(|arg| shell_word(arg)).collect();
    if let Some(program) = words.first_mut() {
        *program = "bird".to_string();
    }
    words
}

/// The hint for a `--cache-only` miss: the same invocation without that
/// flag, which sends the request.
#[must_use]
pub fn cache_miss_hint(invocation: &[String]) -> Hint {
    let words = invocation_words(invocation);
    let flags_end = words.iter().position(|w| w == "--").unwrap_or(words.len());
    let template = words
        .iter()
        .enumerate()
        .filter(|(i, w)| *i >= flags_end || *w != "--cache-only")
        .map(|(_, w)| w.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Hint {
        text_lines: vec![format!(
            "Run it without --cache-only to send the request: {template}"
        )],
        next_step: NextStep {
            template: (!template.is_empty()).then_some(template),
            ..NextStep::fix_input(None)
        },
    }
}

/// The hint for a refused confirmation: the same invocation with `--force`.
#[must_use]
pub fn confirm_hint(invocation: &[String]) -> Hint {
    let mut words = invocation_words(invocation);
    words.push("--force".to_string());
    let template = words.join(" ");
    Hint {
        text_lines: vec![format!("Confirm by running: {template}")],
        next_step: NextStep::confirm(template),
    }
}

/// The hint for an error whose step the library names and no stored
/// credential decides.
fn general_hint(error: &Error, help: &str) -> Option<Hint> {
    if matches!(
        error,
        Error::InvalidMethod(_) | Error::InvalidUrl { .. } | Error::InvalidPathParam { .. }
    ) {
        return Some(show_help_hint(help));
    }
    let docs = error.docs_url().map(str::to_string);
    match action(error)? {
        Action::Retry => Some(Hint {
            text_lines: vec!["Nothing was wrong with the request. Send it again.".to_string()],
            next_step: NextStep::retry(docs),
        }),
        Action::FixInput => Some(Hint {
            text_lines: Vec::new(),
            next_step: NextStep::fix_input(docs),
        }),
        Action::ReportIssue => {
            let docs = docs?;
            Some(Hint {
                text_lines: vec![format!("This is a fault in the library. Report it: {docs}")],
                next_step: NextStep::report_issue(docs),
            })
        }
        _ => None,
    }
}

/// Pulls the `detail` string out of a JSON error body, when there is one.
fn detail_line(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("detail")?
        .as_str()
        .map(str::to_string)
}

/// Rebuilds this invocation to run against `app`: `--app NAME` after the
/// program, without the `--app` the caller passed. Nothing after `--` is a
/// flag, so it is kept as it was.
fn rerun_with_app(invocation: &[String], app: &str) -> String {
    let mut parts = vec!["bird".to_string(), "--app".to_string(), shell_word(app)];
    let mut rest = invocation.iter().skip(1);
    while let Some(arg) = rest.next() {
        if arg == "--" {
            parts.push(shell_word(arg));
            parts.extend(rest.by_ref().map(|arg| shell_word(arg)));
        } else if arg == "--app" {
            rest.next();
        } else if !arg.starts_with("--app=") {
            parts.push(shell_word(arg));
        }
    }
    parts.join(" ")
}

/// Quotes one argument of a rebuilt invocation when a shell would split it.
fn shell_word(arg: &str) -> String {
    if !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '.' | '-' | '/' | ':' | '=' | '@' | '+' | '~')
        })
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// A rate limit's reset as the seconds left until it (zero once passed) and
/// the moment itself as an RFC 3339 UTC timestamp.
#[must_use]
pub fn retry_times(reset_at: u64) -> (u64, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let at = i64::try_from(reset_at)
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default();
    (reset_at.saturating_sub(now), at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_string()).collect()
    }

    #[test]
    fn confirm_template_appends_force_and_quotes_words() {
        let hint = confirm_hint(&argv(&["/usr/bin/bird", "dm", "jack", "hi there"]));
        assert_eq!(
            hint.next_step.template.as_deref(),
            Some("bird dm jack 'hi there' --force")
        );
        assert_eq!(hint.next_step.action, Action::Confirm);
        assert!(hint.next_step.command.is_none());
    }

    #[test]
    fn cache_miss_template_drops_only_the_flag_before_the_separator() {
        let hint = cache_miss_hint(&argv(&[
            "/usr/bin/bird",
            "--cache-only",
            "search",
            "from:jack",
            "--",
            "--cache-only",
        ]));
        assert_eq!(
            hint.next_step.template.as_deref(),
            Some("bird search from:jack -- --cache-only")
        );
        assert_eq!(hint.next_step.action, Action::FixInput);
        assert!(hint.next_step.command.is_none());
    }

    #[test]
    fn rerun_with_app_replaces_the_callers_app() {
        let command = rerun_with_app(
            &argv(&["bird", "--app", "old", "thread", "1", "--app=x"]),
            "work",
        );
        assert_eq!(command, "bird --app work thread 1");
    }

    #[test]
    fn retry_times_formats_rfc3339_utc() {
        let (_, at) = retry_times(1_700_000_000);
        assert_eq!(at, "2023-11-14T22:13:20Z");
    }

    #[test]
    fn sign_in_command_names_the_app_before_the_verb() {
        assert_eq!(
            sign_in_command(Some("work"), true),
            "bird --app work login --no-browser"
        );
        assert_eq!(sign_in_command(None, false), "bird login");
    }
}
