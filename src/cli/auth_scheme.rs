//! `AuthType` enum — bird's representation of "which scheme the caller
//! picked for this call". The xurl wire string (`"app"` / `"oauth1"` /
//! `"oauth2"`) is rendered by `db::client::embedded::auth_type_to_xurl_wire`.

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
