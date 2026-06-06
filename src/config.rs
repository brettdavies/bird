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
    /// Active xurl app name resolved from `--app` / `BIRD_APP`. Empty
    /// `None` means "use xurl's stored default app". `XURL_APP` is not
    /// consulted; bird emits a migration warning when `XURL_APP` is set
    /// but `BIRD_APP` is not (R11 / R20 clause d).
    pub app: Option<String>,
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

/// Injectable path resolution. Captures what `dirs::config_dir()` returns so
/// tests can supply a temp directory instead of mutating `HOME` /
/// `XDG_CONFIG_HOME` at the process level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPaths {
    pub config_dir: PathBuf,
    /// Reserved for future xurl store injection. bird does not maintain a
    /// separate store today; mirrors `config_dir` from `from_env`.
    pub store_path: PathBuf,
}

impl ResolvedPaths {
    /// Resolve real paths from the host environment (production path).
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = dirs::config_dir().ok_or("no config dir")?.join("bird");
        let store_path = config_dir.clone();
        Ok(Self {
            config_dir,
            store_path,
        })
    }
}

/// Injectable environment overrides. Snapshots the process env at construction
/// so the loader is pure with respect to its inputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvOverrides {
    /// `BIRD_OUTPUT` raw string; clap parses later in the runner.
    pub output: Option<String>,
    /// `X_API_USERNAME` raw string (validated by callers before use).
    pub username: Option<String>,
    /// `NO_COLOR` presence — value is ignored per the spec.
    pub no_color: Option<bool>,
    /// `TERM` raw string.
    pub term: Option<String>,
    /// Currently main.rs argv-bound; U8 (runner) wires this through.
    pub timeout_secs: Option<u64>,
    /// `BIRD_NO_CACHE=1` → `Some(false)`; unset → `None` (load_with_paths
    /// applies the default-true at resolution time).
    pub cache_enabled: Option<bool>,
}

impl EnvOverrides {
    /// Read the host environment into an `EnvOverrides` snapshot.
    pub fn from_env() -> Self {
        Self {
            output: std::env::var("BIRD_OUTPUT").ok(),
            username: std::env::var("X_API_USERNAME").ok(),
            no_color: if std::env::var_os("NO_COLOR").is_some() {
                Some(true)
            } else {
                None
            },
            term: std::env::var("TERM").ok(),
            timeout_secs: None,
            cache_enabled: match std::env::var("BIRD_NO_CACHE").as_deref() {
                Ok("1") => Some(false),
                _ => None,
            },
        }
    }
}

impl ResolvedConfig {
    /// Build config with priority: args > file > env > default. Production
    /// shim — resolves paths and env from the host, then delegates to
    /// `load_with_paths`.
    pub fn load(
        arg_overrides: ArgOverrides,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::load_with_paths(
            arg_overrides,
            ResolvedPaths::from_env()?,
            EnvOverrides::from_env(),
        )
    }

    /// Resolve config against caller-supplied paths and env. Pure with respect
    /// to its inputs (no process-env reads, no host path lookups).
    pub fn load_with_paths(
        arg_overrides: ArgOverrides,
        paths: ResolvedPaths,
        env: EnvOverrides,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = paths.config_dir.clone();
        let config_path = config_dir.join("config.toml");

        let file_config: FileConfig = if config_path.exists() {
            let s = std::fs::read_to_string(&config_path)?;
            toml::from_str(&s).unwrap_or_default()
        } else {
            FileConfig::default()
        };

        // env.username (X_API_USERNAME) feeds env_username when ArgOverrides
        // didn't already set it. Args > file > env_username preserved.
        let env_username = arg_overrides.env_username.clone().or(env.username.clone());
        let username = arg_overrides
            .username
            .or(file_config.username)
            .or(env_username);

        let cache_path = config_dir.join("bird.db");
        let cache_enabled = env.cache_enabled.unwrap_or(true);

        Ok(ResolvedConfig {
            username,
            config_dir,
            cache_path,
            cache_enabled,
            cache_max_size_mb: 100,
            app: None,
        })
    }
}

// Plan 1 R19: compile-time guard that the four public config types stay
// `Send + Sync`. They cross the writer-injection boundary in Plan 2 (passed
// into `run_with_paths` and stored on `BirdClient`'s descendants).
const _: fn() = || {
    fn _assert_send_sync<T: Send + Sync>() {}
    _assert_send_sync::<ResolvedConfig>();
    _assert_send_sync::<ResolvedPaths>();
    _assert_send_sync::<ArgOverrides>();
    _assert_send_sync::<EnvOverrides>();
};

#[cfg(test)]
mod tests {
    use super::*;

    fn rand_suffix() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn test_paths() -> ResolvedPaths {
        let tmp = std::env::temp_dir().join(format!(
            "bird-test-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        ResolvedPaths {
            config_dir: tmp.clone(),
            store_path: tmp,
        }
    }

    #[test]
    fn from_env_returns_paths_under_config_dir() {
        let p = ResolvedPaths::from_env().expect("from_env succeeds on this host");
        assert!(p.config_dir.ends_with("bird"));
    }

    #[test]
    fn load_with_paths_honors_cache_enabled_override() {
        let paths = test_paths();
        let env = EnvOverrides {
            cache_enabled: Some(true),
            ..Default::default()
        };
        let cfg = ResolvedConfig::load_with_paths(ArgOverrides::default(), paths.clone(), env)
            .expect("load_with_paths succeeds");
        assert!(cfg.cache_enabled);
        let env_false = EnvOverrides {
            cache_enabled: Some(false),
            ..Default::default()
        };
        let cfg2 = ResolvedConfig::load_with_paths(ArgOverrides::default(), paths, env_false)
            .expect("load_with_paths succeeds");
        assert!(!cfg2.cache_enabled);
    }

    #[test]
    fn load_with_paths_uses_paths_config_dir() {
        let paths = test_paths();
        let cfg = ResolvedConfig::load_with_paths(
            ArgOverrides::default(),
            paths.clone(),
            EnvOverrides::default(),
        )
        .expect("load_with_paths succeeds");
        assert_eq!(cfg.config_dir, paths.config_dir);
        assert_eq!(cfg.cache_path, paths.config_dir.join("bird.db"));
    }

    #[test]
    fn load_is_shim_for_load_with_paths() {
        // Production shim must succeed and produce a usable ResolvedConfig on
        // a sane host. Host-dependent paths vary, so only assert the call
        // returns Ok with the expected shape.
        let cfg = ResolvedConfig::load(ArgOverrides::default()).expect("load succeeds");
        assert!(cfg.config_dir.ends_with("bird"));
        assert_eq!(cfg.cache_path, cfg.config_dir.join("bird.db"));
        assert_eq!(cfg.cache_max_size_mb, 100);
    }

    #[test]
    fn env_overrides_construction_shape() {
        // Direct struct construction proves the field shape without racing
        // other tests on process env. EnvOverrides::from_env() integration is
        // covered by the production-path shim test above.
        let env = EnvOverrides {
            no_color: Some(true),
            term: Some("dumb".into()),
            ..Default::default()
        };
        assert_eq!(env.no_color, Some(true));
        assert_eq!(env.term.as_deref(), Some("dumb"));
        assert!(env.output.is_none());
        assert!(env.username.is_none());
        assert!(env.timeout_secs.is_none());
        assert!(env.cache_enabled.is_none());
    }
}
