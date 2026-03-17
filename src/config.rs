//! Config load with priority: command args > config file > env > default (build-in).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Resolved config after applying priority: args > config file > env > default.
/// Auth is fully delegated to xurl — no token fields here.
#[derive(Clone, Debug)]
pub struct ResolvedConfig {
    pub username: Option<String>,
    pub config_dir: PathBuf,
    pub cache_path: PathBuf,
    pub cache_enabled: bool,
    pub cache_max_size_mb: u64,
}

/// File-backed config (what we read from ~/.config/bird/config.toml).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FileConfig {
    pub username: Option<String>,
    pub watchlist: Option<Vec<String>>,
}

/// Overrides from CLI args (highest priority).
#[derive(Clone, Debug, Default)]
pub struct ArgOverrides {
    pub username: Option<String>,
    /// Env var fallback (lowest priority, below config file).
    pub env_username: Option<String>,
}

impl ResolvedConfig {
    /// Build config with priority: args > file > env > default.
    pub fn load(
        arg_overrides: ArgOverrides,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = dirs::config_dir().ok_or("no config dir")?.join("bird");
        let config_path = config_dir.join("config.toml");

        let file_config: FileConfig = if config_path.exists() {
            let s = std::fs::read_to_string(&config_path)?;
            toml::from_str(&s).unwrap_or_default()
        } else {
            FileConfig::default()
        };

        let username = arg_overrides
            .username
            .or(file_config.username)
            .or(arg_overrides.env_username);

        let cache_path = config_dir.join("bird.db");
        let cache_enabled = std::env::var("BIRD_NO_CACHE").as_deref() != Ok("1");

        Ok(ResolvedConfig {
            username,
            config_dir: config_dir.clone(),
            cache_path,
            cache_enabled,
            cache_max_size_mb: 100,
        })
    }
}
