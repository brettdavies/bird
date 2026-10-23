//! `bird skill install <host>` — clone the bird skill bundle into a host's
//! canonical skills directory using a build-generated host map and a hardened
//! `git clone` invocation. The companion `bird skill update <host>` removes
//! the destination, and any copy at the host's legacy location, and re-clones;
//! `bird skill update --all` does that only for the hosts holding a copy and
//! reports the rest as skipped.
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
//!   skill_env.destination(host)
//!     host config-dir var, else BIRD_SKILL_HOME,
//!     else host base-dir var, else HOME
//!         |                   -- none set --> MissingHome (reason=home-not-set)
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
//! the same malformed inputs, resolve the same destinations, and apply
//! identical hardening.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::output::envelope::NextStep;
use crate::output::{OutputConfig, OutputFormat};

mod destination;

pub use destination::{DestinationStatus, SkillEnv, check_destination, expand_tilde_with};

// `SkillHost`, `KNOWN_HOSTS`, `resolve_host`, `host_envelope_str`,
// `config_dir_env`, `CONFIG_DIR_VARS`, `base_dir_env`, `BASE_DIR_VARS`, and
// `legacy_destination` are auto-generated at build time from
// `src/skill_install/skill.json`. Edit the JSON file to add or remove hosts or
// change a host's variables; `cargo build` regenerates this file.
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
    /// Neither the host's config- or base-directory variable,
    /// `BIRD_SKILL_HOME`, nor `HOME` is set, so a `~` destination cannot
    /// expand.
    MissingHome,
    DestNotEmpty,
    DestIsFile,
    GitNotFound,
    GitCloneFailed {
        code: i32,
    },
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
    /// A copy of the bundle found at a location other than `install_dir` that
    /// the host still reads, such as Codex's deprecated `~/.codex/skills`.
    /// `skill update` removes it and installs at `install_dir`; `skill
    /// install` leaves it in place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_install_dir: Option<String>,
    /// What the caller should do next, when a recovery step exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<NextStep>,
}

impl InstallEnvelope {
    /// Sets the recovery step for this envelope's reason, as `xr`'s skill
    /// envelopes map them: a destination that already holds files is
    /// `skill update`'s to replace, a failed clone is tried again, and the
    /// rest are the machine's state, which no bird command changes.
    fn with_next_step(mut self) -> Self {
        let host = self.host;
        self.next_step = self.reason.and_then(|reason| match reason {
            "destination-not-empty" => {
                Some(NextStep::run_command(format!("bird skill update {host}")))
            }
            REASON_NOT_INSTALLED => {
                Some(NextStep::run_command(format!("bird skill install {host}")))
            }
            "git-clone-failed" => Some(NextStep::retry(None)),
            "home-not-set" | "destination-is-file" | "git-not-found" | REASON_REMOVE_FAILED => {
                Some(NextStep::fix_input(None))
            }
            _ => None,
        });
        self
    }
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
const STATUS_SKIPPED: &str = "skipped";
const REASON_REMOVE_FAILED: &str = "remove-failed";

/// Reason paired with [`STATUS_SKIPPED`] when `skill update --all` passes over
/// a host with nothing installed. Update refreshes what is installed;
/// installing everywhere is what `skill install --all` is for.
const REASON_NOT_INSTALLED: &str = "not-installed";

/// Compute the envelope without performing I/O (dry-run) or, in install mode,
/// after spawning `git`. A copy at the host's legacy location is named in the
/// envelope and left in place.
pub fn compute_install_envelope(
    host: SkillHost,
    dry_run: bool,
    skill_env: &SkillEnv,
) -> InstallEnvelope {
    let mut envelope = install_envelope(host, dry_run, skill_env);
    envelope.legacy_install_dir = skill_env
        .legacy_copy(host)
        .map(|path| path.display().to_string());
    envelope.with_next_step()
}

/// The envelope for a destination that cannot resolve: the template and its
/// literal `~` are the most specific thing to report.
fn missing_home_envelope(action: &'static str, host: SkillHost, dry_run: bool) -> InstallEnvelope {
    let (url, dest_template) = resolve_host(host);
    InstallEnvelope {
        action,
        host: host_envelope_str(host),
        install_dir: dest_template.to_string(),
        command_preview: format_clone_command(url, dest_template),
        destination_status: DestinationStatus::Absent.as_envelope_str(),
        status: STATUS_ERROR,
        would_succeed: if dry_run { Some(false) } else { None },
        exit_code: Some(1),
        reason: Some(InstallError::MissingHome.reason()),
        legacy_install_dir: None,
        next_step: None,
    }
}

fn install_envelope(host: SkillHost, dry_run: bool, skill_env: &SkillEnv) -> InstallEnvelope {
    let (url, _) = resolve_host(host);
    let host_str = host_envelope_str(host);

    let Ok(dest) = skill_env.destination(host) else {
        return missing_home_envelope(ACTION_INSTALL, host, dry_run);
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
                legacy_install_dir: None,
                next_step: None,
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
            legacy_install_dir: None,
            next_step: None,
        };
    }

    let mut cmd = build_clone_command(url, &dest);
    let (status, exit_code, reason) = match spawn_git_clone(&mut cmd) {
        Ok(()) => (STATUS_OK, 0, None),
        Err(e @ InstallError::GitCloneFailed { code }) => (STATUS_ERROR, code, Some(e.reason())),
        Err(e) => (STATUS_ERROR, 1, Some(e.reason())),
    };
    InstallEnvelope {
        action: ACTION_INSTALL,
        host: host_str,
        install_dir: dest_display,
        command_preview,
        destination_status: dest_status_str,
        status,
        would_succeed: None,
        exit_code: Some(exit_code),
        reason,
        legacy_install_dir: None,
        next_step: None,
    }
}

/// Resolve one host's update outcome without emitting it. Removes the existing
/// destination, and any copy at the host's legacy location, then re-runs the
/// install unless `dry_run` is set, so the returned envelope reports what
/// actually happened.
pub fn compute_update_envelope(
    host: SkillHost,
    dry_run: bool,
    skill_env: &SkillEnv,
) -> InstallEnvelope {
    update_envelope(host, dry_run, skill_env).with_next_step()
}

fn update_envelope(host: SkillHost, dry_run: bool, skill_env: &SkillEnv) -> InstallEnvelope {
    let (url, _) = resolve_host(host);
    let host_str = host_envelope_str(host);

    let Ok(dest) = skill_env.destination(host) else {
        return missing_home_envelope(ACTION_UPDATE, host, dry_run);
    };

    let dest_display = dest.display().to_string();
    let command_preview = format_clone_command(url, &dest_display);
    let dest_exists = dest.exists();
    let legacy = skill_env.legacy_copy(host);
    let legacy_install_dir = legacy.as_ref().map(|path| path.display().to_string());
    let dest_label = if dest_exists {
        DestinationStatus::NonEmptyDir.as_envelope_str()
    } else {
        DestinationStatus::Absent.as_envelope_str()
    };

    if dry_run {
        return InstallEnvelope {
            action: ACTION_UPDATE,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_label,
            status: STATUS_DRY_RUN,
            // Update clears the destination first, so an occupied directory is
            // not the blocker it is for a plain install.
            would_succeed: Some(true),
            exit_code: Some(0),
            reason: None,
            legacy_install_dir,
            next_step: None,
        };
    }

    let to_remove = [dest_exists.then_some(dest.as_path()), legacy.as_deref()];
    if to_remove
        .into_iter()
        .flatten()
        .any(|dir| fs::remove_dir_all(dir).is_err())
    {
        return InstallEnvelope {
            action: ACTION_UPDATE,
            host: host_str,
            install_dir: dest_display,
            command_preview,
            destination_status: dest_label,
            status: STATUS_ERROR,
            would_succeed: None,
            exit_code: Some(1),
            reason: Some(REASON_REMOVE_FAILED),
            legacy_install_dir,
            next_step: None,
        };
    }

    let mut envelope = install_envelope(host, false, skill_env);
    envelope.action = ACTION_UPDATE;
    envelope.legacy_install_dir = legacy_install_dir;
    envelope
}

/// The envelope for a host `--all` passes over because nothing is installed
/// there. Carries exit code 0: a host with no installation is not a failure.
fn skipped_envelope(host: SkillHost, skill_env: &SkillEnv) -> InstallEnvelope {
    let (url, dest_template) = resolve_host(host);
    let install_dir = skill_env
        .destination(host)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| dest_template.to_string());

    InstallEnvelope {
        action: ACTION_UPDATE,
        host: host_envelope_str(host),
        command_preview: format_clone_command(url, &install_dir),
        install_dir,
        destination_status: DestinationStatus::Absent.as_envelope_str(),
        status: STATUS_SKIPPED,
        would_succeed: None,
        exit_code: Some(0),
        reason: Some(REASON_NOT_INSTALLED),
        legacy_install_dir: None,
        next_step: None,
    }
    .with_next_step()
}

/// Whether a host has something to update, at its destination or its legacy
/// location.
fn is_installed(host: SkillHost, skill_env: &SkillEnv) -> bool {
    skill_env.destination(host).is_ok_and(|dest| dest.exists())
        || skill_env.legacy_copy(host).is_some()
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

/// Render a single install envelope per the active output format. Text mode
/// adds a note line when the envelope names a copy at the host's legacy
/// location.
fn render_envelope(env: &InstallEnvelope, format: &OutputFormat) -> String {
    if format.is_json() {
        return render_structured(env, format);
    }
    let summary = match env.status {
        STATUS_DRY_RUN => env.command_preview.clone(),
        STATUS_OK => format!("Installed bird skill bundle into {}", env.install_dir),
        _ => {
            let label = if env.status == STATUS_SKIPPED {
                STATUS_SKIPPED
            } else {
                STATUS_ERROR
            };
            let reason = env.reason.unwrap_or("unknown");
            format!("{label}: {reason}: {}", env.install_dir)
        }
    };
    let Some(legacy) = &env.legacy_install_dir else {
        return summary;
    };
    let note = if env.action == ACTION_UPDATE {
        format!("note: {legacy} holds another copy; update removes it along with the destination")
    } else {
        format!(
            "note: {legacy} holds another copy; `bird skill update {}` removes it",
            env.host
        )
    };
    format!("{summary}\n{note}")
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

/// Write `envelope` to `stdout` and return its exit code.
fn emit(envelope: &InstallEnvelope, out: &OutputConfig, stdout: &mut dyn Write) -> i32 {
    let rendered = render_envelope(envelope, &out.format);
    let _ = writeln!(stdout, "{rendered}");
    if envelope.status == STATUS_ERROR {
        envelope.exit_code.unwrap_or(1)
    } else {
        0
    }
}

/// Orchestrate the install pipeline for a single host. Writes the envelope to
/// `stdout` and returns the exit code. `skill_env` is the environment the
/// runner captured; the destination resolves against it alone.
pub fn run_install(
    host: SkillHost,
    dry_run: bool,
    skill_env: &SkillEnv,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    emit(
        &compute_install_envelope(host, dry_run, skill_env),
        out,
        stdout,
    )
}

/// Multi-host wrapper: when `all` is set, iterates every entry in
/// [`KNOWN_HOSTS`]; otherwise dispatches to [`run_install`] for the single
/// host. Per-host failures are reported in the result stream but do not abort
/// the run — the exit code is the worst observed.
pub fn run_install_multi(
    host: Option<SkillHost>,
    all: bool,
    dry_run: bool,
    skill_env: &SkillEnv,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    if all {
        return run_for_all_hosts(ACTION_INSTALL, dry_run, out, stdout, |host| {
            compute_install_envelope(host, dry_run, skill_env)
        });
    }
    let Some(host) = host else {
        return emit_missing_host_envelope(out, stdout);
    };
    run_install(host, dry_run, skill_env, out, stdout)
}

/// Build one envelope per host in [`KNOWN_HOSTS`] order with `envelope_for`
/// and emit them as a single multi envelope carrying the worst exit code.
fn run_for_all_hosts(
    action: &'static str,
    dry_run: bool,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    envelope_for: impl Fn(SkillHost) -> InstallEnvelope,
) -> i32 {
    use clap::ValueEnum as _;

    let mut installations = Vec::with_capacity(KNOWN_HOSTS.len());
    let mut worst: i32 = 0;
    for host in SkillHost::value_variants() {
        let env = envelope_for(*host);
        if env.status == STATUS_ERROR {
            worst = worst.max(env.exit_code.unwrap_or(1));
        }
        installations.push(env);
    }
    let multi = InstallMultiEnvelope {
        action,
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
            "next_step": NextStep::show_help("bird skill install --help".to_string()),
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

/// Orchestrate the update pipeline for a single host: remove the existing
/// destination and any legacy copy, then re-run the install. When `dry_run`
/// is set, emit a `skill-update` envelope describing the resolved operation
/// without touching the filesystem or spawning `git`.
pub fn run_update(
    host: SkillHost,
    dry_run: bool,
    skill_env: &SkillEnv,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    emit(
        &compute_update_envelope(host, dry_run, skill_env),
        out,
        stdout,
    )
}

/// Multi-host wrapper for update. When `all` is set, refreshes every entry in
/// [`KNOWN_HOSTS`] that holds a copy and reports the rest as skipped, in one
/// multi envelope as `skill install --all` prints; otherwise dispatches to
/// [`run_update`] for the single host.
pub fn run_update_multi(
    host: Option<SkillHost>,
    all: bool,
    dry_run: bool,
    skill_env: &SkillEnv,
    out: &OutputConfig,
    stdout: &mut dyn Write,
) -> i32 {
    if all {
        return run_for_all_hosts(ACTION_UPDATE, dry_run, out, stdout, |host| {
            if is_installed(host, skill_env) {
                compute_update_envelope(host, dry_run, skill_env)
            } else {
                skipped_envelope(host, skill_env)
            }
        });
    }
    let Some(host) = host else {
        return emit_missing_host_envelope(out, stdout);
    };
    run_update(host, dry_run, skill_env, out, stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn skill_repo_url() -> &'static str {
        resolve_host(SkillHost::ClaudeCode).0
    }

    /// A skill environment with only `HOME`, at `home`.
    fn at(home: &TempDir) -> SkillEnv {
        SkillEnv {
            home: home.path().to_str().map(String::from),
            ..SkillEnv::default()
        }
    }

    /// The first host with a legacy location, with a copy created there under
    /// `home`.
    fn host_with_legacy_copy(home: &TempDir) -> (SkillHost, PathBuf) {
        let host = SkillHost::value_variants()
            .iter()
            .copied()
            .find(|&host| legacy_destination(host).is_some())
            .expect("a host with a legacy location");
        let legacy = expand_tilde_with(
            legacy_destination(host).expect("found above"),
            home.path().to_str(),
        )
        .expect("home is set in the test");
        std::fs::create_dir_all(&legacy).expect("create the legacy copy");
        (host, legacy)
    }

    #[test]
    fn install_names_a_legacy_copy_and_leaves_it_in_place() {
        let home = TempDir::new().expect("tempdir");
        let (host, legacy) = host_with_legacy_copy(&home);

        let envelope = compute_install_envelope(host, true, &at(&home));
        assert_eq!(envelope.legacy_install_dir.as_deref(), legacy.to_str());
        assert!(legacy.exists(), "install leaves the legacy copy alone");

        let text = render_envelope(&envelope, &OutputFormat::Text);
        assert!(
            text.contains(&format!("bird skill update {}", envelope.host)),
            "{text}"
        );
    }

    #[test]
    fn an_update_dry_run_names_the_legacy_copy_and_removes_nothing() {
        let home = TempDir::new().expect("tempdir");
        let (host, legacy) = host_with_legacy_copy(&home);

        let envelope = compute_update_envelope(host, true, &at(&home));
        assert_eq!(envelope.action, "skill-update");
        assert_eq!(envelope.status, "dry_run");
        assert_eq!(envelope.legacy_install_dir.as_deref(), legacy.to_str());
        assert!(legacy.exists(), "a dry run removes nothing");
    }

    #[test]
    fn update_without_home_is_an_error_naming_the_reason() {
        let envelope = compute_update_envelope(SkillHost::ClaudeCode, true, &SkillEnv::default());
        assert_eq!(envelope.action, "skill-update");
        assert_eq!(envelope.status, "error");
        assert_eq!(envelope.reason, Some("home-not-set"));
        assert_eq!(envelope.exit_code, Some(1));
        assert_eq!(envelope.would_succeed, Some(false));
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
