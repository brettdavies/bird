//! Envelope rendering for the install and update verbs. Owns the per-format
//! serialization of the single-host and multi-host envelopes and the writer.

use std::io::Write;

use crate::output::{OutputConfig, OutputFormat};

use super::{
    ACTION_UPDATE, InstallEnvelope, InstallMultiEnvelope, STATUS_DRY_RUN, STATUS_ERROR, STATUS_OK,
    STATUS_SKIPPED,
};

/// Render a single install envelope per the active output format. Text mode
/// adds a note line when the envelope names a copy at the host's legacy
/// location.
pub(super) fn render_envelope(env: &InstallEnvelope, format: &OutputFormat) -> String {
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
pub(super) fn render_multi(env: &InstallMultiEnvelope, format: &OutputFormat) -> String {
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
pub(super) fn render_structured<T: serde::Serialize>(env: &T, format: &OutputFormat) -> String {
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
pub(super) fn emit(envelope: &InstallEnvelope, out: &OutputConfig, stdout: &mut dyn Write) -> i32 {
    let rendered = render_envelope(envelope, &out.format);
    let _ = writeln!(stdout, "{rendered}");
    if envelope.status == STATUS_ERROR {
        envelope.exit_code.unwrap_or(1)
    } else {
        0
    }
}
