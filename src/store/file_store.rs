//! `FileStore`: the archive, the response files, and the ledger behind one
//! handle, with every failure reported to the run's [`StoreStatus`].

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::archive::Archive;
use super::ledger::{Ledger, Record};
use super::paths::StorePaths;
use super::responses::{Account, Responses, StoredResponse, is_answerable};
use super::status::{Area, StoreStatus};

/// The app a run's requests go out under and that app's default user, read
/// once from the token store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoreAccount {
    /// The `--app` value, else the store's default app.
    pub app: String,
    /// The app's default user; `None` when the store names none.
    pub default_user: Option<String>,
}

impl StoreAccount {
    /// The account a request goes out under (plan 005 KTD8). `None` for a
    /// user-context request that names no user.
    pub fn for_request(&self, app_only: bool, requested: Option<&str>) -> Option<Account> {
        Account::for_request(&self.app, app_only, requested, self.default_user.as_deref())
    }
}

/// The run's file store.
pub struct FileStore {
    responses: Responses,
    archive: Archive,
    ledger: Ledger,
    status: Arc<StoreStatus>,
}

impl FileStore {
    /// The store at `paths`; nothing is touched until it is used.
    pub fn new(paths: &StorePaths, status: Arc<StoreStatus>) -> Self {
        Self {
            responses: Responses::new(paths.responses_dir.clone()),
            archive: Archive::new(paths.archive_dir.clone(), Arc::clone(&status)),
            ledger: Ledger::new(paths),
            status,
        }
    }

    /// Removes response files from before today, once per UTC day.
    pub fn prune(&self, now: DateTime<Utc>) {
        if let Err(e) = self.responses.prune_if_due(now) {
            self.fail(Area::Responses, "response prune", self.responses.dir(), &e);
        }
    }

    /// The same-day response for this request and account, when one is
    /// stored.
    pub fn answer(
        &self,
        method: &str,
        url: &str,
        account: Option<&Account>,
        now: DateTime<Utc>,
    ) -> Option<StoredResponse> {
        if !is_answerable(method, url) {
            return None;
        }
        self.responses.read(method, url, account?, now)
    }

    /// Stores a sent request's 2xx body: its response file and its notes.
    /// Only answerable requests are stored, so direct messages, usage, and
    /// media uploads never reach disk.
    pub fn observe(
        &self,
        method: &str,
        url: &str,
        account: Option<&Account>,
        status: u16,
        body: &serde_json::Value,
        now: DateTime<Utc>,
    ) {
        if !is_answerable(method, url) || !(200..300).contains(&status) {
            return;
        }
        if let Some(account) = account
            && let Err(e) = self.responses.write(method, url, account, status, body)
        {
            let path = self.responses.path_for(method, url, account);
            self.fail(Area::Responses, "response write", &path, &e);
        }
        let username = account.map(|a| a.user.as_str()).filter(|u| !u.is_empty());
        self.archive.observe(url, body, username, now);
    }

    /// Appends one ledger record.
    pub fn record(&self, record: &Record, now: DateTime<Utc>) {
        if let Err(e) = self.ledger.append(record, now) {
            self.fail(Area::Ledger, "ledger append", self.ledger.path(), &e);
        }
    }

    /// Removes the account's response files after a write it sent, so a
    /// later read shows the state after the write.
    pub fn invalidate(&self, account: Option<&Account>) {
        let Some(account) = account else { return };
        if let Err(e) = self.responses.invalidate(account) {
            self.fail(Area::Responses, "response clear", self.responses.dir(), &e);
        }
    }

    /// The response files.
    pub fn responses(&self) -> &Responses {
        &self.responses
    }

    /// The Markdown archive.
    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    /// The spend ledger.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// The run's store failures.
    pub fn status(&self) -> &Arc<StoreStatus> {
        &self.status
    }

    fn fail(&self, area: Area, what: &'static str, path: &Path, err: &std::io::Error) {
        self.status.fail(area, what, path, err);
    }
}
