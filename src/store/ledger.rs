//! The spend ledger: one JSONL file of request and actual-usage records.
//!
//! Appends hold a shared lock on the sibling `spend.lock` and compaction an
//! exclusive one, because compaction replaces the ledger's inode and an
//! append racing it would land in the replaced file. Each append is one
//! whole line in one write. A run whose first record is older than
//! [`RETENTION_DAYS`] compacts once.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::atomic::{self, Durability};
use super::paths::StorePaths;

/// Days of request records the ledger keeps.
pub const RETENTION_DAYS: i64 = 90;

/// The record format version.
pub const FORMAT: u32 = 1;

/// One priced request, sent or answered from the store.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequestRecord {
    pub v: u32,
    /// RFC 3339 UTC.
    pub ts: String,
    pub method: String,
    /// As `normalize_endpoint` gives it.
    pub endpoint: String,
    /// `post`, `user`, `write`, or `none`.
    pub object_type: String,
    pub object_count: i64,
    /// USD.
    pub estimated_cost: f64,
    pub cache_hit: bool,
    /// Empty when none.
    pub app: String,
    /// Empty when none.
    pub username: String,
}

/// X's reported usage for one day, from `bird spend --sync`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActualRecord {
    pub v: u32,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub post_count: i64,
    /// RFC 3339 UTC.
    pub synced_at: String,
}

/// One ledger line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Record {
    Request(RequestRecord),
    Actual(ActualRecord),
}

impl Record {
    /// When the record was written, for retention.
    fn timestamp(&self) -> Option<DateTime<Utc>> {
        let ts = match self {
            Self::Request(r) => &r.ts,
            Self::Actual(a) => &a.synced_at,
        };
        DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|t| t.with_timezone(&Utc))
    }
}

/// What a read found: every record that parsed, and how many lines did not.
#[derive(Debug, Default)]
pub struct Contents {
    pub records: Vec<Record>,
    pub skipped: usize,
}

/// The ledger file and its lock.
pub struct Ledger {
    path: PathBuf,
    lock: PathBuf,
    retention_checked: AtomicBool,
}

impl Ledger {
    /// The ledger at the store's location.
    pub fn new(paths: &StorePaths) -> Self {
        Self::at(paths.ledger.clone(), paths.ledger_lock())
    }

    /// A ledger at `path` locked through `lock`.
    pub fn at(path: PathBuf, lock: PathBuf) -> Self {
        Self {
            path,
            lock,
            retention_checked: AtomicBool::new(false),
        }
    }

    /// The ledger file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one record. The first append of a run compacts when the
    /// ledger's first record is past retention. Where the filesystem refuses
    /// the lock, the record is appended unlocked, compaction is skipped, and
    /// the lock error is returned for the store warning.
    pub fn append(&self, record: &Record, now: DateTime<Utc>) -> io::Result<()> {
        let mut line = serde_json::to_vec(record).map_err(io::Error::other)?;
        line.push(b'\n');
        let lock = self.open_lock()?;
        let locked = lock.lock_shared();
        append_line(&self.path, &line)?;
        drop(lock);
        locked?;
        if !self.retention_checked.swap(true, Ordering::Relaxed) && self.first_is_expired(now)? {
            self.compact(now)?;
        }
        Ok(())
    }

    /// Reads every record, skipping lines that do not parse. A missing
    /// ledger is empty.
    pub fn read(&self) -> io::Result<Contents> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Contents::default()),
            Err(e) => return Err(e),
        };
        let mut contents = Contents::default();
        for line in BufReader::new(file).split(b'\n') {
            let line = line?;
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            match serde_json::from_slice::<Record>(&line) {
                Ok(record) => contents.records.push(record),
                Err(_) => contents.skipped += 1,
            }
        }
        Ok(contents)
    }

    /// Drops request records past retention and keeps each date's latest
    /// actual record, in file order, under the exclusive lock.
    pub fn compact(&self, now: DateTime<Utc>) -> io::Result<()> {
        let lock = self.open_lock()?;
        lock.lock()?;
        let cutoff = now - TimeDelta::days(RETENTION_DAYS);
        let records = self.read()?.records;
        let mut kept: Vec<&Record> = Vec::with_capacity(records.len());
        for (i, record) in records.iter().enumerate() {
            let keep = match record {
                Record::Request(_) => record.timestamp().is_some_and(|t| t >= cutoff),
                Record::Actual(a) => !records[i + 1..]
                    .iter()
                    .any(|later| matches!(later, Record::Actual(b) if b.date == a.date)),
            };
            if keep {
                kept.push(record);
            }
        }
        let mut bytes = Vec::new();
        for record in kept {
            serde_json::to_writer(&mut bytes, record).map_err(io::Error::other)?;
            bytes.push(b'\n');
        }
        atomic::write(&self.path, &bytes, Durability::Synced)
    }

    fn first_is_expired(&self, now: DateTime<Utc>) -> io::Result<bool> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        let mut first = Vec::new();
        BufReader::new(file).read_until(b'\n', &mut first)?;
        let cutoff = now - TimeDelta::days(RETENTION_DAYS);
        Ok(serde_json::from_slice::<Record>(&first)
            .ok()
            .and_then(|r| r.timestamp())
            .is_some_and(|t| t < cutoff))
    }

    fn open_lock(&self) -> io::Result<File> {
        if let Some(dir) = self.lock.parent() {
            atomic::create_dir(dir)?;
        }
        private_options().write(true).open(&self.lock)
    }
}

fn append_line(path: &Path, line: &[u8]) -> io::Result<()> {
    let mut file = private_options().append(true).open(path)?;
    file.write_all(line)
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

#[cfg(test)]
pub(crate) mod tests;
