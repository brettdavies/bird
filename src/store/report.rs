//! `StoreReport`: what `bird cache stats` and the `cache` section of
//! `bird doctor` print about the store's three locations.
//!
//! Reporting reads directories and file metadata only, so it writes nothing
//! into the user's vault. A location's `writable` is the access check of the
//! location, or of its nearest existing ancestor when it does not exist yet.

use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::paths::{ArchiveSource, StorePaths};
use super::responses::{is_account_dir_name, is_request_file_name};

/// The archive directory.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArchiveReport {
    pub path: String,
    pub source: ArchiveSource,
    pub exists: bool,
    pub writable: bool,
    pub posts: u64,
    pub users: u64,
    pub bookmarks: u64,
    pub likes: u64,
    pub size_mb: f64,
}

/// The response files.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResponsesReport {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub files: u64,
    /// Files written today (UTC), which still answer requests.
    pub fresh: u64,
    pub size_mb: f64,
}

/// The spend ledger.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LedgerReport {
    pub path: String,
    pub exists: bool,
    pub size_mb: f64,
}

/// The store's three locations and whether bird can write each of them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StoreReport {
    pub archive: ArchiveReport,
    pub responses: ResponsesReport,
    pub ledger: LedgerReport,
    /// True when the archive, the responses directory, and the ledger's
    /// directory are all writable.
    pub healthy: bool,
}

#[derive(Default)]
struct Tally {
    files: u64,
    fresh: u64,
    bytes: u64,
}

impl StoreReport {
    /// Reads the store at `paths` as of `now`.
    pub fn gather(paths: &StorePaths, now: DateTime<Utc>) -> Self {
        let today = now.date_naive();
        let archive_dir = &paths.archive_dir;
        let notes =
            |kind: &str| tally_notes(&archive_dir.join(kind), kind == "posts" || kind == "users");
        let (posts, users, bookmarks, likes) = (
            notes("posts"),
            notes("users"),
            notes("bookmarks"),
            notes("likes"),
        );
        let archive_bytes = posts.bytes + users.bytes + bookmarks.bytes + likes.bytes;
        let archive = ArchiveReport {
            path: archive_dir.display().to_string(),
            source: paths.archive_source,
            exists: archive_dir.is_dir(),
            writable: writable(archive_dir),
            posts: posts.files,
            users: users.files,
            bookmarks: bookmarks.files,
            likes: likes.files,
            size_mb: megabytes(archive_bytes),
        };
        let stored = tally_responses(&paths.responses_dir, today);
        let responses = ResponsesReport {
            path: paths.responses_dir.display().to_string(),
            exists: paths.responses_dir.is_dir(),
            writable: writable(&paths.responses_dir),
            files: stored.files,
            fresh: stored.fresh,
            size_mb: megabytes(stored.bytes),
        };
        let ledger_meta = std::fs::metadata(&paths.ledger).ok();
        let ledger = LedgerReport {
            path: paths.ledger.display().to_string(),
            exists: ledger_meta.is_some(),
            size_mb: megabytes(ledger_meta.map_or(0, |m| m.len())),
        };
        let ledger_writable = paths.ledger.parent().is_some_and(writable);
        Self {
            healthy: archive.writable && responses.writable && ledger_writable,
            archive,
            responses,
            ledger,
        }
    }
}

/// Counts `<dir>/<id>.md` notes, or `<dir>/<account>/<id>.md` when not
/// `flat`.
fn tally_notes(dir: &Path, flat: bool) -> Tally {
    let mut tally = Tally::default();
    let dirs: Vec<std::path::PathBuf> = if flat {
        vec![dir.to_path_buf()]
    } else {
        entries(dir)
            .filter(|(_, meta)| meta.is_dir())
            .map(|(path, _)| path)
            .collect()
    };
    for dir in dirs {
        for (path, meta) in entries(&dir) {
            if meta.is_file() && path.extension().is_some_and(|e| e == "md") {
                tally.files += 1;
                tally.bytes += meta.len();
            }
        }
    }
    tally
}

/// Counts response files across account directories, and those written on
/// `today`.
fn tally_responses(dir: &Path, today: chrono::NaiveDate) -> Tally {
    let mut tally = Tally::default();
    for (account, meta) in entries(dir) {
        let name = account
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        if !meta.is_dir() || !name.as_deref().is_some_and(is_account_dir_name) {
            continue;
        }
        for (path, meta) in entries(&account) {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            if !meta.is_file() || !name.as_deref().is_some_and(is_request_file_name) {
                continue;
            }
            tally.files += 1;
            tally.bytes += meta.len();
            if meta.modified().is_ok_and(|t| utc_date(t) == today) {
                tally.fresh += 1;
            }
        }
    }
    tally
}

fn entries(dir: &Path) -> impl Iterator<Item = (std::path::PathBuf, std::fs::Metadata)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok().map(|meta| (entry.path(), meta)))
}

fn utc_date(t: SystemTime) -> chrono::NaiveDate {
    DateTime::<Utc>::from(t).date_naive()
}

fn megabytes(bytes: u64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0
}

/// Whether bird could write at `path`, judged at the nearest existing
/// ancestor when `path` does not exist.
fn writable(path: &Path) -> bool {
    let mut probe = path;
    loop {
        if probe.exists() {
            return can_write(probe);
        }
        match probe.parent() {
            Some(parent) => probe = parent,
            None => return false,
        }
    }
}

#[cfg(unix)]
fn can_write(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call.
    unsafe { libc::access(c_path.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn can_write(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| !m.permissions().readonly())
}

#[cfg(test)]
mod tests;
