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

use std::io::Write;

use crate::output::OutputConfig;
use crate::output::envelope::NextStep;

mod destination;
mod git;
mod render;
mod update;

pub use destination::{DestinationStatus, SkillEnv, check_destination, expand_tilde_with};
pub use git::{
    GIT_HARDEN_ENV_REMOVE, GIT_HARDEN_ENV_SET, GIT_HARDEN_FLAGS, build_clone_command,
    format_clone_command,
};
pub use update::{compute_update_envelope, run_update, run_update_multi};

use git::spawn_git_clone;
use render::{emit, render_multi, render_structured};

// `SkillHost`, `KNOWN_HOSTS`, `resolve_host`, `host_envelope_str`,
// `config_dir_env`, `CONFIG_DIR_VARS`, `base_dir_env`, `BASE_DIR_VARS`, and
// `legacy_destination` are auto-generated at build time from
// `src/skill_install/skill.json`. Edit the JSON file to add or remove hosts or
// change a host's variables; `cargo build` regenerates this file.
include!(concat!(env!("OUT_DIR"), "/generated_hosts.rs"));

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

#[cfg(test)]
mod tests {
    use super::render::render_envelope;
    use super::*;
    use crate::output::OutputFormat;
    use clap::ValueEnum;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// A skill environment with only `HOME`, at `home`.
    pub(super) fn at(home: &TempDir) -> SkillEnv {
        SkillEnv {
            home: home.path().to_str().map(String::from),
            ..SkillEnv::default()
        }
    }

    /// The first host with a legacy location, with a copy created there under
    /// `home`.
    pub(super) fn host_with_legacy_copy(home: &TempDir) -> (SkillHost, PathBuf) {
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
