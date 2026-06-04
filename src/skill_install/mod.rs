//! `bird skill install <host>` — clone the bird skill bundle into a host's
//! canonical skills directory using a build-generated host map and a hardened
//! `git clone` invocation. The companion `bird skill update <host>` removes
//! the destination and re-clones.
//!
//! Pipeline:
//!
//! ```text
//!   clap parse (host, --dry-run, --all, --output)
//!         |
//!         v
//!   resolve_host(SkillHost) -> (url, dest_template)
//!         |
//!         v
//!   expand_tilde(dest_template) via $HOME  -- HOME unset --> MissingHome
//!         |                                                  (reason=home-not-set)
//!         v
//!      dry_run? --yes--> emit envelope (mode=dry-run, would_succeed)
//!         |
//!         no
//!         v
//!   check_destination()       -- conflict --> envelope(error, reason)
//!    (canonicalize)
//!         |
//!         v
//!   build_clone_command(url, dest) with hardening
//!         |
//!         v
//!      spawn git -- NotFound ---> reason=git-not-found
//!         |     -- nonzero ----> reason=git-clone-failed
//!         v
//!      exit 0 --> success envelope
//! ```
//!
//! Hardening surface:
//! - `GIT_HARDEN_FLAGS` — five `-c key=value` pairs (`credential.helper`,
//!   `core.askPass`, `protocol.allow=never`, `protocol.https.allow=always`,
//!   `http.followRedirects=false`).
//! - `GIT_HARDEN_ENV_REMOVE` — five env vars stripped before spawn (SSH /
//!   proxy / askpass / exec-path overrides).
//! - `GIT_HARDEN_ENV_SET` — `GIT_CONFIG_GLOBAL=/dev/null`,
//!   `GIT_CONFIG_SYSTEM=/dev/null`, `GIT_TERMINAL_PROMPT=0` — disable every
//!   layer of user-controlled git config and block credential prompts.
//!
//! Mirrors the `xurl-rs` / `agentnative-cli` pattern so all three CLIs reject
//! the same malformed inputs and apply identical hardening.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::output::{OutputConfig, OutputFormat};

// `SkillHost`, `KNOWN_HOSTS`, `resolve_host`, and `host_envelope_str` are
// auto-generated at build time from `src/skill_install/skill.json`. Edit the
// JSON file to add or remove hosts; `cargo build` regenerates this file.
include!(concat!(env!("OUT_DIR"), "/generated_hosts.rs"));

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

/// Typed install error — closed set matching the envelope `reason` taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    MissingHome,
    DestNotEmpty,
    DestIsFile,
    GitNotFound,
    GitCloneFailed { code: i32 },
}

impl InstallError {
    /// Kebab-case identifier for the envelope `reason` field.
    pub fn reason(&self) -> &'static str {
        match self {
            InstallError::MissingHome => "home-not-set",
            InstallError::DestNotEmpty => "destination-not-empty",
            InstallError::DestIsFile => "destination-is-file",
            InstallError::GitNotFound => "git-not-found",
            InstallError::GitCloneFailed { .. } => "git-clone-failed",
        }
    }
}

/// Snapshot of what `check_destination` found at the resolved path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationStatus {
    Absent,
    EmptyDir,
    NonEmptyDir,
    File,
}

impl DestinationStatus {
    /// Kebab-case identifier for the JSON envelope `destination_status` field.
    pub fn as_envelope_str(self) -> &'static str {
        match self {
            DestinationStatus::Absent => "absent",
            DestinationStatus::EmptyDir => "empty-dir",
            DestinationStatus::NonEmptyDir => "non-empty-dir",
            DestinationStatus::File => "file",
        }
    }
}

/// Expand a leading `~` or `~/` to `$HOME`. Pure passthrough on inputs that
/// do not start with `~`. `MissingHome` only fires when the input actually
/// begins with `~` and `$HOME` is unset or empty.
pub fn expand_tilde(template: &str) -> Result<PathBuf, InstallError> {
    let home = std::env::var("HOME").ok();
    expand_tilde_with(template, home.as_deref())
}

/// Pure-function core of [`expand_tilde`]. Tests pass `home` explicitly so
/// they never mutate the process environment (which would race with parallel
/// tests).
pub fn expand_tilde_with(template: &str, home: Option<&str>) -> Result<PathBuf, InstallError> {
    let needs_home = template == "~" || template.starts_with("~/");
    if !needs_home {
        return Ok(PathBuf::from(template));
    }
    let home = home
        .filter(|s| !s.is_empty())
        .ok_or(InstallError::MissingHome)?;
    if template == "~" {
        return Ok(PathBuf::from(home));
    }
    let rest = template
        .strip_prefix("~/")
        .expect("template starts with ~/ per the branch guard");
    let mut p = PathBuf::from(home);
    p.push(rest);
    Ok(p)
}

/// Canonicalizes the path so a symlinked skills directory resolves to its
/// real target before the check runs. Returns `Absent`/`EmptyDir` on success;
/// `DestIsFile` for a regular file, `DestNotEmpty` for a populated directory.
pub fn check_destination(path: &Path) -> Result<DestinationStatus, InstallError> {
    match path.try_exists() {
        Ok(false) => return Ok(DestinationStatus::Absent),
        Ok(true) => {}
        Err(_) => return Err(InstallError::DestNotEmpty),
    }

    let canonical = fs::canonicalize(path).map_err(|_| InstallError::DestNotEmpty)?;
    let metadata = fs::metadata(&canonical).map_err(|_| InstallError::DestNotEmpty)?;

    if metadata.is_file() {
        return Err(InstallError::DestIsFile);
    }

    if metadata.is_dir() {
        let mut entries = fs::read_dir(&canonical).map_err(|_| InstallError::DestNotEmpty)?;
        if entries.next().is_some() {
            return Err(InstallError::DestNotEmpty);
        }
        return Ok(DestinationStatus::EmptyDir);
    }

    Err(InstallError::DestIsFile)
}

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

/// Result envelope shared by both `--output text` and `--output json`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallEnvelope {
    pub action: &'static str,
    pub host: &'static str,
    pub install_dir: String,
    pub command_preview: String,
    pub destination_status: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub would_succeed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// Multi-host envelope for `--all` invocations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallMultiEnvelope {
    pub action: &'static str,
    pub status: &'static str,
    pub installations: Vec<InstallEnvelope>,
    pub exit_code: i32,
}

const ACTION_INSTALL: &str = "skill-install";
const ACTION_UPDATE: &str = "skill-update";
const STATUS_DRY_RUN: &str = "dry_run";
const STATUS_OK: &str = "ok";
const STATUS_ERROR: &str = "error";

/// Compute the envelope without performing I/O (dry-run) or, in install mode,
/// after spawning `git`.
pub fn compute_install_envelope(host: SkillHost, dry_run: bool) -> InstallEnvelope {
    let (url, dest_template) = resolve_host(host);
    let host_str = host_envelope_str(host);

    let dest = match expand_tilde(dest_template) {
        Ok(p) => p,
        Err(InstallError::MissingHome) => {
            let command_preview = format_clone_command(url, dest_template);
            return InstallEnvelope {
                action: ACTION_INSTALL,
                host: host_str,
                install_dir: dest_template.to_string(),
                command_preview,
                destination_status: DestinationStatus::Absent.as_envelope_str(),
                status: STATUS_ERROR,
                would_succeed: if dry_run { Some(false) } else { None },
                exit_code: Some(1),
                reason: Some(InstallError::MissingHome.reason()),
            };
        }
        Err(_) => unreachable!("expand_tilde only emits MissingHome"),
    };

    let dest_display = dest.display().to_string();
    let command_preview = format_clone_command(url, &dest_display);

    let dest_status = match check_destination(&dest) {
        Ok(s) => s,
        Err(e @ InstallError::DestIsFile) | Err(e @ InstallError::DestNotEmpty) => {
            let status_label = match e {
                InstallError::DestIsFile => DestinationStatus::File,
                InstallError::DestNotEmpty => DestinationStatus::NonEmptyDir,
                _ => unreachable!(),
            };
            return InstallEnvelope {
                action: ACTION_INSTALL,
                host: host_str,
                install_dir: dest_display,
                command_preview,
                destination_status: status_label.as_envelope_str(),
                status: STATUS_ERROR,
                would_succeed: if dry_run { Some(false) } else { None },
                exit_code: Some(1),
                reason: Some(e.reason()),
            };
        }
        Err(_) => unreachable!("check_destination only emits DestIsFile / DestNotEmpty"),
    };

    let dest_status_str = dest_status.as_envelope_str();

    if dry_run {
        return InstallEnvelope {
            action: ACTION_INSTALL,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_status_str,
            status: STATUS_DRY_RUN,
            would_succeed: Some(true),
            exit_code: Some(0),
            reason: None,
        };
    }

    let mut cmd = build_clone_command(url, &dest);
    match spawn_git_clone(&mut cmd) {
        Ok(()) => InstallEnvelope {
            action: ACTION_INSTALL,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_status_str,
            status: STATUS_OK,
            would_succeed: None,
            exit_code: Some(0),
            reason: None,
        },
        Err(InstallError::GitCloneFailed { code }) => InstallEnvelope {
            action: ACTION_INSTALL,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_status_str,
            status: STATUS_ERROR,
            would_succeed: None,
            exit_code: Some(code),
            reason: Some(InstallError::GitCloneFailed { code }.reason()),
        },
        Err(InstallError::GitNotFound) => InstallEnvelope {
            action: ACTION_INSTALL,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_status_str,
            status: STATUS_ERROR,
            would_succeed: None,
            exit_code: Some(1),
            reason: Some(InstallError::GitNotFound.reason()),
        },
        Err(_) => unreachable!("spawn_git_clone only emits GitCloneFailed / GitNotFound"),
    }
}

/// Spawn the prepared `git clone` command and reduce the result to typed
/// `InstallError` variants matching the reason taxonomy.
fn spawn_git_clone(cmd: &mut Command) -> Result<(), InstallError> {
    match cmd.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(InstallError::GitCloneFailed {
            code: status.code().unwrap_or(1),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(InstallError::GitNotFound),
        Err(_) => Err(InstallError::GitCloneFailed { code: 1 }),
    }
}

/// Render a single install envelope per the active output format.
fn render_envelope(env: &InstallEnvelope, format: &OutputFormat) -> String {
    if format.is_json() {
        return render_structured(env, format);
    }
    match env.status {
        STATUS_DRY_RUN => env.command_preview.clone(),
        STATUS_OK => format!("Installed bird skill bundle into {}", env.install_dir),
        _ => {
            let reason = env.reason.unwrap_or("unknown");
            format!("error: {reason}: {}", env.install_dir)
        }
    }
}

/// Render a multi-host envelope per the active output format.
fn render_multi(env: &InstallMultiEnvelope, format: &OutputFormat) -> String {
    if format.is_json() {
        return render_structured(env, format);
    }
    let mut out = String::new();
    for inst in &env.installations {
        out.push_str(&render_envelope(inst, format));
        out.push('\n');
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Best-effort serializer for any of the structured formats.
fn render_structured<T: serde::Serialize>(env: &T, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(env)
            .unwrap_or_else(|_| "{\"status\":\"error\"}".to_string()),
        OutputFormat::Jsonl | OutputFormat::Ndjson => {
            serde_json::to_string(env).unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
        }
        OutputFormat::Text => unreachable!("guarded by is_json()"),
    }
}

/// Orchestrate the install pipeline for a single host. Writes the envelope to
/// `stdout` and returns the exit code.
pub fn run_install(
    host: SkillHost,
    dry_run: bool,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    let envelope = compute_install_envelope(host, dry_run);
    let rendered = render_envelope(&envelope, &out.format);
    let _ = writeln!(stdout, "{rendered}");
    if envelope.status == STATUS_ERROR {
        envelope.exit_code.unwrap_or(1)
    } else {
        0
    }
}

/// Multi-host wrapper: when `all` is set, iterates every entry in
/// [`KNOWN_HOSTS`]; otherwise dispatches to [`run_install`] for the single
/// host. Per-host failures are reported in the result stream but do not abort
/// the run — the exit code is the worst observed.
pub fn run_install_multi(
    host: Option<SkillHost>,
    all: bool,
    dry_run: bool,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    if all {
        return run_for_all_hosts(dry_run, out, stdout);
    }
    let Some(host) = host else {
        return emit_missing_host_envelope(out, stdout);
    };
    run_install(host, dry_run, out, stdout)
}

/// Render a per-host envelope sequence as a single multi envelope.
fn run_for_all_hosts(dry_run: bool, out: &OutputConfig, stdout: &mut dyn Write) -> i32 {
    use clap::ValueEnum as _;

    let mut installations = Vec::with_capacity(KNOWN_HOSTS.len());
    let mut worst: i32 = 0;
    for host in SkillHost::value_variants() {
        let env = compute_install_envelope(*host, dry_run);
        if env.status == STATUS_ERROR {
            worst = worst.max(env.exit_code.unwrap_or(1));
        }
        installations.push(env);
    }
    let multi = InstallMultiEnvelope {
        action: ACTION_INSTALL,
        status: if worst == 0 {
            if dry_run { STATUS_DRY_RUN } else { STATUS_OK }
        } else {
            STATUS_ERROR
        },
        installations,
        exit_code: worst,
    };
    let rendered = render_multi(&multi, &out.format);
    let _ = writeln!(stdout, "{rendered}");
    worst
}

/// Emit a hint envelope listing the known hosts. Returned when neither a host
/// nor `--all` is supplied.
fn emit_missing_host_envelope(out: &OutputConfig, stdout: &mut dyn Write) -> i32 {
    if out.format.is_json() {
        let json = serde_json::json!({
            "action": ACTION_INSTALL,
            "status": "error",
            "reason": "missing-host",
            "exit_code": 2,
            "message": "missing target host; pass <host> or --all",
            "known_hosts": KNOWN_HOSTS,
        });
        let _ = writeln!(stdout, "{}", render_structured(&json, &out.format));
    } else {
        let _ = writeln!(
            stdout,
            "error: missing target host; pass <host> or --all\nsupported hosts:"
        );
        for h in KNOWN_HOSTS {
            let _ = writeln!(stdout, "  {h}");
        }
    }
    2
}

/// Orchestrate the update pipeline for a single host. Semantics: remove the
/// existing destination (if present) and re-run the install. When `dry_run`
/// is set, emit a `skill-update` envelope describing the resolved operation
/// without touching the filesystem or spawning `git`.
pub fn run_update(
    host: SkillHost,
    dry_run: bool,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    let (_, dest_template) = resolve_host(host);
    let host_str = host_envelope_str(host);

    let dest = match expand_tilde(dest_template) {
        Ok(p) => p,
        Err(InstallError::MissingHome) => {
            let rendered = render_update_error(host_str, dest_template, dry_run, "home-not-set");
            let _ = writeln!(stdout, "{rendered}");
            return 1;
        }
        Err(_) => unreachable!("expand_tilde only emits MissingHome"),
    };

    if dry_run {
        let rendered = render_update_dry_run(host_str, &dest.display().to_string(), dest.exists());
        let _ = writeln!(stdout, "{rendered}");
        return 0;
    }

    if dest.exists()
        && let Err(e) = fs::remove_dir_all(&dest)
    {
        let rendered = render_update_error(
            host_str,
            &dest.display().to_string(),
            false,
            &format!("remove-failed: {e}"),
        );
        let _ = writeln!(stdout, "{rendered}");
        return 1;
    }

    let env = compute_install_envelope(host, false);
    let mut rendered = render_envelope(&env, &out.format);
    if env.status == STATUS_OK {
        rendered = rendered.replace(ACTION_INSTALL, ACTION_UPDATE);
    }
    let _ = writeln!(stdout, "{rendered}");
    if env.status == STATUS_ERROR {
        env.exit_code.unwrap_or(1)
    } else {
        0
    }
}

/// Multi-host wrapper for update. When `all` is set, updates every entry in
/// [`KNOWN_HOSTS`] whose destination currently exists, skipping hosts that
/// aren't installed. Otherwise dispatches to [`run_update`] for the single
/// host.
pub fn run_update_multi(
    host: Option<SkillHost>,
    all: bool,
    dry_run: bool,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    if all {
        use clap::ValueEnum as _;
        let mut worst: i32 = 0;
        for h in SkillHost::value_variants() {
            let code = run_update(*h, dry_run, out, stdout);
            if code != 0 {
                worst = worst.max(code);
            }
        }
        return worst;
    }
    let Some(host) = host else {
        return emit_missing_host_envelope(out, stdout);
    };
    run_update(host, dry_run, out, stdout)
}

fn render_update_dry_run(host: &str, install_dir: &str, dest_exists: bool) -> String {
    let json = serde_json::json!({
        "action": ACTION_UPDATE,
        "host": host,
        "install_dir": install_dir,
        "destination_status": if dest_exists { "non-empty-dir" } else { "absent" },
        "status": STATUS_DRY_RUN,
        "would_succeed": true,
        "exit_code": 0,
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
}

fn render_update_error(host: &str, install_dir: &str, dry_run: bool, reason: &str) -> String {
    let json = serde_json::json!({
        "action": ACTION_UPDATE,
        "host": host,
        "install_dir": install_dir,
        "status": STATUS_ERROR,
        "would_succeed": if dry_run { Some(false) } else { None },
        "exit_code": 1,
        "reason": reason,
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    fn skill_repo_url() -> &'static str {
        resolve_host(SkillHost::ClaudeCode).0
    }

    #[test]
    fn expand_tilde_replaces_leading_tilde_slash_with_home() {
        let got = expand_tilde_with("~/.claude/skills/bird", Some("/home/test"))
            .expect("HOME present + ~/ prefix should expand cleanly");
        assert_eq!(got, PathBuf::from("/home/test/.claude/skills/bird"));
    }

    #[test]
    fn expand_tilde_missing_home_only_when_input_starts_with_tilde() {
        let err = expand_tilde_with("~/anything", None)
            .expect_err("HOME unset + tilde input should be MissingHome");
        assert_eq!(err, InstallError::MissingHome);

        let err_empty =
            expand_tilde_with("~", Some("")).expect_err("HOME empty string is treated as unset");
        assert_eq!(err_empty, InstallError::MissingHome);
    }

    #[test]
    fn expand_tilde_no_tilde_passthrough() {
        let got = expand_tilde_with("/abs/path", Some("/home/test"))
            .expect("non-tilde input never errors");
        assert_eq!(got, PathBuf::from("/abs/path"));

        let got_no_home =
            expand_tilde_with("/abs/path", None).expect("non-tilde input ignores HOME");
        assert_eq!(got_no_home, PathBuf::from("/abs/path"));
    }

    #[test]
    fn check_destination_absent_for_nonexistent_path() {
        let tmp = tempfile::tempdir().expect("tempdir creation");
        let target = tmp.path().join("does-not-exist");
        let status = check_destination(&target).expect("absent path should be Ok(Absent)");
        assert_eq!(status, DestinationStatus::Absent);
    }

    #[test]
    fn check_destination_empty_dir() {
        let tmp = tempfile::tempdir().expect("tempdir creation");
        let status = check_destination(tmp.path()).expect("empty tempdir should be Ok(EmptyDir)");
        assert_eq!(status, DestinationStatus::EmptyDir);
    }

    #[test]
    fn check_destination_non_empty_dir_errors() {
        let tmp = tempfile::tempdir().expect("tempdir creation");
        std::fs::write(tmp.path().join("placeholder"), b"x").expect("write placeholder");
        let err = check_destination(tmp.path()).expect_err("populated dir should be DestNotEmpty");
        assert_eq!(err, InstallError::DestNotEmpty);
    }

    #[test]
    fn check_destination_regular_file_errors() {
        let tmp = tempfile::tempdir().expect("tempdir creation");
        let target = tmp.path().join("a-file");
        std::fs::write(&target, b"contents").expect("write file");
        let err = check_destination(&target).expect_err("file should be DestIsFile");
        assert_eq!(err, InstallError::DestIsFile);
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
    fn skill_host_clap_value_names_match_known_hosts() {
        for &expected in KNOWN_HOSTS {
            let parsed = SkillHost::from_str(expected, false)
                .unwrap_or_else(|_| panic!("KNOWN_HOSTS entry {expected:?} not parseable"));
            let rendered = parsed
                .to_possible_value()
                .expect("clap ValueEnum variant always has a possible value")
                .get_name()
                .to_string();
            assert_eq!(rendered, expected);
        }
    }

    #[test]
    fn known_hosts_matches_skill_host_variant_count_and_names() {
        let variant_names: Vec<String> = SkillHost::value_variants()
            .iter()
            .map(|v| {
                v.to_possible_value()
                    .expect("clap ValueEnum variant always has a possible value")
                    .get_name()
                    .to_string()
            })
            .collect();
        let known: Vec<String> = KNOWN_HOSTS.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(
            variant_names, known,
            "SkillHost variants and KNOWN_HOSTS must stay in lockstep",
        );
    }

    #[test]
    fn format_clone_command_matches_canonical_shape() {
        let s = format_clone_command(skill_repo_url(), "/home/u/.claude/skills/bird");
        assert_eq!(
            s,
            "git clone --depth 1 https://github.com/brettdavies/bird-skill.git /home/u/.claude/skills/bird",
        );
    }

    #[test]
    fn resolve_host_returns_expected_pair_for_every_variant() {
        let fixture_text = include_str!("skill.json");
        let fixture: serde_json::Value =
            serde_json::from_str(fixture_text).expect("fixture is valid JSON");
        let install = fixture
            .get("install")
            .and_then(|v| v.as_object())
            .expect("fixture has install map");

        for &host_name in KNOWN_HOSTS {
            let cmd = install
                .get(host_name)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("fixture missing install.{host_name}"));
            let tokens: Vec<&str> = cmd.split_whitespace().collect();
            let expected_url = tokens[4];
            let expected_dest = tokens[5];

            let host = SkillHost::from_str(host_name, false)
                .unwrap_or_else(|_| panic!("KNOWN_HOSTS entry {host_name:?} unparseable"));
            let (url, dest) = resolve_host(host);

            assert_eq!(url, expected_url, "url mismatch for {host_name}");
            assert_eq!(dest, expected_dest, "dest mismatch for {host_name}");
        }
    }

    #[test]
    fn install_error_reasons_match_closed_set() {
        assert_eq!(InstallError::MissingHome.reason(), "home-not-set");
        assert_eq!(InstallError::DestNotEmpty.reason(), "destination-not-empty");
        assert_eq!(InstallError::DestIsFile.reason(), "destination-is-file");
        assert_eq!(InstallError::GitNotFound.reason(), "git-not-found");
        assert_eq!(
            InstallError::GitCloneFailed { code: 128 }.reason(),
            "git-clone-failed"
        );
    }
}
