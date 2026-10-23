//! Where the file store lives: the archive, the response files, and the spend
//! ledger, resolved from captured inputs only.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{EnvOverrides, FileConfig, ResolvedPaths};

/// The `config.toml` key that moves the archive.
pub const ARCHIVE_DIR_KEY: &str = "archive_dir";

/// The variable that moves the archive, ahead of [`ARCHIVE_DIR_KEY`].
pub const ARCHIVE_DIR_VAR: &str = "BIRD_ARCHIVE_DIR";

/// The recovery line for a refused archive directory.
pub const ARCHIVE_DIR_FIX: &str =
    "Set archive_dir in config.toml, or BIRD_ARCHIVE_DIR, to an absolute directory.";

/// Which input chose the archive directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveSource {
    /// Neither the variable nor the key was set.
    Default,
    /// `archive_dir` in `config.toml`.
    Config,
    /// `BIRD_ARCHIVE_DIR`.
    Env,
}

impl ArchiveSource {
    /// The name the source is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Config => "config",
            Self::Env => "env",
        }
    }

    fn setting(self) -> &'static str {
        match self {
            Self::Env => ARCHIVE_DIR_VAR,
            _ => ARCHIVE_DIR_KEY,
        }
    }
}

/// The three store locations: notes, response files, and the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorePaths {
    /// The Markdown archive; a user may point it into a notes vault.
    pub archive_dir: PathBuf,
    /// Which input chose [`Self::archive_dir`].
    pub archive_source: ArchiveSource,
    /// The per-account response files, under the cache directory.
    pub responses_dir: PathBuf,
    /// The JSONL spend ledger, under the data directory.
    pub ledger: PathBuf,
}

impl StorePaths {
    /// Resolve the store from the captured paths, environment, and config
    /// file. `BIRD_ARCHIVE_DIR` beats `archive_dir`, which beats the default;
    /// an empty variable counts as unset.
    pub fn resolve(
        paths: &ResolvedPaths,
        env: &EnvOverrides,
        file: &FileConfig,
    ) -> Result<Self, ArchiveDirError> {
        let data = paths.data_dir.join("bird");
        let chosen = env
            .archive_dir
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|v| (v, ArchiveSource::Env))
            .or_else(|| {
                file.archive_dir
                    .as_deref()
                    .map(|v| (v, ArchiveSource::Config))
            });
        let (archive_dir, archive_source) = match chosen {
            Some((value, source)) => (expand(value, paths.home_dir.as_deref(), source)?, source),
            None => (data.join("archive"), ArchiveSource::Default),
        };
        Ok(Self {
            archive_dir,
            archive_source,
            responses_dir: paths.cache_dir.join("bird").join("responses"),
            ledger: data.join("spend.jsonl"),
        })
    }

    /// Every location under `root`, for tests that build a config by hand.
    pub fn under(root: &Path) -> Self {
        Self {
            archive_dir: root.join("archive"),
            archive_source: ArchiveSource::Default,
            responses_dir: root.join("responses"),
            ledger: root.join("spend.jsonl"),
        }
    }

    /// The sibling file appends and compaction lock.
    pub fn ledger_lock(&self) -> PathBuf {
        self.ledger.with_file_name("spend.lock")
    }
}

/// Expands a leading `~` against `home` and refuses any other relative path,
/// or a path that names something other than a directory.
fn expand(
    value: &str,
    home: Option<&Path>,
    source: ArchiveSource,
) -> Result<PathBuf, ArchiveDirError> {
    let refuse = |problem| ArchiveDirError {
        value: value.to_string(),
        source,
        problem,
    };
    let rest = if value == "~" {
        Some("")
    } else {
        value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix("~\\").filter(|_| cfg!(windows)))
    };
    let path = match rest {
        Some(rest) => home.ok_or_else(|| refuse(Problem::NoHome))?.join(rest),
        None => PathBuf::from(value),
    };
    if !path.is_absolute() {
        return Err(refuse(Problem::Relative));
    }
    if std::fs::metadata(&path).is_ok_and(|m| !m.is_dir()) {
        return Err(refuse(Problem::NotADirectory));
    }
    Ok(path)
}

/// What is wrong with an archive directory setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Neither absolute nor `~`-prefixed.
    Relative,
    /// It names a file or something else that is not a directory.
    NotADirectory,
    /// It starts with `~` and no home directory is known.
    NoHome,
}

/// An archive directory setting bird refuses (exit 78).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveDirError {
    /// The value as set.
    pub value: String,
    /// The input it came from.
    pub source: ArchiveSource,
    /// What is wrong with it.
    pub problem: Problem,
}

impl fmt::Display for ArchiveDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.problem {
            Problem::Relative => "is a relative path",
            Problem::NotADirectory => "is not a directory",
            Problem::NoHome => "starts with ~ and no home directory is known",
        };
        write!(
            f,
            "{} {what}: {}; set {ARCHIVE_DIR_KEY} in config.toml or {ARCHIVE_DIR_VAR} to an absolute directory or one under ~/",
            self.source.setting(),
            self.value,
        )
    }
}

impl std::error::Error for ArchiveDirError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _tmp: tempfile::TempDir,
        paths: ResolvedPaths,
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let paths = ResolvedPaths {
            config_dir: root.join("config").join("bird"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            home_dir: Some(root.join("home")),
            token_store: None,
        };
        Fixture { _tmp: tmp, paths }
    }

    fn file_with(dir: &str) -> FileConfig {
        FileConfig {
            archive_dir: Some(dir.to_string()),
            ..FileConfig::default()
        }
    }

    fn env_with(dir: &str) -> EnvOverrides {
        EnvOverrides {
            archive_dir: Some(dir.to_string()),
            ..EnvOverrides::default()
        }
    }

    fn abs(f: &Fixture, name: &str) -> String {
        f.paths.data_dir.join(name).display().to_string()
    }

    #[test]
    fn archive_dir_defaults_under_the_data_dir() {
        let f = fixture();
        let store = StorePaths::resolve(&f.paths, &EnvOverrides::default(), &FileConfig::default())
            .expect("resolves");
        assert_eq!(
            store.archive_dir,
            f.paths.data_dir.join("bird").join("archive")
        );
        assert_eq!(store.archive_source, ArchiveSource::Default);
    }

    #[test]
    fn config_archive_dir_moves_the_archive() {
        let f = fixture();
        let vault = abs(&f, "v/X");
        let store = StorePaths::resolve(&f.paths, &EnvOverrides::default(), &file_with(&vault))
            .expect("resolves");
        assert_eq!(store.archive_dir, PathBuf::from(&vault));
        assert_eq!(store.archive_source, ArchiveSource::Config);
    }

    #[test]
    fn env_archive_dir_beats_config() {
        let f = fixture();
        let (from_env, from_file) = (abs(&f, "env"), abs(&f, "file"));
        let store = StorePaths::resolve(&f.paths, &env_with(&from_env), &file_with(&from_file))
            .expect("resolves");
        assert_eq!(store.archive_dir, PathBuf::from(&from_env));
        assert_eq!(store.archive_source, ArchiveSource::Env);
    }

    #[test]
    fn empty_env_archive_dir_counts_as_unset() {
        let f = fixture();
        let from_file = abs(&f, "file");
        let store =
            StorePaths::resolve(&f.paths, &env_with(""), &file_with(&from_file)).expect("resolves");
        assert_eq!(store.archive_dir, PathBuf::from(&from_file));
        assert_eq!(store.archive_source, ArchiveSource::Config);
    }

    #[test]
    fn tilde_expands_against_the_captured_home() {
        let f = fixture();
        let store = StorePaths::resolve(&f.paths, &EnvOverrides::default(), &file_with("~/notes"))
            .expect("resolves");
        let home = f.paths.home_dir.clone().expect("home");
        assert_eq!(store.archive_dir, home.join("notes"));
    }

    #[test]
    fn a_tilde_without_a_home_is_refused() {
        let mut f = fixture();
        f.paths.home_dir = None;
        let err = StorePaths::resolve(&f.paths, &env_with("~/notes"), &FileConfig::default())
            .expect_err("no home to expand against");
        assert_eq!(err.problem, Problem::NoHome);
    }

    #[test]
    fn relative_archive_dir_is_refused_naming_both_settings() {
        let f = fixture();
        let err = StorePaths::resolve(&f.paths, &EnvOverrides::default(), &file_with("notes/x"))
            .expect_err("relative is refused");
        assert_eq!(err.problem, Problem::Relative);
        let message = err.to_string();
        assert!(message.contains(ARCHIVE_DIR_KEY), "{message}");
        assert!(message.contains(ARCHIVE_DIR_VAR), "{message}");
        assert!(message.contains("notes/x"), "{message}");
    }

    #[test]
    fn a_file_is_not_an_archive_dir() {
        let f = fixture();
        std::fs::create_dir_all(&f.paths.data_dir).expect("mkdir");
        let file = f.paths.data_dir.join("a-file");
        std::fs::write(&file, b"x").expect("write");
        let err = StorePaths::resolve(
            &f.paths,
            &env_with(&file.display().to_string()),
            &FileConfig::default(),
        )
        .expect_err("a file is refused");
        assert_eq!(err.problem, Problem::NotADirectory);
    }

    #[test]
    fn responses_and_ledger_never_follow_the_archive() {
        let f = fixture();
        let store = StorePaths::resolve(
            &f.paths,
            &EnvOverrides::default(),
            &file_with(&abs(&f, "vault")),
        )
        .expect("resolves");
        assert_eq!(
            store.responses_dir,
            f.paths.cache_dir.join("bird").join("responses")
        );
        assert_eq!(
            store.ledger,
            f.paths.data_dir.join("bird").join("spend.jsonl")
        );
        assert_eq!(
            store.ledger_lock(),
            f.paths.data_dir.join("bird").join("spend.lock")
        );
    }

    /// Every store location comes from injected inputs: resolving with an
    /// empty environment lands under the injected directories whatever the
    /// process environment holds, and no store source reads the process
    /// environment.
    #[test]
    fn no_store_path_reads_the_process_environment() {
        let f = fixture();
        let store = StorePaths::resolve(&f.paths, &EnvOverrides::default(), &FileConfig::default())
            .expect("resolves");
        assert!(store.archive_dir.starts_with(&f.paths.data_dir));
        assert!(store.responses_dir.starts_with(&f.paths.cache_dir));
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("store");
        let mut stack = vec![dir];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read store sources") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read source");
                for needle in ["env::var(", "env::var_os(", "dirs::"] {
                    let hits = text
                        .lines()
                        .filter(|l| l.contains(needle) && !l.contains("for needle in"))
                        .count();
                    assert_eq!(hits, 0, "{} calls {needle}", path.display());
                }
            }
        }
    }
}
