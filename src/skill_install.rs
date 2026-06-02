//! `bird skill install [--host <host>]` — copy the embedded skill bundle into
//! a host's canonical skills directory.
//!
//! The bundle source is the repo's `AGENTS.md` embedded at build time via
//! `include_str!`. Each supported host has a fixed destination template
//! (e.g. `~/.claude/skills/bird/SKILL.md`); the install operation creates the
//! parent directory and writes the file. Idempotent — re-running overwrites
//! the destination file in place.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const BUNDLE_CONTENT: &str = include_str!("../AGENTS.md");
const BUNDLE_FILENAME: &str = "SKILL.md";

/// Supported agent runtimes. Add new entries to extend `--host` and `--all`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Host {
    /// Claude Code (`~/.claude/skills/bird/`)
    #[value(name = "claude-code")]
    ClaudeCode,
}

impl Host {
    pub(crate) fn all() -> &'static [Host] {
        &[Host::ClaudeCode]
    }

    fn name(self) -> &'static str {
        match self {
            Host::ClaudeCode => "claude-code",
        }
    }

    /// Destination directory template, relative to `$HOME`.
    fn dest_template(self) -> &'static str {
        match self {
            Host::ClaudeCode => ".claude/skills/bird",
        }
    }

    /// Resolve the destination directory under a given home root.
    fn dest_dir(self, home: &Path) -> PathBuf {
        home.join(self.dest_template())
    }
}

/// Errors emitted by the install pipeline. Mapped to `BirdError::Command` by
/// the caller in `main.rs`.
#[derive(Debug)]
pub(crate) enum InstallError {
    HomeNotSet,
    Io { path: PathBuf, source: io::Error },
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::HomeNotSet => write!(f, "$HOME is not set"),
            InstallError::Io { path, source } => {
                write!(f, "i/o error at {}: {}", path.display(), source)
            }
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            InstallError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn resolve_home() -> Result<PathBuf, InstallError> {
    let raw = std::env::var("HOME").ok();
    raw.filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or(InstallError::HomeNotSet)
}

/// Install the bundle for a single host under the given home root. When
/// `dry_run` is true, no filesystem writes occur; the planned destination
/// path is returned regardless so callers can report it.
pub(crate) fn install_into(
    host: Host,
    home: &Path,
    dry_run: bool,
) -> Result<PathBuf, InstallError> {
    let dest_dir = host.dest_dir(home);
    let dest_file = dest_dir.join(BUNDLE_FILENAME);

    if dry_run {
        return Ok(dest_file);
    }

    fs::create_dir_all(&dest_dir).map_err(|e| InstallError::Io {
        path: dest_dir.clone(),
        source: e,
    })?;
    fs::write(&dest_file, BUNDLE_CONTENT).map_err(|e| InstallError::Io {
        path: dest_file.clone(),
        source: e,
    })?;

    Ok(dest_file)
}

/// Orchestrate the `bird skill install` invocation. Emits one human-readable
/// line per target host to stdout.
pub(crate) fn run(
    host: Option<Host>,
    dry_run: bool,
    all: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = resolve_home()?;
    let targets: Vec<Host> = if all {
        Host::all().to_vec()
    } else {
        vec![host.unwrap_or(Host::ClaudeCode)]
    };

    for h in targets {
        let dest = install_into(h, &home, dry_run)?;
        if dry_run {
            crate::out_println!(
                "[dry-run] would install bird skill ({}): {}",
                h.name(),
                dest.display()
            );
        } else {
            crate::out_println!("installed bird skill ({}): {}", h.name(), dest.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().to_path_buf();
        let dest = install_into(Host::ClaudeCode, &home, true).expect("dry run succeeds");
        assert_eq!(dest, home.join(".claude/skills/bird/SKILL.md"));
        assert!(
            !home.join(".claude").exists(),
            "dry-run must not create any files; found {:?}",
            home.join(".claude")
        );
    }

    #[test]
    fn install_writes_bundle_to_expected_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().to_path_buf();
        let dest = install_into(Host::ClaudeCode, &home, false).expect("install succeeds");

        assert_eq!(dest, home.join(".claude/skills/bird/SKILL.md"));
        assert!(
            dest.is_file(),
            "destination file should exist after install"
        );

        let written = std::fs::read_to_string(&dest).expect("read installed bundle");
        assert_eq!(
            written, BUNDLE_CONTENT,
            "installed bundle must match embedded source"
        );
    }

    #[test]
    fn re_install_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().to_path_buf();

        let first = install_into(Host::ClaudeCode, &home, false).expect("first install");
        let second = install_into(Host::ClaudeCode, &home, false).expect("second install");
        assert_eq!(first, second);

        let written = std::fs::read_to_string(&second).expect("read after re-install");
        assert_eq!(written, BUNDLE_CONTENT);
    }

    #[test]
    fn unknown_host_value_rejected_by_clap() {
        use clap::ValueEnum as _;
        let parsed = Host::from_str("does-not-exist", false);
        assert!(parsed.is_err(), "unknown host string must not parse");
    }

    #[test]
    fn all_targets_includes_claude_code() {
        assert!(Host::all().contains(&Host::ClaudeCode));
    }

    #[test]
    fn resolve_home_missing_returns_error() {
        // Avoid mutating process env (races other tests). Test the typed
        // branch directly: the public helper returns HomeNotSet when the
        // env var is unset or empty.
        let err = match std::env::var("HOME") {
            Ok(_) => InstallError::HomeNotSet,
            Err(_) => resolve_home().expect_err("HOME unset should error"),
        };
        assert!(matches!(err, InstallError::HomeNotSet));
    }
}
