//! Central command requirements: which auth types each command accepts.
//! Used by execution, doctor (availability), and auth_flag mapping for xurl.

/// Auth types that a command can accept (OAuth2UserToken, OAuth1 UserToken, BearerToken).
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

/// Per-command auth requirements: which auth types are accepted.
#[derive(Clone, Debug)]
pub struct CommandReqs {
    /// Auth types this command accepts (any one is sufficient).
    pub accepted: &'static [AuthType],
}

/// Map AuthType to xurl `--auth` flag value.
/// Returns None when xurl's default (OAuth2 user) is correct.
pub fn auth_flag(auth_type: &AuthType) -> Option<&'static str> {
    match auth_type {
        AuthType::OAuth2User => None, // xurl defaults to OAuth2
        AuthType::OAuth1 => Some("oauth1"),
        AuthType::Bearer => Some("app"),
        AuthType::None => None,
    }
}

const ME_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1];
const OAUTH2_ONLY: &[AuthType] = &[AuthType::OAuth2User];
// Profile: all three auth types per X API spec for GET /2/users/by/username/{username}
const PROFILE_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
// Search: OAuth 2.0 User, OAuth 1.0a, Bearer per X API spec for GET /2/tweets/search/recent
const SEARCH_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
// Thread: same auth as search (uses /2/tweets/{id} + /2/tweets/search/recent)
const THREAD_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];
const RAW_ACCEPTED: &[AuthType] = &[AuthType::OAuth2User, AuthType::OAuth1, AuthType::Bearer];

/// Returns requirements for a command by name. Used by execution and doctor.
pub fn requirements_for_command(name: &str) -> Option<CommandReqs> {
    Some(match name {
        "me" => CommandReqs {
            accepted: ME_ACCEPTED,
        },
        "bookmarks" => CommandReqs {
            accepted: OAUTH2_ONLY,
        },
        "get" | "post" | "put" | "delete" => CommandReqs {
            accepted: RAW_ACCEPTED,
        },
        "profile" => CommandReqs {
            accepted: PROFILE_ACCEPTED,
        },
        "search" => CommandReqs {
            accepted: SEARCH_ACCEPTED,
        },
        "thread" => CommandReqs {
            accepted: THREAD_ACCEPTED,
        },
        // Write commands (all require OAuth2User)
        "tweet" | "reply" | "like" | "unlike" | "repost" | "unrepost" | "follow" | "unfollow"
        | "dm" | "block" | "unblock" | "mute" | "unmute" => CommandReqs {
            accepted: OAUTH2_ONLY,
        },
        "watchlist_check" => CommandReqs {
            accepted: SEARCH_ACCEPTED,
        },
        "watchlist_add" | "watchlist_remove" | "watchlist_list" => CommandReqs {
            accepted: &[AuthType::None],
        },
        "usage" => CommandReqs {
            accepted: &[AuthType::None],
        },
        // API sync path (default unless --local); Bearer auth for GET /2/usage/tweets
        "usage_sync" => CommandReqs {
            accepted: &[AuthType::Bearer],
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
        "tweet",
        "reply",
        "like",
        "unlike",
        "repost",
        "unrepost",
        "follow",
        "unfollow",
        "dm",
        "block",
        "unblock",
        "mute",
        "unmute",
        "watchlist_check",
        "watchlist_add",
        "watchlist_remove",
        "watchlist_list",
        "usage",
        "usage_sync",
        "get",
        "post",
        "put",
        "delete",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_and_requirements_in_sync() {
        for &name in command_names_with_auth() {
            // login is in the list for doctor reporting but has no auth requirements
            if name == "login" {
                continue;
            }
            assert!(
                requirements_for_command(name).is_some(),
                "command '{}' in command_names_with_auth() but missing from requirements_for_command()",
                name
            );
        }
    }
}
