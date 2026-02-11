//! Central command requirements: which auth types each command accepts, and human-readable hints.
//! Used by execution (resolve token), errors (format auth-required message), and doctor (availability/reasons).

/// Auth types that a command can accept. Matches OpenAPI spec (OAuth2UserToken, UserToken, BearerToken).
/// The None variant indicates no authentication is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AuthType {
    #[serde(rename = "oauth2_user")]
    OAuth2User,
    #[serde(rename = "oauth1")]
    OAuth1,
    #[serde(rename = "bearer")]
    Bearer,
    #[serde(rename = "none")]
    None,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::OAuth2User => write!(f, "oauth2_user"),
            AuthType::OAuth1 => write!(f, "oauth1"),
            AuthType::Bearer => write!(f, "bearer"),
            AuthType::None => write!(f, "none"),
        }
    }
}

/// Per-command auth requirements: which auth types are accepted and hint strings for errors/doctor.
#[derive(Clone, Debug)]
pub struct CommandReqs {
    /// Auth types this command accepts (any one is sufficient).
    pub accepted: &'static [AuthType],
    /// Human-readable hint for OAuth 2.0 user (when accepted).
    pub oauth2_hint: &'static str,
    /// Human-readable hint for OAuth 1.0a (when accepted).
    pub oauth1_hint: &'static str,
    /// Human-readable hint for Bearer (when accepted).
    pub bearer_hint: &'static str,
}

pub const OAUTH2_HINT: &str =
    "Run `bird login` or set X_API_ACCESS_TOKEN (and optionally X_API_REFRESH_TOKEN).";
pub const OAUTH1_HINT: &str = "set X_API_CONSUMER_KEY, X_API_CONSUMER_SECRET, X_API_OAUTH1_ACCESS_TOKEN, X_API_OAUTH1_ACCESS_TOKEN_SECRET.";
pub const BEARER_HINT: &str = "set X_API_BEARER_TOKEN.";

const ME_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1];
const BOOKMARKS_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User];
// Profile: all three auth types per X API spec for GET /2/users/by/username/{username}
const PROFILE_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
// Search: OAuth 2.0 User, OAuth 1.0a, Bearer per X API spec for GET /2/tweets/search/recent
const SEARCH_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
// Thread: same auth as search (uses /2/tweets/{id} + /2/tweets/search/recent)
const THREAD_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
const RAW_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];

/// Returns requirements for a command by name. Used by execution, errors, and doctor.
pub fn requirements_for_command(name: &str) -> Option<CommandReqs> {
    Some(match name {
        "me" => CommandReqs {
            accepted: ME_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "bookmarks" => CommandReqs {
            accepted: BOOKMARKS_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "get" => CommandReqs {
            accepted: RAW_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "post" => CommandReqs {
            accepted: RAW_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "put" => CommandReqs {
            accepted: RAW_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "delete" => CommandReqs {
            accepted: RAW_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "profile" => CommandReqs {
            accepted: PROFILE_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "search" => CommandReqs {
            accepted: SEARCH_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "thread" => CommandReqs {
            accepted: THREAD_ACCEPTED,
            oauth2_hint: OAUTH2_HINT,
            oauth1_hint: OAUTH1_HINT,
            bearer_hint: BEARER_HINT,
        },
        "login" => return None,
        _ => return None,
    })
}

/// All command names that have auth requirements (for doctor full report).
pub fn command_names_with_auth() -> &'static [&'static str] {
    &[
        "login",
        "me",
        "bookmarks",
        "profile",
        "search",
        "thread",
        "get",
        "post",
        "put",
        "delete",
    ]
}

/// Format a multi-line "auth required" error for a command, listing what to do for each accepted auth type.
/// Does not include the command name (caller prefixes e.g. "me failed: ").
pub fn format_auth_required_error(command_name: &str) -> String {
    let reqs = match requirements_for_command(command_name) {
        Some(r) => r,
        None => return "no valid auth.".to_string(),
    };
    let mut out = "no valid auth for this command.\n".to_string();
    let mut first = true;
    for at in reqs.accepted {
        let hint = match at {
            AuthType::OAuth2User => reqs.oauth2_hint,
            AuthType::OAuth1 => reqs.oauth1_hint,
            AuthType::Bearer => reqs.bearer_hint,
            AuthType::None => continue,
        };
        out.push_str(if first { "  " } else { "  Or " });
        out.push_str(hint);
        out.push('\n');
        first = false;
    }
    out
}

/// Error returned when a command has no valid auth; carries the formatted message for display.
#[derive(Debug)]
pub struct AuthRequiredError(pub String);

impl std::error::Error for AuthRequiredError {}

impl std::fmt::Display for AuthRequiredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Build an AuthRequiredError for a command (used by auth layer when resolve fails).
pub fn auth_required_error(command_name: &str) -> AuthRequiredError {
    AuthRequiredError(format_auth_required_error(command_name))
}

/// One-line reason for doctor when command is unavailable (hints joined by " Or ").
pub fn reason_for_unavailable(reqs: &CommandReqs) -> String {
    let hints: Vec<&str> = reqs
        .accepted
        .iter()
        .filter_map(|at| match at {
            AuthType::OAuth2User => Some(reqs.oauth2_hint),
            AuthType::OAuth1 => Some(reqs.oauth1_hint),
            AuthType::Bearer => Some(reqs.bearer_hint),
            AuthType::None => Option::None,
        })
        .collect();
    hints.join(" Or ")
}
