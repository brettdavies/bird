//! bird doctor: living view of xurl status, auth state, command availability, and entity store health.

use crate::config::ResolvedConfig;
use crate::db::BirdClient;
use serde::Serialize;
use std::collections::HashMap;

/// Full list of bird command names that participate in the doctor report.
/// Replaces U13-deleted `requirements::command_names_with_auth`; the slice
/// stays here because doctor is the only consumer.
const COMMAND_NAMES: &[&str] = &[
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
];

/// `true` when the named command's API endpoint requires any auth scheme
/// other than `AuthType::None`.
fn command_needs_auth(name: &str) -> bool {
    !matches!(
        name,
        "usage" | "watchlist_add" | "watchlist_remove" | "watchlist_list" | "login"
    )
}

/// The embedded X API client: the xdk-rs version linked into this build and
/// whether the client was built.
#[derive(Clone, Debug, Serialize)]
pub struct XurlStatus {
    pub version: Option<String>,
    pub available: bool,
    /// Why the client could not be built; present only when `available` is
    /// `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    /// Per-app credential snapshots, one per app in the `~/.xurl` token
    /// store.
    pub apps: HashMap<String, AppCredentials>,
    pub env: EnvCredentials,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Auth schemes the endpoint accepts (`"app"`, `"oauth1"`, `"oauth2"`),
    /// resolved against `xdk::api::auth_matrix::supported_auth(method,
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
    /// Version of the xurl crate linked into this bird build, populated
    /// from `xdk::CRATE_VERSION`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_xurl_version: Option<String>,
}

/// Map a bird command name to the `(method, path_template)` pair used to
/// query `xdk::api::auth_matrix::supported_auth`. Templates use xdk's
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

fn build_xurl_status(client: &BirdClient) -> XurlStatus {
    let reason = client.client_unavailable_reason();
    XurlStatus {
        version: Some(xdk::CRATE_VERSION.to_string()),
        available: reason.is_none(),
        reason,
    }
}

/// Report whether `CLIENT_ID` / `CLIENT_SECRET` / `XURL_BEARER_TOKEN` were
/// set in the captured environment without ever inspecting their values. An
/// empty value counts as "not set" because that is the predominant cause of
/// subtle auth failures.
fn build_env_credentials(env: &xdk::config::EnvOverrides) -> EnvCredentials {
    let is_set = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.is_empty());
    EnvCredentials {
        client_id_set: is_set(&env.client_id),
        client_secret_set: is_set(&env.client_secret),
        bearer_token_set: is_set(&env.bearer_token),
    }
}

/// Build the auth section by enumerating every app in xurl's `TokenStore`
/// and reporting presence-only flags for every credential type. The active
/// app is the one xurl's runner would pick when no `--app NAME` is supplied.
fn build_auth_state(config: &ResolvedConfig) -> AuthState {
    let (_, auth) =
        crate::xurl_client::open_store(&config.xdk_env, config.token_store.as_deref(), None);
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
        env: build_env_credentials(&config.xdk_env),
    }
}

/// Map a polymorphic `Token` carrying an `OAuth2` payload to bird's
/// presence descriptor. `token.oauth2` is `Some` when
/// `token_type == TokenType::Oauth2`; for any other discriminator the
/// payload is absent and every presence flag stays `false`.
fn oauth2_presence(token: &xdk::store::Token) -> OAuth2TokenPresence {
    let payload = token.oauth2.as_ref();
    OAuth2TokenPresence {
        access_token_present: payload.is_some_and(|p| !p.access_token.is_empty()),
        refresh_token_present: payload.is_some_and(|p| !p.refresh_token.is_empty()),
    }
}

fn oauth1_presence(token: &xdk::store::Token) -> OAuth1TokenPresence {
    let payload = token.oauth1.as_ref();
    OAuth1TokenPresence {
        access_token_present: payload.is_some_and(|p| !p.access_token.is_empty()),
        token_secret_present: payload.is_some_and(|p| !p.token_secret.is_empty()),
        consumer_key_present: payload.is_some_and(|p| !p.consumer_key.is_empty()),
        consumer_secret_present: payload.is_some_and(|p| !p.consumer_secret.is_empty()),
    }
}

fn bearer_presence(token: &xdk::store::Token) -> BearerTokenPresence {
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

/// Resolve accepted_schemes for a single command via xurl's `auth_matrix`.
fn accepted_schemes_for(method: &str, template: &str) -> Vec<String> {
    use xdk::api::auth_matrix::{WireScheme, supported_auth};
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

/// Command availability based on xurl + auth state. Populates the new
/// R17b fields `accepted_schemes`, `credentialed_schemes`, and `reachable`
/// against the resolved active app.
fn build_commands_section(xurl: &XurlStatus, auth: &AuthState) -> HashMap<String, CommandStatus> {
    let xurl_available = xurl.available;
    let unavailable_reason = || {
        format!(
            "X API client unavailable: {}",
            xurl.reason.as_deref().unwrap_or("unknown error")
        )
    };
    let mut cmds = HashMap::new();
    let active = auth.apps.get(&auth.active_app);
    let credentialed: Vec<String> = active
        .map(|app| credentialed_for_app(app, &auth.env))
        .unwrap_or_default();
    let authenticated = !credentialed.is_empty();

    for &name in COMMAND_NAMES {
        if name == "login" {
            let reason = if xurl_available {
                None
            } else {
                Some(unavailable_reason())
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
        let needs_auth = command_needs_auth(name);
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
            Some(unavailable_reason())
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

/// Build full or scoped report.
pub(crate) fn report(
    client: &BirdClient,
    config: &ResolvedConfig,
    scope: Option<&str>,
) -> DoctorReport {
    let xurl = build_xurl_status(client);
    let auth = build_auth_state(config);
    let mut commands = build_commands_section(&xurl, &auth);
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

    let linked_xurl_version = Some(xdk::CRATE_VERSION.to_string());

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

    out.push_str(&format!("{}\n", output::section("X API client", use_color)));
    if report.xurl.available {
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
            output::error("unavailable", use_color)
        ));
        if let Some(ref reason) = report.xurl.reason {
            out.push_str(&format!("  {reason}\n"));
        }
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
    config: &ResolvedConfig,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    pretty: bool,
    scope: Option<&str>,
    use_emoji: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let use_color = cfg.use_color;
    let r = report(client, config, scope);
    if pretty {
        writeln!(stdout, "{}", format_pretty(&r, use_color, use_emoji))?;
    } else {
        cfg.print_ok(stdout, serde_json::to_value(&r)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{BirdClient, CacheOpts};
    use crate::xurl_client::mock::MockXurlClient;
    use std::path::Path;

    fn no_cache_client() -> BirdClient {
        BirdClient::new(
            Box::new(MockXurlClient::new()),
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

    /// A config whose token store and environment are empty, so the report
    /// never reads the machine's real `~/.xurl` or credentials.
    fn isolated_config() -> ResolvedConfig {
        let dir = std::env::temp_dir().join(format!("bird-doctor-test-{}", std::process::id()));
        ResolvedConfig {
            username: None,
            config_dir: dir.clone(),
            cache_path: dir.join("bird.db"),
            cache_enabled: false,
            cache_max_size_mb: 100,
            token_store: Some(dir.join("no-such-store").join("auth.yml")),
            xdk_env: xdk::config::EnvOverrides::default(),
            app: None,
        }
    }

    #[test]
    fn doctor_report_has_commands() {
        let client = no_cache_client();
        let r = report(&client, &isolated_config(), None);
        assert!(!r.commands.is_empty());
        assert!(r.commands.contains_key("me"));
        assert!(r.commands.contains_key("login"));
    }

    #[test]
    fn doctor_report_scoped_has_only_that_command() {
        let client = no_cache_client();
        let r = report(&client, &isolated_config(), Some("me"));
        assert_eq!(r.commands.len(), 1);
        assert!(r.commands.contains_key("me"));
    }

    /// A client that failed to build reports itself unavailable, names the
    /// construction error, and marks every API command unavailable with it.
    #[test]
    fn doctor_reports_a_client_that_failed_to_build() {
        let client = BirdClient::new(
            Box::new(crate::xurl_client::ConstructionStub::new(
                "cannot start the async runtime".into(),
            )),
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
        );
        let r = report(&client, &isolated_config(), None);
        assert!(!r.xurl.available);
        assert_eq!(
            r.xurl.reason.as_deref(),
            Some("cannot start the async runtime")
        );
        let me = &r.commands["me"];
        assert!(!me.available);
        assert!(
            me.reason
                .as_deref()
                .is_some_and(|reason| reason.contains("cannot start the async runtime")),
            "command reason must name the construction error: {:?}",
            me.reason,
        );
    }

    #[test]
    fn doctor_report_built_client_is_available_without_reason() {
        let client = no_cache_client();
        let r = report(&client, &isolated_config(), None);
        assert!(r.xurl.available);
        assert!(r.xurl.reason.is_none());
        let json = serde_json::to_value(&r).expect("serialize");
        assert!(json["xurl"].get("reason").is_none());
        assert!(json["xurl"].get("path").is_none());
    }

    #[test]
    fn doctor_report_json_serializable() {
        let client = no_cache_client();
        let r = report(&client, &isolated_config(), None);
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
}
