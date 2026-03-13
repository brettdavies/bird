//! bird doctor: living view of xurl status, auth state, command availability, and entity store health.

use crate::db::BirdClient;
use crate::requirements::{command_names_with_auth, requirements_for_command, AuthType};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct XurlStatus {
    pub path: Option<String>,
    pub version: Option<String>,
    pub available: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthState {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
}

fn build_xurl_status() -> XurlStatus {
    match crate::transport::resolve_xurl_path() {
        Ok(path) => {
            let version = crate::transport::check_xurl_version(path).ok();
            XurlStatus {
                path: Some(path.display().to_string()),
                version,
                available: true,
            }
        }
        Err(_) => XurlStatus {
            path: None,
            version: None,
            available: false,
        },
    }
}

/// Detect auth state by running `xurl whoami`. Returns username on success.
fn detect_auth() -> AuthState {
    match crate::transport::xurl_call(&["whoami"]) {
        Ok(json) => {
            let username = json
                .get("data")
                .and_then(|d| d.get("username"))
                .and_then(|u| u.as_str())
                .or_else(|| json.get("username").and_then(|u| u.as_str()))
                .map(String::from);
            AuthState {
                authenticated: true,
                username,
            }
        }
        Err(_) => AuthState {
            authenticated: false,
            username: None,
        },
    }
}

/// Command availability based on xurl + auth state.
fn build_commands_section(
    xurl_available: bool,
    authenticated: bool,
) -> HashMap<String, CommandStatus> {
    let mut cmds = HashMap::new();
    for &name in command_names_with_auth() {
        if name == "login" {
            cmds.insert(
                name.to_string(),
                CommandStatus {
                    available: xurl_available,
                    reason: if xurl_available {
                        None
                    } else {
                        Some("xurl not found. Install: brew install xdevplatform/tap/xurl".into())
                    },
                },
            );
            continue;
        }
        let reqs = match requirements_for_command(name) {
            Some(r) => r,
            None => continue,
        };
        let needs_auth = reqs
            .accepted
            .iter()
            .any(|at| !matches!(at, AuthType::None));
        let available = if needs_auth {
            xurl_available && authenticated
        } else {
            true
        };
        let reason = if !xurl_available {
            Some("xurl not found. Install: brew install xdevplatform/tap/xurl".into())
        } else if needs_auth && !authenticated {
            Some("not authenticated. Run `bird login`.".into())
        } else {
            None
        };
        cmds.insert(
            name.to_string(),
            CommandStatus { available, reason },
        );
    }
    cmds
}

/// Build full or scoped report.
pub(crate) fn report(
    client: &BirdClient,
    scope: Option<&str>,
) -> DoctorReport {
    let xurl = build_xurl_status();
    let auth = if xurl.available {
        detect_auth()
    } else {
        AuthState {
            authenticated: false,
            username: None,
        }
    };
    let mut commands = build_commands_section(xurl.available, auth.authenticated);
    if let Some(cmd) = scope {
        if let Some(status) = commands.remove(cmd) {
            commands.clear();
            commands.insert(cmd.to_string(), status);
        }
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

    DoctorReport {
        xurl,
        auth,
        commands,
        cache,
    }
}

fn format_pretty(report: &DoctorReport, use_color: bool, use_emoji: bool) -> String {
    use crate::output;
    let mut out = String::new();

    // Xurl section
    out.push_str(&format!("{}\n", output::section("Xurl", use_color)));
    if report.xurl.available {
        if let Some(ref path) = report.xurl.path {
            out.push_str(&format!(
                "  path: {}\n",
                output::muted(path, use_color)
            ));
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
        out.push_str("  Install: brew install xdevplatform/tap/xurl\n");
    }

    // Auth section
    out.push_str(&format!("\n{}\n", output::section("Auth", use_color)));
    if report.auth.authenticated {
        if let Some(ref username) = report.auth.username {
            out.push_str(&format!(
                "  user: {}\n",
                output::muted(&format!("@{}", username), use_color)
            ));
        }
        out.push_str(&format!(
            "  status: {}\n",
            output::success("authenticated", use_color)
        ));
    } else {
        out.push_str(&format!(
            "  status: {}\n",
            output::error("not authenticated", use_color)
        ));
        out.push_str("  Run `bird login` to authenticate.\n");
    }

    // Commands section
    out.push_str(&format!("\n{}\n", output::section("Commands", use_color)));
    let mut names: Vec<_> = report.commands.keys().collect();
    names.sort();
    for name in names {
        let status = report.commands.get(name).unwrap();
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
    }

    // Cache section
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
pub fn run_doctor(
    client: &BirdClient,
    pretty: bool,
    scope: Option<&str>,
    use_color: bool,
    use_emoji: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let r = report(client, scope);
    if pretty {
        println!("{}", format_pretty(&r, use_color, use_emoji));
    } else {
        println!("{}", serde_json::to_string(&r)?);
    }
    Ok(())
}

#[cfg(test)]
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
        )
    }

    #[test]
    fn doctor_report_has_commands() {
        let client = no_cache_client();
        let r = report(&client, None);
        assert!(!r.commands.is_empty());
        assert!(r.commands.contains_key("me"));
        assert!(r.commands.contains_key("login"));
    }

    #[test]
    fn doctor_report_scoped_has_only_that_command() {
        let client = no_cache_client();
        let r = report(&client, Some("me"));
        assert_eq!(r.commands.len(), 1);
        assert!(r.commands.contains_key("me"));
    }

    #[test]
    fn doctor_report_json_serializable() {
        let client = no_cache_client();
        let r = report(&client, None);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("xurl"));
        assert!(json.contains("auth"));
        assert!(json.contains("commands"));
    }

    #[test]
    fn build_commands_not_authenticated_auth_commands_unavailable() {
        let cmds = build_commands_section(true, false);
        // login should be available (xurl is present)
        assert!(cmds.get("login").unwrap().available);
        // me requires auth, should be unavailable
        assert!(!cmds.get("me").unwrap().available);
        assert!(cmds.get("me").unwrap().reason.as_ref().unwrap().contains("not authenticated"));
        // usage is local-only (AuthType::None), always available
        assert!(cmds.get("usage").unwrap().available);
    }

    #[test]
    fn build_commands_authenticated_all_available() {
        let cmds = build_commands_section(true, true);
        assert!(cmds.get("me").unwrap().available);
        assert!(cmds.get("bookmarks").unwrap().available);
        assert!(cmds.get("search").unwrap().available);
    }

    #[test]
    fn build_commands_no_xurl_all_auth_commands_unavailable() {
        let cmds = build_commands_section(false, false);
        assert!(!cmds.get("login").unwrap().available);
        assert!(!cmds.get("me").unwrap().available);
        // Local-only commands still available
        assert!(cmds.get("usage").unwrap().available);
    }
}
