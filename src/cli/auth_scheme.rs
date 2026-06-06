//! `AuthType` enum + subprocess `xurl --auth` flag mapping. Lives here so
//! bird carries a single type representing "which scheme bird picked for
//! this call". Under `--features embedded-xurl` the enum's only consumer
//! is `db::client::embedded::auth_type_to_xurl_wire`, which renders it as
//! xurl's wire string (`"app"` / `"oauth1"` / `"oauth2"`). Under the
//! subprocess transport, `auth_flag` maps the enum to the equivalent
//! `xr --auth` argv flag.
//!
//! The "what scheme does THIS command need" knowledge (per-command auth
//! tables) was lifted out of bird at U13 and now flows from
//! `xurl::api::auth_matrix::supported_auth` under embedded or a tiny
//! inline match in `cli::dispatch` under subprocess. PR3 deletes the
//! subprocess path entirely, at which point `auth_flag` and the wire
//! mapping converge in one home.

/// Auth schemes bird picks at request time. The enum has stayed identical
/// since bird's pre-xurl era; renaming or restructuring would churn every
/// handler signature for no downstream gain.
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

/// Map `AuthType` to the `xurl --auth` flag string used in subprocess
/// argv. Returns `None` when xurl's default (`oauth2`) is correct. Only
/// the subprocess transport path consults this; the embedded transport
/// reads the wire string directly via `auth_type_to_xurl_wire` in
/// `db::client::embedded`.
#[cfg(not(feature = "embedded-xurl"))]
pub fn auth_flag(auth_type: &AuthType) -> Option<&'static str> {
    match auth_type {
        AuthType::OAuth2User => None,
        AuthType::OAuth1 => Some("oauth1"),
        AuthType::Bearer => Some("app"),
        AuthType::None => None,
    }
}
