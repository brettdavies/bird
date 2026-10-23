//! `bird cache clear` and `bird cache stats` — the store's response files,
//! archive, and ledger.

use crate::cli::dispatch::{GuardOutcome, require_confirmation};
use crate::cli::{CacheAction, OutputFlags};
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::store::StorePaths;
use crate::store::report::StoreReport;
use crate::store::responses::Responses;
use std::io::Write;

pub fn run(
    paths: &StorePaths,
    out: &OutputConfig,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    action: CacheAction,
    no_interactive: bool,
) -> Result<(), BirdError> {
    match action {
        CacheAction::Clear { guard } => {
            let target = paths.responses_dir.display().to_string();
            match require_confirmation(
                "clear",
                "LOCAL",
                &target,
                None,
                guard,
                out,
                no_interactive,
                stdout,
                stderr,
                None,
            )? {
                GuardOutcome::DryRun | GuardOutcome::Declined => return Ok(()),
                GuardOutcome::Proceed => {}
            }
            let cleared = Responses::new(paths.responses_dir.clone())
                .clear()
                .map_err(|e| BirdError::from_source("cache", Box::new(e)))?;
            let message = format!(
                "Cleared {} stored responses ({:.1} MB); kept the archive at {}.",
                cleared.files,
                cleared.bytes as f64 / (1024.0 * 1024.0),
                paths.archive_dir.display()
            );
            if out.format.is_json() {
                out.print_ok_message(stdout, &message)
                    .map_err(cache_io_err)?;
            } else if !out.suppress_diag() {
                writeln!(stderr, "{message}").ok();
            }
        }
        CacheAction::Stats {
            common: OutputFlags { pretty },
        } => {
            let report = StoreReport::gather(paths, chrono::Utc::now());
            if out.is_raw_text() && !pretty {
                write_raw(stdout, &report).map_err(cache_io_err)?;
            } else if out.report_as_text(pretty) {
                write_pretty(stdout, &report).map_err(cache_io_err)?;
            } else {
                let value = serde_json::to_value(&report)
                    .map_err(|e| BirdError::from_source("cache", Box::new(e)))?;
                out.print_ok(stdout, value).map_err(cache_io_err)?;
            }
        }
    }
    Ok(())
}

fn write_pretty(w: &mut dyn Write, r: &StoreReport) -> std::io::Result<()> {
    let a = &r.archive;
    writeln!(w, "Archive:   {} ({})", a.path, a.source.as_str())?;
    writeln!(
        w,
        "  posts {}, users {}, bookmarks {}, likes {}, {:.1} MB",
        a.posts, a.users, a.bookmarks, a.likes, a.size_mb
    )?;
    let s = &r.responses;
    writeln!(w, "Responses: {}", s.path)?;
    writeln!(
        w,
        "  {} files, {} from today, {:.1} MB",
        s.files, s.fresh, s.size_mb
    )?;
    writeln!(w, "Ledger:    {}", r.ledger.path)?;
    writeln!(w, "  {:.1} MB", r.ledger.size_mb)?;
    let status = if r.healthy { "healthy" } else { "unhealthy" };
    writeln!(w, "Status:    {status}")
}

/// One `key=value` per line, pipe-safe.
fn write_raw(w: &mut dyn Write, r: &StoreReport) -> std::io::Result<()> {
    let a = &r.archive;
    writeln!(w, "archive.path={}", a.path)?;
    writeln!(w, "archive.source={}", a.source.as_str())?;
    writeln!(w, "archive.exists={}", a.exists)?;
    writeln!(w, "archive.writable={}", a.writable)?;
    writeln!(w, "archive.posts={}", a.posts)?;
    writeln!(w, "archive.users={}", a.users)?;
    writeln!(w, "archive.bookmarks={}", a.bookmarks)?;
    writeln!(w, "archive.likes={}", a.likes)?;
    writeln!(w, "archive.size_mb={:.1}", a.size_mb)?;
    let s = &r.responses;
    writeln!(w, "responses.path={}", s.path)?;
    writeln!(w, "responses.exists={}", s.exists)?;
    writeln!(w, "responses.writable={}", s.writable)?;
    writeln!(w, "responses.files={}", s.files)?;
    writeln!(w, "responses.fresh={}", s.fresh)?;
    writeln!(w, "responses.size_mb={:.1}", s.size_mb)?;
    writeln!(w, "ledger.path={}", r.ledger.path)?;
    writeln!(w, "ledger.exists={}", r.ledger.exists)?;
    writeln!(w, "ledger.size_mb={:.1}", r.ledger.size_mb)?;
    writeln!(w, "healthy={}", r.healthy)
}

fn cache_io_err(e: std::io::Error) -> BirdError {
    BirdError::general("cache", Box::<dyn std::error::Error + Send + Sync>::from(e))
}
