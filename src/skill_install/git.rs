//! Hardened `git clone` construction and spawn. Owns the three hardening
//! constant tables, the command builders, and the spawn-to-`InstallError`
//! reducer.

use std::path::Path;
use std::process::Command;

use super::InstallError;

/// `git clone` config flags applied via `-c key=value` pairs, in token order
/// suitable for `Command::args`.
pub const GIT_HARDEN_FLAGS: &[&str] = &[
    "-c",
    "credential.helper=",
    "-c",
    "core.askPass=",
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.https.allow=always",
    "-c",
    "http.followRedirects=false",
];

/// Environment variables removed via `Command::env_remove` before spawn.
/// Each one is a known git-side override that could redirect or hijack the
/// clone. Never `env_clear()` — that strips PATH and breaks git's helper
/// resolution.
pub const GIT_HARDEN_ENV_REMOVE: &[&str] = &[
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_PROXY_COMMAND",
    "GIT_ASKPASS",
    "GIT_EXEC_PATH",
];

/// Environment variables *set* on the spawned process. The `GIT_CONFIG_*` pair
/// points global / system config at `/dev/null` so user-config `insteadOf`
/// rewriting and other ambient overrides cannot fire — this is the defense
/// against URL-rewriting attacks. `GIT_TERMINAL_PROMPT=0` blocks credential
/// prompts.
pub const GIT_HARDEN_ENV_SET: &[(&str, &str)] = &[
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_SYSTEM", "/dev/null"),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// Build the hardened `git clone` command. Pure constructor — no spawn, no
/// I/O. The returned `Command` carries the full hardening surface.
///
/// `--depth 1` matches the canonical install command shipped in `skill.json`.
pub fn build_clone_command(url: &str, dest: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(GIT_HARDEN_FLAGS);
    cmd.args(["clone", "--depth", "1"]);
    cmd.arg(url);
    cmd.arg(dest);
    for var in GIT_HARDEN_ENV_REMOVE {
        cmd.env_remove(var);
    }
    for (key, value) in GIT_HARDEN_ENV_SET {
        cmd.env(key, value);
    }
    cmd
}

/// User-visible representation of the clone command for the JSON envelope's
/// `command_preview` field and dry-run text output. Intentionally omits the
/// hardening flags — those are an implementation detail.
pub fn format_clone_command(url: &str, dest: &str) -> String {
    format!("git clone --depth 1 {url} {dest}")
}

/// Spawn the prepared `git clone` command and reduce the result to typed
/// `InstallError` variants matching the reason taxonomy.
pub(super) fn spawn_git_clone(cmd: &mut Command) -> Result<(), InstallError> {
    match cmd.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(InstallError::GitCloneFailed {
            code: status.code().unwrap_or(1),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(InstallError::GitNotFound),
        Err(_) => Err(InstallError::GitCloneFailed { code: 1 }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_install::{SkillHost, resolve_host};

    fn skill_repo_url() -> &'static str {
        resolve_host(SkillHost::ClaudeCode).0
    }

    #[test]
    fn build_clone_command_applies_hardening_surface() {
        let url = skill_repo_url();
        let dest = Path::new("/tmp/bird-skill-introspect");
        let cmd = build_clone_command(url, dest);

        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();

        for &flag in GIT_HARDEN_FLAGS {
            assert!(
                args.iter().any(|a| a == flag),
                "GIT_HARDEN_FLAGS entry {flag:?} missing from command args; got {args:?}",
            );
        }
        assert!(
            args.iter().any(|a| a == "clone"),
            "missing 'clone' subcommand: {args:?}"
        );
        assert!(
            args.iter().any(|a| a == "--depth"),
            "missing --depth flag: {args:?}"
        );
        assert!(args.iter().any(|a| a == "1"), "missing --depth value");
        assert!(args.iter().any(|a| a == url), "missing url operand");

        let envs: std::collections::HashMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|s| s.to_string_lossy().into_owned()),
                )
            })
            .collect();

        for &var in GIT_HARDEN_ENV_REMOVE {
            let entry = envs.get(var);
            assert!(
                matches!(entry, Some(None)),
                "GIT_HARDEN_ENV_REMOVE entry {var:?} should be removed; got {entry:?}",
            );
        }

        for &(key, value) in GIT_HARDEN_ENV_SET {
            let entry = envs.get(key);
            assert_eq!(
                entry,
                Some(&Some(value.to_string())),
                "GIT_HARDEN_ENV_SET entry {key}={value:?} not present",
            );
        }
    }

    #[test]
    fn git_harden_env_set_disables_user_config() {
        let pairs: std::collections::HashMap<&str, &str> =
            GIT_HARDEN_ENV_SET.iter().copied().collect();
        for var in ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"] {
            let v = pairs
                .get(var)
                .unwrap_or_else(|| panic!("GIT_HARDEN_ENV_SET missing {var}"));
            assert_eq!(*v, "/dev/null", "{var} must be set to /dev/null");
        }
    }

    #[test]
    fn format_clone_command_matches_canonical_shape() {
        let s = format_clone_command(skill_repo_url(), "/home/u/.claude/skills/bird");
        assert_eq!(
            s,
            "git clone --depth 1 https://github.com/brettdavies/bird-skill.git /home/u/.claude/skills/bird",
        );
    }
}
