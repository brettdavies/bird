//! bird doctor: living view of xurl status, auth state, command availability, and entity store health.

use crate::db::BirdClient;
use crate::requirements::{AuthType, command_names_with_auth, requirements_for_command};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct XurlStatus {
    pub path: Option<String>,
    pub version: Option<String>,
    pub available: bool,
}

/// Presence-only descriptor for a single OAuth2 token entry. Mirrors xurl's
/// `Token` shape but exposes booleans only, per KTD-6: no token material
/// reaches the report.
#[derive(Clone, Debug, Serialize, Default)]
pub struct OAuth2TokenPresence {
    pub access_token_present: bool,
    pub refresh_token_present: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct OAuth1TokenPresence {
    pub access_token_present: bool,
    pub token_secret_present: bool,
    pub consumer_key_present: bool,
    pub consumer_secret_present: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct BearerTokenPresence {
    pub token_present: bool,
}

/// Per-app credential snapshot. Every field that touches a token / secret is
/// surfaced as a boolean `*_present` / `*_set` so the JSON output cannot
/// contain credential material.
#[derive(Clone, Debug, Serialize, Default)]
pub struct AppCredentials {
    pub client_id_set: bool,
    pub client_secret_set: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_user: Option<String>,
    pub oauth2_tokens: HashMap<String, OAuth2TokenPresence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unnamed_oauth2: Option<OAuth2TokenPresence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth1: Option<OAuth1TokenPresence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer: Option<BearerTokenPresence>,
}

/// Environment-variable credential presence. Read once at report-build
/// time so the JSON shows the same values agents and humans would observe.
#[derive(Clone, Debug, Serialize, Default)]
pub struct EnvCredentials {
    pub client_id_set: bool,
    pub client_secret_set: bool,
    pub bearer_token_set: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthState {
    /// Resolved active app (the one used when no `--app NAME` is supplied).
    pub active_app: String,
    /// Per-app credential snapshots. Subprocess builds populate a single
    /// entry for the active app from `xurl whoami`; embedded builds
    /// enumerate every stored app via xurl's `TokenStore`.
    pub apps: HashMap<String, AppCredentials>,
    pub env: EnvCredentials,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Auth schemes the endpoint accepts (`"app"`, `"oauth1"`, `"oauth2"`),
    /// resolved against `xurl::api::auth_matrix::supported_auth(method,
    /// template)` under embedded.
    pub accepted_schemes: Vec<String>,
    /// Subset of `accepted_schemes` for which the active app has a stored
    /// credential. Presence-only (per KTD-6).
    pub credentialed_schemes: Vec<String>,
    /// `true` when `accepted_schemes ∩ credentialed_schemes` is non-empty.
    /// Agents key on this single bit to know whether the command will
    /// succeed without an additional auth step.
    pub reachable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheStatus {
    pub path: String,
    pub exists: bool,
    pub size_mb: f64,
    pub max_size_mb: u64,
    pub tweets: u64,
    pub users: u64,
    pub raw_responses: u64,
    pub healthy: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub xurl: XurlStatus,
    pub auth: AuthState,
    pub commands: HashMap<String, CommandStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheStatus>,
    /// Version of the xurl crate linked into this bird build. Populated
    /// under `--features embedded-xurl` from `xurl::CRATE_VERSION`;
    /// subprocess builds leave this `None` so the subprocess version (read
    /// via `xurl --version`) keeps its existing `xurl.version` slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_xurl_version: Option<String>,
}

/// Map a bird command name to the `(method, path_template)` pair used to
/// query `xurl::api::auth_matrix::supported_auth`. Templates use xurl's
/// substitution vocabulary (`{id}`, `{username}`, `{participant_id}`,
/// `{source_user_id}`, `{target_user_id}`, `{source_tweet_id}`) verbatim.
/// `None` for commands that do not hit the X API (local-only watchlist
/// operations, cache, doctor itself).
fn command_template(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "me" => ("GET", "/2/users/me"),
        "bookmarks" => ("GET", "/2/users/{id}/bookmarks"),
        "profile" => ("GET", "/2/users/by/username/{username}"),
        "search" => ("GET", "/2/tweets/search/recent"),
        "thread" => ("GET", "/2/tweets/{id}"),
        "tweet" | "reply" => ("POST", "/2/tweets"),
        "like" => ("POST", "/2/users/{id}/likes"),
        "unlike" => ("DELETE", "/2/users/{id}/likes/{tweet_id}"),
        "repost" => ("POST", "/2/users/{id}/retweets"),
        "unrepost" => ("DELETE", "/2/users/{id}/retweets/{source_tweet_id}"),
        "follow" => ("POST", "/2/users/{id}/following"),
        "unfollow" => (
            "DELETE",
            "/2/users/{source_user_id}/following/{target_user_id}",
        ),
        "mute" => ("POST", "/2/users/{id}/muting"),
        "unmute" => (
            "DELETE",
            "/2/users/{source_user_id}/muting/{target_user_id}",
        ),
        "block" => ("POST", "/2/users/{id}/blocking"),
        "unblock" => (
            "DELETE",
            "/2/users/{source_user_id}/blocking/{target_user_id}",
        ),
        "dm" => ("POST", "/2/dm_conversations/with/{participant_id}/messages"),
        "usage" | "usage_sync" => ("GET", "/2/usage/tweets"),
        "watchlist_check" => ("GET", "/2/tweets/search/recent"),
        "get" | "post" | "put" | "delete" | "login" | "watchlist_add" | "watchlist_remove"
        | "watchlist_list" => return None,
        _ => return None,
    })
}

#[cfg(feature = "embedded-xurl")]
fn build_xurl_status(
    _client: &BirdClient,
    _stderr: &mut dyn std::io::Write,
    _quiet: bool,
) -> XurlStatus {
    XurlStatus {
        path: None,
        version: Some(xurl::CRATE_VERSION.to_string()),
        available: true,
    }
}

#[cfg(not(feature = "embedded-xurl"))]
fn build_xurl_status(
    client: &BirdClient,
    stderr: &mut dyn std::io::Write,
    quiet: bool,
) -> XurlStatus {
    match client.xurl_path() {
        Some(path) => {
            let version = crate::transport::check_xurl_version(path, stderr, quiet).ok();
            XurlStatus {
                path: Some(path.display().to_string()),
                version,
                available: true,
            }
        }
        None => XurlStatus {
            path: None,
            version: None,
            available: false,
        },
    }
}

/// Read presence of `CLIENT_ID` / `CLIENT_SECRET` / `XURL_BEARER_TOKEN` env
/// vars without ever inspecting their values. An empty value counts as
/// "not set" because that is the predominant cause of subtle auth failures.
fn build_env_credentials() -> EnvCredentials {
    let is_set =
        |name: &str| -> bool { std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false) };
    EnvCredentials {
        client_id_set: is_set("CLIENT_ID"),
        client_secret_set: is_set("CLIENT_SECRET"),
        bearer_token_set: is_set("XURL_BEARER_TOKEN"),
    }
}

/// Build the auth section under subprocess. Reads `xurl whoami` (which
/// the subprocess transport already handles) to learn the active username,
/// then surfaces a single-app shape so the JSON envelope stays well-formed
/// without invading xurl's token store directly.
#[cfg(not(feature = "embedded-xurl"))]
fn build_auth_state(client: &BirdClient) -> AuthState {
    let username = client
        .transport_request(&["whoami".to_string()])
        .ok()
        .and_then(|json| {
            json.get("data")
                .and_then(|d| d.get("username"))
                .and_then(|u| u.as_str())
                .or_else(|| json.get("username").and_then(|u| u.as_str()))
                .map(String::from)
        });
    let mut apps = HashMap::new();
    apps.insert(
        "default".to_string(),
        AppCredentials {
            default_user: username,
            ..AppCredentials::default()
        },
    );
    AuthState {
        active_app: "default".to_string(),
        apps,
        env: build_env_credentials(),
    }
}

/// Build the auth section under embedded. Enumerates every app in xurl's
/// `TokenStore` and reports presence-only flags for every credential type.
/// The active app is the one xurl's runner would pick when no `--app NAME`
/// is supplied.
#[cfg(feature = "embedded-xurl")]
fn build_auth_state(_client: &BirdClient) -> AuthState {
    let cfg = xurl::config::Config::new();
    let auth = xurl::auth::Auth::new(&cfg);
    let store = auth.token_store();
    let active_app = store.get_default_app().to_string();

    let mut apps = HashMap::new();
    for name in store.list_apps() {
        let Some(app) = store.get_app(&name) else {
            continue;
        };
        let mut oauth2_tokens = HashMap::new();
        for (user, token) in &app.oauth2_tokens {
            oauth2_tokens.insert(user.clone(), oauth2_presence(token));
        }
        let unnamed_oauth2 = app.unnamed_oauth2_token.as_ref().map(oauth2_presence);
        let oauth1 = app.oauth1_token.as_ref().map(oauth1_presence);
        let bearer = app.bearer_token.as_ref().map(bearer_presence);
        let default_user_str = store.get_default_user(&name);
        let default_user = if default_user_str.is_empty() {
            None
        } else {
            Some(default_user_str.to_string())
        };
        apps.insert(
            name.clone(),
            AppCredentials {
                client_id_set: !app.client_id.is_empty(),
                client_secret_set: !app.client_secret.is_empty(),
                default_user,
                oauth2_tokens,
                unnamed_oauth2,
                oauth1,
                bearer,
            },
        );
    }
    AuthState {
        active_app,
        apps,
        env: build_env_credentials(),
    }
}

/// Map a polymorphic `Token` carrying an `OAuth2` payload to bird's
/// presence descriptor. `token.oauth2` is `Some` when
/// `token_type == TokenType::Oauth2`; for any other discriminator the
/// payload is absent and every presence flag stays `false`.
#[cfg(feature = "embedded-xurl")]
fn oauth2_presence(token: &xurl::store::types::Token) -> OAuth2TokenPresence {
    let payload = token.oauth2.as_ref();
    OAuth2TokenPresence {
        access_token_present: payload.is_some_and(|p| !p.access_token.is_empty()),
        refresh_token_present: payload.is_some_and(|p| !p.refresh_token.is_empty()),
    }
}

#[cfg(feature = "embedded-xurl")]
fn oauth1_presence(token: &xurl::store::types::Token) -> OAuth1TokenPresence {
    let payload = token.oauth1.as_ref();
    OAuth1TokenPresence {
        access_token_present: payload.is_some_and(|p| !p.access_token.is_empty()),
        token_secret_present: payload.is_some_and(|p| !p.token_secret.is_empty()),
        consumer_key_present: payload.is_some_and(|p| !p.consumer_key.is_empty()),
        consumer_secret_present: payload.is_some_and(|p| !p.consumer_secret.is_empty()),
    }
}

#[cfg(feature = "embedded-xurl")]
fn bearer_presence(token: &xurl::store::types::Token) -> BearerTokenPresence {
    BearerTokenPresence {
        token_present: token.bearer.as_ref().is_some_and(|s| !s.is_empty()),
    }
}

/// Returns the wire-string set of auth schemes this app has stored credentials
/// for: `["oauth2", "oauth1", "app"]` in preference order, filtered by which
/// types are populated. Mirrors xurl's auto-detect ordering.
fn credentialed_for_app(app: &AppCredentials, env: &EnvCredentials) -> Vec<String> {
    let mut out = Vec::new();
    let has_oauth2 = !app.oauth2_tokens.is_empty() || app.unnamed_oauth2.is_some();
    if has_oauth2 {
        out.push("oauth2".to_string());
    }
    if app.oauth1.is_some() {
        out.push("oauth1".to_string());
    }
    let has_bearer = app.bearer.is_some() || env.bearer_token_set;
    if has_bearer {
        out.push("app".to_string());
    }
    out
}

/// Resolve accepted_schemes for a single command. Under embedded queries
/// xurl's `auth_matrix`; under subprocess returns an empty vec (the
/// subprocess transport can't query the matrix at all without spawning
/// xurl, which the doctor command deliberately avoids).
#[cfg(feature = "embedded-xurl")]
fn accepted_schemes_for(method: &str, template: &str) -> Vec<String> {
    use xurl::api::auth_matrix::{WireScheme, supported_auth};
    let Some(schemes) = supported_auth(method, template) else {
        return Vec::new();
    };
    let mut wires: Vec<String> = schemes
        .iter()
        .map(|s| s.wire().as_wire().to_string())
        .collect();
    // Preserve preference order (oauth2 → oauth1 → app).
    let order = |w: &str| match w {
        "oauth2" => 0,
        "oauth1" => 1,
        "app" => 2,
        _ => 3,
    };
    wires.sort_by_key(|w| order(w));
    wires.dedup();
    let _ = WireScheme::ALL_BY_PREFERENCE; // touch to discourage drift on the upstream enum
    wires
}

#[cfg(not(feature = "embedded-xurl"))]
fn accepted_schemes_for(_method: &str, _template: &str) -> Vec<String> {
    Vec::new()
}

/// Command availability based on xurl + auth state. Populates the new
/// R17b fields `accepted_schemes`, `credentialed_schemes`, and `reachable`
/// against the resolved active app.
fn build_commands_section(
    xurl_available: bool,
    auth: &AuthState,
) -> HashMap<String, CommandStatus> {
    let mut cmds = HashMap::new();
    let active = auth.apps.get(&auth.active_app);
    let credentialed: Vec<String> = active
        .map(|app| credentialed_for_app(app, &auth.env))
        .unwrap_or_default();
    let authenticated = !credentialed.is_empty();

    for &name in command_names_with_auth() {
        if name == "login" {
            let reason = if xurl_available {
                None
            } else {
                Some(xurl_unavailable_reason())
            };
            cmds.insert(
                name.to_string(),
                CommandStatus {
                    available: xurl_available,
                    reason,
                    accepted_schemes: Vec::new(),
                    credentialed_schemes: Vec::new(),
                    reachable: xurl_available,
                },
            );
            continue;
        }
        let reqs = match requirements_for_command(name) {
            Some(r) => r,
            None => continue,
        };
        let needs_auth = reqs.accepted.iter().any(|at| !matches!(at, AuthType::None));
        let accepted_schemes = command_template(name)
            .map(|(m, t)| accepted_schemes_for(m, t))
            .unwrap_or_default();
        let credentialed_schemes: Vec<String> = accepted_schemes
            .iter()
            .filter(|s| credentialed.iter().any(|c| c == *s))
            .cloned()
            .collect();
        let reachable = if needs_auth {
            xurl_available && !credentialed_schemes.is_empty()
        } else {
            true
        };
        let available = if needs_auth {
            xurl_available && authenticated
        } else {
            true
        };
        let reason = if !xurl_available {
            Some(xurl_unavailable_reason())
        } else if needs_auth && !authenticated {
            Some("not authenticated. Run `bird login`.".into())
        } else {
            None
        };
        cmds.insert(
            name.to_string(),
            CommandStatus {
                available,
                reason,
                accepted_schemes,
                credentialed_schemes,
                reachable,
            },
        );
    }
    cmds
}

#[cfg(not(feature = "embedded-xurl"))]
fn xurl_unavailable_reason() -> String {
    format!("xurl not found. {}", crate::transport::XURL_INSTALL_HINT)
}

#[cfg(feature = "embedded-xurl")]
fn xurl_unavailable_reason() -> String {
    "embedded xurl client unavailable; check CLIENT_ID/CLIENT_SECRET env".into()
}

/// Build full or scoped report.
pub(crate) fn report(
    client: &BirdClient,
    stderr: &mut dyn std::io::Write,
    scope: Option<&str>,
    quiet: bool,
) -> DoctorReport {
    let xurl = build_xurl_status(client, stderr, quiet);
    let auth = build_auth_state(client);
    let mut commands = build_commands_section(xurl.available, &auth);
    if let Some(cmd) = scope
        && let Some(status) = commands.remove(cmd)
    {
        commands.clear();
        commands.insert(cmd.to_string(), status);
    }

    let cache = match client.db_stats() {
        Some(Ok(stats)) => {
            let path = client
                .db_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            Some(CacheStatus {
                path,
                exists: true,
                size_mb: (stats.size_mb() * 10.0).round() / 10.0,
                max_size_mb: stats.max_size_mb() as u64,
                tweets: stats.tweet_count,
                users: stats.user_count,
                raw_responses: stats.raw_response_count,
                healthy: stats.healthy(),
            })
        }
        Some(Err(_)) => Some(CacheStatus {
            path: "unknown".to_string(),
            exists: false,
            size_mb: 0.0,
            max_size_mb: 100,
            tweets: 0,
            users: 0,
            raw_responses: 0,
            healthy: false,
        }),
        None => None,
    };

    #[cfg(feature = "embedded-xurl")]
    let linked_xurl_version = Some(xurl::CRATE_VERSION.to_string());
    #[cfg(not(feature = "embedded-xurl"))]
    let linked_xurl_version: Option<String> = None;

    DoctorReport {
        xurl,
        auth,
        commands,
        cache,
        linked_xurl_version,
    }
}

fn format_pretty(report: &DoctorReport, use_color: bool, use_emoji: bool) -> String {
    use crate::output;
    let mut out = String::new();

    out.push_str(&format!("{}\n", output::section("Xurl", use_color)));
    if report.xurl.available {
        if let Some(ref path) = report.xurl.path {
            out.push_str(&format!("  path: {}\n", output::muted(path, use_color)));
        }
        if let Some(ref version) = report.xurl.version {
            out.push_str(&format!(
                "  version: {}\n",
                output::muted(version, use_color)
            ));
        }
        out.push_str(&format!(
            "  status: {}\n",
            output::success("available", use_color)
        ));
    } else {
        out.push_str(&format!(
            "  status: {}\n",
            output::error("not found", use_color)
        ));
        out.push_str(&format!("  {}\n", xurl_unavailable_reason()));
    }
    if let Some(ref linked) = report.linked_xurl_version {
        out.push_str(&format!(
            "  linked_crate_version: {}\n",
            output::muted(linked, use_color)
        ));
    }

    out.push_str(&format!("\n{}\n", output::section("Auth", use_color)));
    out.push_str(&format!(
        "  active_app: {}\n",
        output::muted(&report.auth.active_app, use_color)
    ));
    out.push_str(&format!(
        "  env CLIENT_ID: {}\n",
        if report.auth.env.client_id_set {
            "set"
        } else {
            "not set"
        },
    ));
    out.push_str(&format!(
        "  env CLIENT_SECRET: {}\n",
        if report.auth.env.client_secret_set {
            "set"
        } else {
            "not set"
        },
    ));
    out.push_str(&format!(
        "  env XURL_BEARER_TOKEN: {}\n",
        if report.auth.env.bearer_token_set {
            "set"
        } else {
            "not set"
        },
    ));
    let mut app_names: Vec<_> = report.auth.apps.keys().collect();
    app_names.sort();
    for name in app_names {
        let Some(app) = report.auth.apps.get(name) else {
            continue;
        };
        out.push_str(&format!("  app `{}`:\n", name));
        out.push_str(&format!(
            "    client_id: {}\n",
            if app.client_id_set { "set" } else { "not set" },
        ));
        out.push_str(&format!(
            "    client_secret: {}\n",
            if app.client_secret_set {
                "set"
            } else {
                "not set"
            },
        ));
        if let Some(ref user) = app.default_user {
            out.push_str(&format!(
                "    default_user: @{}\n",
                output::muted(user, use_color)
            ));
        }
        if !app.oauth2_tokens.is_empty() {
            let users: Vec<&String> = app.oauth2_tokens.keys().collect();
            out.push_str(&format!("    oauth2_users: {} stored\n", users.len()));
        }
        if app.unnamed_oauth2.is_some() {
            out.push_str("    oauth2_unnamed: present\n");
        }
        if app.oauth1.is_some() {
            out.push_str("    oauth1: present\n");
        }
        if app.bearer.is_some() {
            out.push_str("    bearer: present\n");
        }
    }

    out.push_str(&format!("\n{}\n", output::section("Commands", use_color)));
    let mut names: Vec<_> = report.commands.keys().collect();
    names.sort();
    for name in names {
        let Some(status) = report.commands.get(name) else {
            continue;
        };
        let (emoji, r) = if status.available {
            (
                output::emoji_available(use_emoji),
                output::success("available", use_color),
            )
        } else {
            let reason = status.reason.as_deref().unwrap_or("");
            (
                output::emoji_unavailable(use_emoji),
                format!(
                    "{}{}",
                    output::error("unavailable: ", use_color),
                    output::muted(reason, use_color)
                ),
            )
        };
        out.push_str(&format!(
            "  {}: {}{}\n",
            output::command(name, use_color),
            emoji,
            r
        ));
        if !status.accepted_schemes.is_empty() {
            out.push_str(&format!(
                "    accepts: {}\n",
                status.accepted_schemes.join(", "),
            ));
        }
        if !status.credentialed_schemes.is_empty() {
            out.push_str(&format!(
                "    credentialed: {}\n",
                status.credentialed_schemes.join(", "),
            ));
        }
        out.push_str(&format!(
            "    reachable: {}\n",
            if status.reachable { "yes" } else { "no" },
        ));
    }

    if let Some(ref cache) = report.cache {
        out.push_str(&format!("\n{}\n", output::section("Cache", use_color)));
        out.push_str(&format!(
            "  path: {}\n",
            output::muted(&cache.path, use_color)
        ));
        out.push_str(&format!(
            "  size: {}\n",
            output::muted(
                &format!("{:.1} MB / {} MB", cache.size_mb, cache.max_size_mb),
                use_color
            )
        ));
        out.push_str(&format!(
            "  tweets: {}\n",
            output::muted(&cache.tweets.to_string(), use_color)
        ));
        out.push_str(&format!(
            "  users: {}\n",
            output::muted(&cache.users.to_string(), use_color)
        ));
        out.push_str(&format!(
            "  raw_responses: {}\n",
            output::muted(&cache.raw_responses.to_string(), use_color)
        ));
        let status = if cache.healthy {
            "healthy"
        } else {
            "unhealthy"
        };
        out.push_str(&format!(
            "  status: {}\n",
            if cache.healthy {
                output::success(status, use_color)
            } else {
                output::error(status, use_color)
            }
        ));
    }

    out
}

/// Run doctor: build report and print JSON (compact) or human summary.
///
/// Signature takes `&OutputConfig` and an injected stdout writer (Plan 2 U2);
/// per-line output writes through `writeln!(stdout, ...)` (Plan 2 U5 / R13).
/// `use_emoji` stays a caller-resolved arg because the dispatcher derives it
/// from `use_color && pretty`.
pub fn run_doctor(
    client: &BirdClient,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    pretty: bool,
    scope: Option<&str>,
    use_emoji: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let use_color = cfg.use_color;
    let quiet = cfg.suppress_diag();
    let r = report(client, stderr, scope, quiet);
    if pretty {
        writeln!(stdout, "{}", format_pretty(&r, use_color, use_emoji))?;
    } else {
        writeln!(stdout, "{}", serde_json::to_string(&r)?)?;
    }
    Ok(())
}

#[cfg(all(test, not(feature = "embedded-xurl")))]
mod tests {
    use super::*;
    use crate::db::{BirdClient, CacheOpts};
    use crate::transport::tests::MockTransport;
    use std::path::Path;

    fn no_cache_client() -> BirdClient {
        let transport = Box::new(MockTransport::new(vec![]));
        BirdClient::new(
            transport,
            Path::new("/dev/null"),
            CacheOpts {
                no_store: true,
                refresh: false,
                cache_only: false,
            },
            100,
            None,
            false,
            std::sync::Arc::new(std::sync::Mutex::new(std::io::sink())),
        )
    }

    #[test]
    fn doctor_report_has_commands() {
        let client = no_cache_client();
        let r = report(&client, &mut std::io::sink(), None, false);
        assert!(!r.commands.is_empty());
        assert!(r.commands.contains_key("me"));
        assert!(r.commands.contains_key("login"));
    }

    #[test]
    fn doctor_report_scoped_has_only_that_command() {
        let client = no_cache_client();
        let r = report(&client, &mut std::io::sink(), Some("me"), false);
        assert_eq!(r.commands.len(), 1);
        assert!(r.commands.contains_key("me"));
    }

    #[test]
    fn doctor_report_json_serializable() {
        let client = no_cache_client();
        let r = report(&client, &mut std::io::sink(), None, false);
        let json = serde_json::to_string(&r).expect("test");
        assert!(json.contains("xurl"));
        assert!(json.contains("auth"));
        assert!(json.contains("commands"));
        assert!(json.contains("active_app"));
        assert!(json.contains("env"));
        assert!(json.contains("accepted_schemes"));
        assert!(json.contains("credentialed_schemes"));
        assert!(json.contains("reachable"));
    }

    #[test]
    fn auth_state_subprocess_has_default_app_entry() {
        let client = no_cache_client();
        let r = report(&client, &mut std::io::sink(), None, false);
        assert_eq!(r.auth.active_app, "default");
        assert!(r.auth.apps.contains_key("default"));
    }
}
