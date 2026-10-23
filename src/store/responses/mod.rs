//! Response files: a GET's 2xx body, stored per account, answers the same
//! request until the UTC day ends.
//!
//! Layout: `<dir>/<account_key>/<request_key>.json`, plus a `.pruned` marker
//! whose mtime records the day's prune.

mod key;
#[cfg(test)]
mod tests;

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::atomic::{self, Durability};
use key::normalize_url;
pub use key::{
    Account, account_key, is_account_dir_name, is_answerable, is_request_file_name, request_key,
};

const FORMAT: u32 = 1;
const PRUNED_MARKER: &str = ".pruned";

/// A stored answer: the status and whole decoded body X returned.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredResponse {
    /// The HTTP status, always 2xx.
    pub status: u16,
    /// The whole body, `errors` and all.
    pub body: serde_json::Value,
}

/// What [`Responses::clear`] removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cleared {
    /// Response and temporary files removed.
    pub files: u64,
    /// Their total size.
    pub bytes: u64,
}

#[derive(Serialize, Deserialize)]
struct StoredFile {
    format: u32,
    method: String,
    url: String,
    app: String,
    user: String,
    status: u16,
    body: serde_json::Value,
}

/// The response-file directory.
pub struct Responses {
    dir: PathBuf,
}

impl Responses {
    /// Response files under `dir`, which need not exist yet.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The directory the files live in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the response to a request for `account` is filed.
    pub fn path_for(&self, method: &str, url: &str, account: &Account) -> PathBuf {
        self.account_dir(account)
            .join(format!("{}.json", request_key(method, url)))
    }

    fn account_dir(&self, account: &Account) -> PathBuf {
        self.dir.join(account_key(account))
    }

    /// The stored answer for this request, when one was written for the same
    /// account the same UTC day as `now` and records this exact request. An
    /// unreadable or mismatched file is a miss.
    pub fn read(
        &self,
        method: &str,
        url: &str,
        account: &Account,
        now: DateTime<Utc>,
    ) -> Option<StoredResponse> {
        if !is_answerable(method, url) {
            return None;
        }
        let path = self.path_for(method, url, account);
        let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
        if utc_date(modified) != now.date_naive() {
            return None;
        }
        let stored: StoredFile = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
        let matches = stored.format == FORMAT
            && stored.method == method
            && stored.url == normalize_url(url)
            && stored.app == account.app
            && stored.user == account.user;
        matches.then_some(StoredResponse {
            status: stored.status,
            body: stored.body,
        })
    }

    /// Stores a response. A non-2xx status or a request that is never stored
    /// writes nothing.
    pub fn write(
        &self,
        method: &str,
        url: &str,
        account: &Account,
        status: u16,
        body: &serde_json::Value,
    ) -> io::Result<()> {
        if !(200..300).contains(&status) || !is_answerable(method, url) {
            return Ok(());
        }
        let stored = StoredFile {
            format: FORMAT,
            method: method.to_string(),
            url: normalize_url(url),
            app: account.app.clone(),
            user: account.user.clone(),
            status,
            body: body.clone(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(io::Error::other)?;
        atomic::write(
            &self.path_for(method, url, account),
            &bytes,
            Durability::Unsynced,
        )
    }

    /// Removes every response stored for `account`, so a read after its own
    /// write is sent.
    pub fn invalidate(&self, account: &Account) -> io::Result<()> {
        match std::fs::remove_dir_all(self.account_dir(account)) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    /// Removes response files older than `now`'s UTC day and the account
    /// directories that leaves empty, at most once per day. Returns whether it
    /// scanned.
    pub fn prune_if_due(&self, now: DateTime<Utc>) -> io::Result<bool> {
        let marker = self.dir.join(PRUNED_MARKER);
        let today = now.date_naive();
        let pruned_today = std::fs::metadata(&marker)
            .and_then(|m| m.modified())
            .is_ok_and(|t| utc_date(t) == today);
        if pruned_today || !self.dir.is_dir() {
            return Ok(false);
        }
        self.sweep(|modified| utc_date(modified) < today)?;
        atomic::write(&marker, b"", Durability::Unsynced)?;
        Ok(true)
    }

    /// Removes every response file and bird temporary file, leaving anything
    /// else in the directory alone.
    pub fn clear(&self) -> io::Result<Cleared> {
        if !self.dir.is_dir() {
            return Ok(Cleared::default());
        }
        self.sweep(|_| true)
    }

    /// Removes bird's files that `doomed` picks by mtime from every account
    /// directory, then each account directory left empty.
    fn sweep(&self, doomed: impl Fn(SystemTime) -> bool) -> io::Result<Cleared> {
        let mut cleared = Cleared::default();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if is_account_dir_name(&name) {
                    sweep_account(&path, &doomed, &mut cleared)?;
                }
            } else if atomic::is_temp_name(&name) {
                remove_counted(&path, &entry.metadata()?, &doomed, &mut cleared)?;
            }
        }
        Ok(cleared)
    }
}

fn sweep_account(
    dir: &Path,
    doomed: &impl Fn(SystemTime) -> bool,
    cleared: &mut Cleared,
) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if is_request_file_name(&name) || atomic::is_temp_name(&name) {
            remove_counted(&entry.path(), &entry.metadata()?, doomed, cleared)?;
        }
    }
    if std::fs::read_dir(dir)?.next().is_none() {
        std::fs::remove_dir(dir)?;
    }
    Ok(())
}

fn remove_counted(
    path: &Path,
    meta: &std::fs::Metadata,
    doomed: &impl Fn(SystemTime) -> bool,
    cleared: &mut Cleared,
) -> io::Result<()> {
    if !meta.is_file() || !doomed(meta.modified()?) {
        return Ok(());
    }
    std::fs::remove_file(path)?;
    cleared.files += 1;
    cleared.bytes += meta.len();
    Ok(())
}

fn utc_date(t: SystemTime) -> NaiveDate {
    DateTime::<Utc>::from(t).date_naive()
}
