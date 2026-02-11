//! Config load with priority: command args > config file > env > default (build-in).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8765/callback";
/// Baked-in OAuth2 client_id for development (public client). Used when BIRD_DEFAULT_CLIENT_ID is not set at build time.
const OAUTH2_CLIENT_ID_DEV: &str = "Mkt1TFoyazFqdkpiSFJRdHVqVGw6MTpjaQ";
pub const AUTHORIZE_URL: &str = "https://x.com/i/oauth2/authorize";
pub const TOKEN_URL: &str = "https://api.x.com/2/oauth2/token";

/// Resolved config after applying priority: args > config file > env > default.
#[derive(Clone)]
pub struct ResolvedConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub bearer_token: Option<String>,
    pub username: Option<String>,
    /// OAuth 1.0a consumer key (for app-only or user context)
    pub oauth1_consumer_key: Option<String>,
    /// OAuth 1.0a consumer secret
    pub oauth1_consumer_secret: Option<String>,
    /// OAuth 1.0a access token
    pub oauth1_access_token: Option<String>,
    /// OAuth 1.0a access token secret
    pub oauth1_access_token_secret: Option<String>,
    pub config_dir: PathBuf,
    pub tokens_path: PathBuf,
    pub cache_path: PathBuf,
    pub cache_enabled: bool,
    pub cache_max_size_mb: u64,
}

impl std::fmt::Debug for ResolvedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedConfig")
            .field("client_id", &self.client_id.as_ref().map(|_| "[REDACTED]"))
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("redirect_uri", &self.redirect_uri)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("username", &self.username)
            .field(
                "oauth1_consumer_key",
                &self.oauth1_consumer_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "oauth1_consumer_secret",
                &self.oauth1_consumer_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "oauth1_access_token",
                &self.oauth1_access_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "oauth1_access_token_secret",
                &self
                    .oauth1_access_token_secret
                    .as_ref()
                    .map(|_| "[REDACTED]"),
            )
            .field("config_dir", &self.config_dir)
            .field("tokens_path", &self.tokens_path)
            .field("cache_path", &self.cache_path)
            .field("cache_enabled", &self.cache_enabled)
            .field("cache_max_size_mb", &self.cache_max_size_mb)
            .finish()
    }
}

/// File-backed config (what we read from ~/.config/bird/config.toml).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FileConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub username: Option<String>,
}

/// Overrides from CLI args (highest priority).
#[derive(Clone, Debug, Default)]
pub struct ArgOverrides {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub bearer_token: Option<String>,
    pub username: Option<String>,
    pub oauth1_consumer_key: Option<String>,
    pub oauth1_consumer_secret: Option<String>,
    pub oauth1_access_token: Option<String>,
    pub oauth1_access_token_secret: Option<String>,
}

impl ResolvedConfig {
    /// Build config with priority: args > file > env > default.
    pub fn load(
        arg_overrides: ArgOverrides,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = dirs::config_dir().ok_or("no config dir")?.join("bird");
        let tokens_path = config_dir.join("tokens.json");
        let config_path = config_dir.join("config.toml");

        let file_config: FileConfig = if config_path.exists() {
            let s = std::fs::read_to_string(&config_path)?;
            toml::from_str(&s).unwrap_or_default()
        } else {
            FileConfig::default()
        };

        let redirect_uri = file_config
            .redirect_uri
            .clone()
            .or_else(|| std::env::var("X_API_REDIRECT_URI").ok())
            .unwrap_or_else(|| DEFAULT_REDIRECT_URI.to_string());

        let default_client_id =
            option_env!("BIRD_DEFAULT_CLIENT_ID").unwrap_or(OAUTH2_CLIENT_ID_DEV);
        let client_id = arg_overrides
            .client_id
            .or(file_config.client_id)
            .or_else(|| std::env::var("X_API_CLIENT_ID").ok())
            .or_else(|| Some(default_client_id.to_string()));

        let client_secret = arg_overrides
            .client_secret
            .or(file_config.client_secret)
            .or_else(|| std::env::var("X_API_CLIENT_SECRET").ok());

        let access_token = arg_overrides
            .access_token
            .or_else(|| std::env::var("X_API_ACCESS_TOKEN").ok());

        let refresh_token = arg_overrides
            .refresh_token
            .or_else(|| std::env::var("X_API_REFRESH_TOKEN").ok());

        let bearer_token = arg_overrides
            .bearer_token
            .or_else(|| std::env::var("X_API_BEARER_TOKEN").ok());

        let username = arg_overrides
            .username
            .or(file_config.username)
            .or_else(|| std::env::var("X_API_USERNAME").ok());

        let oauth1_consumer_key = arg_overrides
            .oauth1_consumer_key
            .or_else(|| std::env::var("X_API_CONSUMER_KEY").ok());
        let oauth1_consumer_secret = arg_overrides
            .oauth1_consumer_secret
            .or_else(|| std::env::var("X_API_CONSUMER_SECRET").ok());
        let oauth1_access_token = arg_overrides
            .oauth1_access_token
            .or_else(|| std::env::var("X_API_OAUTH1_ACCESS_TOKEN").ok());
        let oauth1_access_token_secret = arg_overrides
            .oauth1_access_token_secret
            .or_else(|| std::env::var("X_API_OAUTH1_ACCESS_TOKEN_SECRET").ok());

        let cache_path = config_dir.join("cache.db");
        let cache_enabled = std::env::var("BIRD_NO_CACHE").as_deref() != Ok("1");

        Ok(ResolvedConfig {
            client_id,
            client_secret,
            redirect_uri,
            access_token,
            refresh_token,
            bearer_token,
            username,
            oauth1_consumer_key,
            oauth1_consumer_secret,
            oauth1_access_token,
            oauth1_access_token_secret,
            config_dir: config_dir.clone(),
            tokens_path,
            cache_path,
            cache_enabled,
            cache_max_size_mb: 100,
        })
    }

    /// Ensure config directory exists.
    pub fn ensure_config_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)
    }
}
