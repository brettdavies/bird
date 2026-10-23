//! `bird skill update` orchestration. Owns the remove-then-reinstall verb for
//! one host and for every host, plus the skip rule that keeps `--all` from
//! creating installations.

use std::fs;
use std::io::Write;

use crate::output::OutputConfig;

use super::{
    ACTION_UPDATE, DestinationStatus, InstallEnvelope, REASON_NOT_INSTALLED, REASON_REMOVE_FAILED,
    STATUS_DRY_RUN, STATUS_ERROR, STATUS_SKIPPED, SkillEnv, SkillHost, emit,
    emit_missing_host_envelope, format_clone_command, host_envelope_str, install_envelope,
    missing_home_envelope, resolve_host, run_for_all_hosts,
};

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
/// [`KNOWN_HOSTS`](super::KNOWN_HOSTS) that holds a copy and reports the rest
/// as skipped, in one multi envelope as `skill install --all` prints;
/// otherwise dispatches to [`run_update`] for the single host.
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
    use crate::skill_install::tests::{at, host_with_legacy_copy};
    use tempfile::TempDir;

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
}
