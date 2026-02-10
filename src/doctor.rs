//! bird doctor: living view of auth state, effective config (with source), and which commands are usable.

use crate::auth::{load_stored_tokens, resolve_bearer_token, resolve_oauth2_token};
use crate::config::{FileConfig, ResolvedConfig, DEFAULT_REDIRECT_URI};
use crate::requirements::{requirements_for_command, AuthType, command_names_with_auth, reason_for_unavailable};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Source of a config value: arg not used by doctor (we load with empty overrides).
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    Env,
    Config,
    Default,
}

/// Entry for a non-secret config key (value + source).
#[derive(Clone, Debug, Serialize)]
pub struct ConfigValue {
    pub value: String,
    pub source: ConfigSource,
}

/// Entry for a secret (set + source, no value).
#[derive(Clone, Debug, Serialize)]
pub struct ConfigSecret {
    pub set: bool,
    pub source: Option<ConfigSource>,
}

/// Where the active auth came from.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthSource {
    Env,
    Config,
    Stored,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthState {
    pub auth_type: AuthType,
    pub source: Option<AuthSource>,
    pub username: Option<String>,
    pub can_refresh: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub auth: AuthState,
    pub config: HashMap<String, serde_json::Value>,
    pub commands: HashMap<String, CommandStatus>,
}

fn env_set(key: &str) -> bool {
    std::env::var(key).is_ok()
}

fn file_config(config_dir: &Path) -> FileConfig {
    let config_path = config_dir.join("config.toml");
    if config_path.exists() {
        if let Ok(s) = std::fs::read_to_string(&config_path) {
            return toml::from_str(&s).unwrap_or_default();
        }
    }
    FileConfig::default()
}

fn source_client_id(file: &FileConfig) -> ConfigSource {
    if env_set("X_API_CLIENT_ID") {
        ConfigSource::Env
    } else if file.client_id.is_some() {
        ConfigSource::Config
    } else {
        ConfigSource::Default
    }
}

fn source_client_secret(file: &FileConfig) -> ConfigSource {
    if env_set("X_API_CLIENT_SECRET") {
        ConfigSource::Env
    } else if file.client_secret.is_some() {
        ConfigSource::Config
    } else {
        ConfigSource::Default
    }
}

fn source_redirect_uri(file: &FileConfig) -> (String, ConfigSource) {
    if env_set("X_API_REDIRECT_URI") {
        (
            std::env::var("X_API_REDIRECT_URI").unwrap_or_default(),
            ConfigSource::Env,
        )
    } else if let Some(ref u) = file.redirect_uri {
        (u.clone(), ConfigSource::Config)
    } else {
        (DEFAULT_REDIRECT_URI.to_string(), ConfigSource::Default)
    }
}

fn source_username(file: &FileConfig) -> (Option<String>, Option<ConfigSource>) {
    if env_set("X_API_USERNAME") {
        (
            std::env::var("X_API_USERNAME").ok(),
            Some(ConfigSource::Env),
        )
    } else if file.username.is_some() {
        (file.username.clone(), Some(ConfigSource::Config))
    } else {
        (None, Some(ConfigSource::Default))
    }
}

fn build_config_section(config: &ResolvedConfig, file: &FileConfig) -> HashMap<String, serde_json::Value> {
    let mut out = HashMap::new();

    let (redirect_uri, redirect_source) = source_redirect_uri(file);
    out.insert(
        "redirect_uri".to_string(),
        serde_json::to_value(ConfigValue {
            value: redirect_uri,
            source: redirect_source,
        })
        .unwrap(),
    );

    out.insert(
        "client_id".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.client_id.is_some(),
            source: Some(source_client_id(file)),
        })
        .unwrap(),
    );
    out.insert(
        "client_secret".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.client_secret.is_some(),
            source: Some(source_client_secret(file)),
        })
        .unwrap(),
    );

    let (username_val, username_src) = source_username(file);
    if let Some(src) = username_src {
        out.insert(
            "username".to_string(),
            serde_json::to_value(serde_json::json!({
                "value": username_val,
                "source": src
            }))
            .unwrap(),
        );
    }

    let access_token_set = config.access_token.is_some() || env_set("X_API_ACCESS_TOKEN");
    out.insert(
        "access_token".to_string(),
        serde_json::to_value(ConfigSecret {
            set: access_token_set,
            source: Some(if access_token_set {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );
    let bearer_set = config.bearer_token.is_some() || env_set("X_API_BEARER_TOKEN");
    out.insert(
        "bearer_token".to_string(),
        serde_json::to_value(ConfigSecret {
            set: bearer_set,
            source: Some(if bearer_set {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );
    out.insert(
        "oauth1_consumer_key".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.oauth1_consumer_key.is_some(),
            source: Some(if env_set("X_API_CONSUMER_KEY") {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );
    out.insert(
        "oauth1_consumer_secret".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.oauth1_consumer_secret.is_some(),
            source: Some(if env_set("X_API_CONSUMER_SECRET") {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );
    out.insert(
        "oauth1_access_token".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.oauth1_access_token.is_some(),
            source: Some(if env_set("X_API_OAUTH1_ACCESS_TOKEN") {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );
    out.insert(
        "oauth1_access_token_secret".to_string(),
        serde_json::to_value(ConfigSecret {
            set: config.oauth1_access_token_secret.is_some(),
            source: Some(if env_set("X_API_OAUTH1_ACCESS_TOKEN_SECRET") {
                ConfigSource::Env
            } else {
                ConfigSource::Default
            }),
        })
        .unwrap(),
    );

    out
}

fn build_auth_state(config: &ResolvedConfig) -> AuthState {
    let stored = load_stored_tokens(&config.tokens_path);
    let oauth2 = resolve_oauth2_token(config, stored.as_ref());
    let bearer = resolve_bearer_token(config);
    let oauth1_all = config.oauth1_consumer_key.is_some()
        && config.oauth1_consumer_secret.is_some()
        && config.oauth1_access_token.is_some()
        && config.oauth1_access_token_secret.is_some();

    let (auth_type, source, username, can_refresh) = if bearer.is_some() {
        (
            AuthType::Bearer,
            Some(AuthSource::Env),
            config.username.clone(),
            false,
        )
    } else if oauth1_all {
        (
            AuthType::OAuth1,
            Some(AuthSource::Env),
            config.username.clone(),
            false,
        )
    } else if let Some((_, refresh_opt)) = oauth2 {
        let from_stored = config.access_token.is_none() && !env_set("X_API_ACCESS_TOKEN");
        let source = if config.access_token.is_some() || env_set("X_API_ACCESS_TOKEN") {
            Some(AuthSource::Env)
        } else if from_stored {
            Some(AuthSource::Stored)
        } else {
            Some(AuthSource::Env)
        };
        let username = config
            .username
            .clone()
            .or_else(|| stored.as_ref().and_then(|s| s.accounts.keys().next().cloned()));
        let can_refresh = refresh_opt.is_some() && config.client_id.is_some();
        (AuthType::OAuth2User, source, username, can_refresh)
    } else {
        (
            AuthType::None,
            None,
            None,
            false,
        )
    };

    AuthState {
        auth_type,
        source,
        username,
        can_refresh,
    }
}

/// Command availability from centralized requirements (openapi/x-api-openapi.json).
fn build_commands_section(config: &ResolvedConfig, auth: &AuthState) -> HashMap<String, CommandStatus> {
    let has_oauth2_user = auth.auth_type == AuthType::OAuth2User;
    let has_oauth1 = auth.auth_type == AuthType::OAuth1;
    let has_bearer = auth.auth_type == AuthType::Bearer;

    let mut cmds = HashMap::new();
    for &name in command_names_with_auth() {
        if name == "login" {
            let available = config.client_id.is_some();
            cmds.insert(
                name.to_string(),
                CommandStatus {
                    available,
                    reason: if available {
                        Some("client_id is set (default or override; optional X_API_CLIENT_SECRET for your own app)".to_string())
                    } else {
                        Some("set X_API_CLIENT_ID".to_string())
                    },
                },
            );
            continue;
        }
        let reqs = match requirements_for_command(name) {
            Some(r) => r,
            None => continue,
        };
        let available = reqs.accepted.iter().any(|at| match at {
            AuthType::OAuth2User => has_oauth2_user,
            AuthType::OAuth1 => has_oauth1,
            AuthType::Bearer => has_bearer,
            AuthType::None => false,
        });
        cmds.insert(
            name.to_string(),
            CommandStatus {
                available,
                reason: if available {
                    None
                } else {
                    Some(reason_for_unavailable(&reqs))
                },
            },
        );
    }
    cmds
}

/// Build full or scoped report. When scope is Some("me"), only that command appears in report.commands.
pub(crate) fn report(config: &ResolvedConfig, scope: Option<&str>) -> DoctorReport {
    let file = file_config(&config.config_dir);
    let auth = build_auth_state(config);
    let mut commands = build_commands_section(config, &auth);
    if let Some(cmd) = scope {
        if let Some(status) = commands.remove(cmd) {
            commands.clear();
            commands.insert(cmd.to_string(), status);
        }
    }
    DoctorReport {
        auth: auth.clone(),
        config: build_config_section(config, &file),
        commands,
    }
}

fn format_pretty(report: &DoctorReport, use_color: bool, use_emoji: bool) -> String {
    use crate::output;
    let mut out = String::new();
    out.push_str(&format!("{}\n", output::section("Auth", use_color)));
    let type_str = serde_json::to_string(&report.auth.auth_type).unwrap_or_else(|_| "unknown".into());
    out.push_str(&format!("  type: {}\n", output::muted(type_str.trim_matches('"'), use_color)));
    if let Some(ref s) = report.auth.source {
        let src_str = serde_json::to_string(s).unwrap_or_else(|_| "?".into());
        out.push_str(&format!("  source: {}\n", output::muted(src_str.trim_matches('"'), use_color)));
    }
    if let Some(ref u) = report.auth.username {
        out.push_str(&format!("  username: {}\n", output::muted(u, use_color)));
    }
    out.push_str(&format!(
        "  can_refresh: {}\n",
        output::muted(&report.auth.can_refresh.to_string(), use_color)
    ));

    out.push_str(&format!("\n{}\n", output::section("Config (sources)", use_color)));
    for (k, v) in &report.config {
        out.push_str(&format!("  {}: {}\n", k, serde_json::to_string(v).unwrap_or_default()));
    }

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
                format!("{}{}", output::error("unavailable: ", use_color), output::muted(reason, use_color)),
            )
        };
        out.push_str(&format!("  {}: {}{}\n", output::command(name, use_color), emoji, r));
    }
    out
}

/// Run doctor: build report and print JSON (compact) or human summary. When scope is Some("me"), report only that command.
pub fn run_doctor(
    config: &ResolvedConfig,
    pretty: bool,
    scope: Option<&str>,
    use_color: bool,
    use_emoji: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let r = report(config, scope);
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
    use crate::config::DEFAULT_REDIRECT_URI;

    /// Unset auth-related env vars so report() only sees the config we pass (no env leakage).
    fn clear_auth_env() {
        std::env::remove_var("X_API_ACCESS_TOKEN");
        std::env::remove_var("X_API_REFRESH_TOKEN");
        std::env::remove_var("X_API_BEARER_TOKEN");
        std::env::remove_var("X_API_CONSUMER_KEY");
        std::env::remove_var("X_API_CONSUMER_SECRET");
        std::env::remove_var("X_API_OAUTH1_ACCESS_TOKEN");
        std::env::remove_var("X_API_OAUTH1_ACCESS_TOKEN_SECRET");
    }

    fn minimal_config_no_auth() -> ResolvedConfig {
        let config_dir = std::env::temp_dir().join("bird-doctor-test");
        ResolvedConfig {
            client_id: None,
            client_secret: None,
            redirect_uri: DEFAULT_REDIRECT_URI.to_string(),
            access_token: None,
            refresh_token: None,
            bearer_token: None,
            username: None,
            oauth1_consumer_key: None,
            oauth1_consumer_secret: None,
            oauth1_access_token: None,
            oauth1_access_token_secret: None,
            config_dir: config_dir.clone(),
            tokens_path: config_dir.join("tokens.json"),
        }
    }

    #[test]
    fn doctor_report_no_auth_has_none_type_and_me_unavailable() {
        clear_auth_env();
        let config = minimal_config_no_auth();
        let r = report(&config, None);
        assert_eq!(r.auth.auth_type, AuthType::None);
        assert!(!r.commands.get("me").unwrap().available);
        assert!(!r.commands.get("login").unwrap().available);
    }

    #[test]
    fn doctor_report_json_contains_no_secret_values() {
        let mut config = minimal_config_no_auth();
        config.access_token = Some("secret-token-value".to_string());
        let r = report(&config, None);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("secret-token-value"));
        assert!(!json.contains("Bearer "));
    }

    #[test]
    fn doctor_report_with_access_token_has_me_and_bookmarks_available() {
        clear_auth_env();
        let mut config = minimal_config_no_auth();
        config.access_token = Some("test-token".to_string());
        let r = report(&config, None);
        assert_eq!(r.auth.auth_type, AuthType::OAuth2User);
        assert!(r.commands.get("me").unwrap().available);
        assert!(r.commands.get("bookmarks").unwrap().available);
    }

    #[test]
    fn doctor_report_oauth1_me_available_bookmarks_not() {
        clear_auth_env();
        let mut config = minimal_config_no_auth();
        config.oauth1_consumer_key = Some("ck".into());
        config.oauth1_consumer_secret = Some("cs".into());
        config.oauth1_access_token = Some("at".into());
        config.oauth1_access_token_secret = Some("ats".into());
        let r = report(&config, None);
        assert_eq!(r.auth.auth_type, AuthType::OAuth1);
        assert!(r.commands.get("me").unwrap().available, "me accepts OAuth 1.0a per spec");
        assert!(
            !r.commands.get("bookmarks").unwrap().available,
            "bookmarks requires OAuth 2.0 user only per spec"
        );
    }

    #[test]
    fn doctor_report_bearer_me_and_bookmarks_unavailable() {
        let mut config = minimal_config_no_auth();
        config.bearer_token = Some("bearer".into());
        let r = report(&config, None);
        assert_eq!(r.auth.auth_type, AuthType::Bearer);
        assert!(!r.commands.get("me").unwrap().available);
        assert!(!r.commands.get("bookmarks").unwrap().available);
    }

    #[test]
    fn doctor_report_scoped_has_only_that_command() {
        let config = minimal_config_no_auth();
        let r = report(&config, Some("me"));
        assert_eq!(r.commands.len(), 1);
        assert!(r.commands.contains_key("me"));
    }
}
