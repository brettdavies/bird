//! One `StoreStatus` per run: the first store failure, warned once.
//!
//! A store failure never changes a printing command's result. The first one
//! prints `[store] warning: <what> failed for <path>: <os error>` unless the
//! run is quiet, and later ones print nothing. A command whose job is storing
//! reads the first archive failure back and fails on it.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// The part of the store a failure happened in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    /// A Markdown note.
    Archive,
    /// A response file or the responses directory.
    Responses,
    /// The spend ledger.
    Ledger,
    /// The 0.2.0 database files.
    Legacy,
}

/// One recorded store failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Which part of the store failed.
    pub area: Area,
    /// What was being done, as the warning names it (`archive write`).
    pub what: &'static str,
    /// The file or directory it was done to.
    pub path: PathBuf,
    /// The OS error's kind.
    pub kind: io::ErrorKind,
    /// The OS error's message.
    pub error: String,
}

#[derive(Default)]
struct Recorded {
    first: Option<Failure>,
    first_archive: Option<Failure>,
}

/// The run's store failures, shared by every store component.
pub struct StoreStatus {
    quiet: bool,
    stderr: Arc<Mutex<dyn Write + Send>>,
    recorded: Mutex<Recorded>,
}

impl StoreStatus {
    /// A status that warns through `stderr` unless `quiet`.
    pub fn new(quiet: bool, stderr: Arc<Mutex<dyn Write + Send>>) -> Arc<Self> {
        Arc::new(Self {
            quiet,
            stderr,
            recorded: Mutex::new(Recorded::default()),
        })
    }

    /// A status whose warnings go nowhere, for tests and for callers that
    /// read failures back instead.
    pub fn silent() -> Arc<Self> {
        Self::new(true, Arc::new(Mutex::new(io::sink())))
    }

    /// Records a failure; the first one of the run prints the warning.
    pub fn fail(&self, area: Area, what: &'static str, path: &Path, err: &io::Error) {
        let failure = Failure {
            area,
            what,
            path: path.to_path_buf(),
            kind: err.kind(),
            error: err.to_string(),
        };
        let first = {
            let mut recorded = self.recorded.lock().unwrap_or_else(PoisonError::into_inner);
            if area == Area::Archive && recorded.first_archive.is_none() {
                recorded.first_archive = Some(failure.clone());
            }
            if recorded.first.is_some() {
                return;
            }
            recorded.first = Some(failure);
            recorded.first.clone()
        };
        if !self.quiet
            && let Some(f) = first
        {
            let mut w = crate::output::lock_writer(&self.stderr);
            writeln!(
                *w,
                "[store] warning: {} failed for {}: {}",
                f.what,
                f.path.display(),
                f.error
            )
            .ok();
        }
    }

    /// Prints one informational store line unless the run is quiet.
    pub fn notice(&self, line: std::fmt::Arguments<'_>) {
        if !self.quiet {
            let mut w = crate::output::lock_writer(&self.stderr);
            writeln!(*w, "[store] {line}").ok();
        }
    }

    /// The run's first store failure, of any area.
    pub fn first_failure(&self) -> Option<Failure> {
        self.recorded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .first
            .clone()
    }

    /// The run's first archive write failure, for a command that must not
    /// succeed without its notes.
    pub fn first_archive_failure(&self) -> Option<Failure> {
        self.recorded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .first_archive
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PanicOnWrite;

    impl Write for PanicOnWrite {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            panic!("quiet store failure reached the writer");
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("quiet store failure reached the writer");
        }
    }

    fn captured(quiet: bool) -> (Arc<StoreStatus>, Arc<Mutex<Vec<u8>>>) {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let stderr: Arc<Mutex<dyn Write + Send>> = buf.clone();
        (StoreStatus::new(quiet, stderr), buf)
    }

    fn text(buf: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8")
    }

    fn denied() -> io::Error {
        io::Error::from(io::ErrorKind::PermissionDenied)
    }

    #[test]
    fn the_first_failure_warns_once() {
        let (status, buf) = captured(false);
        status.fail(
            Area::Responses,
            "response write",
            Path::new("/r/a.json"),
            &denied(),
        );
        status.fail(
            Area::Archive,
            "archive write",
            Path::new("/a/1.md"),
            &denied(),
        );
        status.fail(Area::Ledger, "ledger append", Path::new("/l"), &denied());
        let out = text(&buf);
        assert_eq!(out.lines().count(), 1, "{out}");
        assert_eq!(
            out,
            format!(
                "[store] warning: response write failed for /r/a.json: {}\n",
                denied()
            )
        );
    }

    #[test]
    fn quiet_store_failures_touch_no_writer() {
        let stderr: Arc<Mutex<dyn Write + Send>> = Arc::new(Mutex::new(PanicOnWrite));
        let status = StoreStatus::new(true, stderr);
        status.fail(
            Area::Archive,
            "archive write",
            Path::new("/a/1.md"),
            &denied(),
        );
        status.notice(format_args!("removed the 0.2.0 store at /c/bird.db"));
        assert!(status.first_failure().is_some());
    }

    #[test]
    fn strict_callers_see_the_first_archive_failure() {
        let status = StoreStatus::silent();
        assert!(status.first_archive_failure().is_none());
        status.fail(Area::Ledger, "ledger append", Path::new("/l"), &denied());
        assert!(status.first_archive_failure().is_none());
        status.fail(
            Area::Archive,
            "archive write",
            Path::new("/a/1.md"),
            &denied(),
        );
        status.fail(
            Area::Archive,
            "archive write",
            Path::new("/a/2.md"),
            &denied(),
        );
        let failure = status.first_archive_failure().expect("archive failure");
        assert_eq!(failure.path, PathBuf::from("/a/1.md"));
        assert_eq!(failure.kind, io::ErrorKind::PermissionDenied);
        assert_eq!(failure.error, denied().to_string());
        assert_eq!(
            status.first_failure().expect("first").area,
            Area::Ledger,
            "the first failure of any area is kept separately"
        );
    }

    #[test]
    fn a_notice_prints_one_line() {
        let (status, buf) = captured(false);
        status.notice(format_args!("removed the 0.2.0 store at /c/bird.db"));
        assert_eq!(
            text(&buf),
            "[store] removed the 0.2.0 store at /c/bird.db\n"
        );
    }
}
