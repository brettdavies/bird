//! Config load with priority: command args > config file > env > default (build-in).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::skill_install::{BASE_DIR_VARS, CONFIG_DIR_VARS, SkillEnv};
use crate::store::StorePaths;
use crate::store::paths::ARCHIVE_DIR_VAR;

/// Resolved config after applying priority: args > config file > env > default.
/// Auth is delegated to xdk-rs; no token fields here.
#[derive(Clone, Debug)]
pub struct ResolvedConfig {
    pub username: Option<String>,
    pub config_dir: PathBuf,
    pub cache_enabled: bool,
    /// The file store's locations: archive, response files, and ledger.
    pub store: StorePaths,
    /// The xdk-rs token store, from [`ResolvedPaths::token_store`].
    pub token_store: Option<PathBuf>,
    /// The environment xdk-rs reads, from [`EnvOverrides::xdk`].
    pub xdk_env: xdk::config::EnvOverrides,
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
    /// Where the Markdown archive lives; `BIRD_ARCHIVE_DIR` wins over it.
    pub archive_dir: Option<String>,
}

/// A `config.toml` that does not parse, with the parser's position in it.
#[derive(Debug)]
pub struct ConfigParseError {
    pub path: PathBuf,
    /// The 1-based line and column the parser stopped at, when it named one.
    pub position: Option<(usize, usize)>,
    /// The parser's reason, without its position or source excerpt.
    pub reason: String,
}

impl ConfigParseError {
    fn new(path: &std::path::Path, source: &str, err: &toml::de::Error) -> Self {
        let position = err.span().map(|span| {
            let before = source.get(..span.start).unwrap_or(source);
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
            (line, column)
        });
        Self {
            path: path.to_path_buf(),
            position,
            reason: err.message().to_string(),
        }
    }

    /// The line telling a person what to change.
    pub fn fix(&self) -> String {
        match self.position {
            Some((line, column)) => format!(
                "Fix line {line}, column {column} of {}, then run the command again.",
                self.path.display()
            ),
            None => format!(
                "Fix the TOML in {}, then run the command again.",
                self.path.display()
            ),
        }
    }
}

impl std::fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config.toml at {} does not parse", self.path.display())?;
        if let Some((line, column)) = self.position {
            write!(f, " at line {line}, column {column}")?;
        }
        write!(f, ": {}", self.reason)
    }
}

impl std::error::Error for ConfigParseError {}

/// Overrides from CLI args (highest priority).
#[derive(Clone, Debug, Default)]
pub struct ArgOverrides {
    pub username: Option<String>,
    /// Env var fallback (lowest priority, below config file).
    pub env_username: Option<String>,
}

/// Injectable path resolution. Captures what `dirs::config_dir()`,
/// `dirs::data_dir()`, `dirs::cache_dir()`, and `dirs::home_dir()` return so
/// tests can supply a temp directory instead of mutating `HOME` /
/// `XDG_CONFIG_HOME` at the process level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPaths {
    /// bird's config directory, `bird` included.
    pub config_dir: PathBuf,
    /// The platform data directory; the archive and ledger sit under its
    /// `bird` folder.
    pub data_dir: PathBuf,
    /// The platform cache directory; response files sit under its `bird`
    /// folder.
    pub cache_dir: PathBuf,
    /// The home directory a leading `~` in `archive_dir` expands against.
    pub home_dir: Option<PathBuf>,
    /// The xdk-rs token store `xr` shares, when one is named: `XURL_TOKEN_STORE`,
    /// or a test's temporary store. `None` opens xdk's default store, where
    /// `xr` keeps it. The OAuth2 pending state sits beside the store.
    pub token_store: Option<PathBuf>,
}

impl ResolvedPaths {
    /// Resolve real paths from the host environment (production path).
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = dirs::config_dir().ok_or("no config dir")?.join("bird");
        let data_dir = dirs::data_dir().ok_or("no data dir")?;
        let cache_dir = dirs::cache_dir().ok_or("no cache dir")?;
        let token_store = std::env::var_os("XURL_TOKEN_STORE")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        Ok(Self {
            config_dir,
            data_dir,
            cache_dir,
            home_dir: dirs::home_dir(),
            token_store,
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
    /// The environment xdk-rs reads (`CLIENT_ID`, `CLIENT_SECRET`,
    /// `XURL_BEARER_TOKEN`, the endpoint URLs), plus the `HOME` the skill
    /// installer expands `~` against, captured once. With a named token store
    /// nothing below the runner reads the process environment. The default is
    /// an empty environment.
    pub xdk: xdk::config::EnvOverrides,
    /// `BIRD_ARCHIVE_DIR` raw string; empty counts as unset.
    pub archive_dir: Option<String>,
    /// `BIRD_SKILL_HOME`, which stands in for `HOME` in skill destinations.
    pub skill_home: Option<String>,
    /// Each skill host's config- or base-directory variable that is set, by
    /// name ([`CONFIG_DIR_VARS`], [`BASE_DIR_VARS`]).
    pub skill_dirs: BTreeMap<String, String>,
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
            xdk: xdk::config::EnvOverrides {
                home: std::env::var("HOME").ok(),
                ..xdk::config::EnvOverrides::from_env()
            },
            archive_dir: std::env::var(ARCHIVE_DIR_VAR).ok(),
            skill_home: std::env::var("BIRD_SKILL_HOME").ok(),
            skill_dirs: CONFIG_DIR_VARS
                .iter()
                .chain(BASE_DIR_VARS)
                .filter_map(|var| {
                    std::env::var(var)
                        .ok()
                        .map(|value| ((*var).to_string(), value))
                })
                .collect(),
        }
    }

    /// The environment a skill destination resolves against: the captured
    /// `HOME`, `BIRD_SKILL_HOME`, and host directory variables.
    pub fn skill_env(&self) -> SkillEnv {
        SkillEnv {
            home: self.xdk.home.clone(),
            skill_home: self.skill_home.clone(),
            config_dirs: self.skill_dirs.clone(),
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
            toml::from_str(&s).map_err(|e| ConfigParseError::new(&config_path, &s, &e))?
        } else {
            FileConfig::default()
        };
        let store = StorePaths::resolve(&paths, &env, &file_config)?;

        // env.username (X_API_USERNAME) feeds env_username when ArgOverrides
        // didn't already set it. Args > file > env_username preserved.
        let env_username = arg_overrides.env_username.clone().or(env.username.clone());
        let username = arg_overrides
            .username
            .or(file_config.username)
            .or(env_username);

        let cache_enabled = env.cache_enabled.unwrap_or(true);

        Ok(ResolvedConfig {
            username,
            config_dir,
            cache_enabled,
            store,
            token_store: paths.token_store,
            xdk_env: env.xdk,
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
            data_dir: tmp.join("data"),
            cache_dir: tmp.join("cache"),
            home_dir: None,
            token_store: Some(tmp.join("auth.yml")),
        }
    }

    /// The column counts characters, not bytes, so a non-ASCII value before
    /// the error does not shift it.
    #[test]
    fn a_parse_error_reports_its_line_and_character_column() {
        let source = "username = \"jack\"\na = \"é\" x\n";
        let err = toml::from_str::<FileConfig>(source).expect_err("trailing word");
        let parse = ConfigParseError::new(std::path::Path::new("/c/config.toml"), source, &err);
        assert_eq!(parse.position, Some((2, 9)));
        assert!(
            parse.to_string().contains("at line 2, column 9: "),
            "{parse}"
        );
        assert!(
            parse
                .fix()
                .starts_with("Fix line 2, column 9 of /c/config.toml")
        );
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
        assert!(cfg.store.archive_dir.starts_with(&paths.data_dir));
    }

    #[test]
    fn load_is_shim_for_load_with_paths() {
        // Production shim must succeed and produce a usable ResolvedConfig on
        // a sane host. Host-dependent paths vary, so only assert the call
        // returns Ok with the expected shape.
        let cfg = ResolvedConfig::load(ArgOverrides::default()).expect("load succeeds");
        assert!(cfg.config_dir.ends_with("bird"));
    }

    #[test]
    fn malformed_config_toml_is_a_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("config.toml"), "archive_dir = \"/v\n").expect("write");
        let paths = ResolvedPaths {
            config_dir: dir.path().to_path_buf(),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
            home_dir: None,
            token_store: None,
        };
        let err = ResolvedConfig::load_with_paths(
            ArgOverrides::default(),
            paths,
            EnvOverrides::default(),
        )
        .expect_err("a config.toml that does not parse is refused");
        assert!(err.to_string().contains("config.toml"), "{err}");
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
