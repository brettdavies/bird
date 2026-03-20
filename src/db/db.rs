//! BirdDb: entity-level SQLite store replacing the request-level cache.
//! Stores tweets, users, bookmarks, and raw responses keyed by entity ID.
//! Single connection per CLI invocation -- no pool needed (short-lived process).
//! Blocking SQLite calls are fine: bird is synchronous (no async runtime).

use rusqlite::Connection;
use rusqlite::params;
use rusqlite_migration::{M, Migrations};
use std::path::{Path, PathBuf};

use super::unix_now;
use crate::diag;

// -- Model structs --

#[derive(Debug, Clone)]
pub struct TweetRow {
    pub id: String,
    pub author_id: Option<String>,
    pub conversation_id: Option<String>,
    pub raw_json: String,
    pub last_refreshed_at: i64,
}

impl TweetRow {
    /// Extract a TweetRow from an X API tweet JSON object.
    /// Stores the full JSON as `raw_json`; only extracts lookup-relevant fields.
    pub fn from_api_json(json: &serde_json::Value) -> Option<Self> {
        let id = json.get("id")?.as_str()?.to_string();
        let raw_json = serde_json::to_string(json).ok()?;
        Some(Self {
            id,
            author_id: json
                .get("author_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            conversation_id: json
                .get("conversation_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            raw_json,
            last_refreshed_at: unix_now(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub username: Option<String>,
    pub raw_json: String,
    pub last_refreshed_at: i64,
}

impl UserRow {
    /// Extract a UserRow from an X API user JSON object.
    pub fn from_api_json(json: &serde_json::Value) -> Option<Self> {
        let id = json.get("id")?.as_str()?.to_string();
        let raw_json = serde_json::to_string(json).ok()?;
        Some(Self {
            id,
            username: json
                .get("username")
                .and_then(|v| v.as_str())
                .map(String::from),
            raw_json,
            last_refreshed_at: unix_now(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BookmarkRow {
    pub username: String,
    pub tweet_id: String,
    pub position: i64,
    pub refreshed_at: i64,
}

#[derive(Debug, Clone)]
pub struct RawResponseRow {
    pub status_code: i64,
    pub body: Vec<u8>,
}

/// Entity store statistics for `bird cache stats` and `bird doctor`.
#[derive(Debug, serde::Serialize)]
pub struct StoreStats {
    pub tweet_count: u64,
    pub user_count: u64,
    pub bookmark_count: u64,
    pub raw_response_count: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
}

impl StoreStats {
    pub fn size_mb(&self) -> f64 {
        self.total_size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn max_size_mb(&self) -> f64 {
        self.max_size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn healthy(&self) -> bool {
        self.total_size_bytes < self.max_size_bytes
    }
}

// -- Schema migrations --

pub(crate) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        // Migration 1: core entity tables
        M::up(
            "CREATE TABLE tweets (
                id                TEXT PRIMARY KEY,
                author_id         TEXT,
                conversation_id   TEXT,
                raw_json          TEXT NOT NULL,
                last_refreshed_at INTEGER NOT NULL
            );
            CREATE INDEX idx_tweets_conversation_id ON tweets(conversation_id);
            CREATE INDEX idx_tweets_last_refreshed_at ON tweets(last_refreshed_at);

            CREATE TABLE users (
                id                TEXT PRIMARY KEY,
                username          TEXT,
                raw_json          TEXT NOT NULL,
                last_refreshed_at INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX idx_users_username ON users(username);

            CREATE TABLE bookmarks (
                account_username TEXT NOT NULL,
                tweet_id         TEXT NOT NULL,
                position         INTEGER NOT NULL,
                refreshed_at     INTEGER NOT NULL,
                PRIMARY KEY (account_username, tweet_id)
            ) WITHOUT ROWID;
            CREATE INDEX idx_bookmarks_tweet_id ON bookmarks(tweet_id);

            CREATE TABLE raw_responses (
                key         TEXT PRIMARY KEY,
                url         TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                body        BLOB NOT NULL,
                body_size   INTEGER NOT NULL,
                created_at  INTEGER NOT NULL
            );
            CREATE INDEX idx_raw_created_at ON raw_responses(created_at);",
        ),
        // Migration 2: usage tracking (same schema as cache.db for migration compatibility)
        M::up(
            "CREATE TABLE usage (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp      INTEGER NOT NULL,
                date_ymd       INTEGER NOT NULL,
                endpoint       TEXT NOT NULL,
                method         TEXT NOT NULL,
                object_type    TEXT,
                object_count   INTEGER NOT NULL DEFAULT 0,
                estimated_cost REAL NOT NULL DEFAULT 0.0,
                cache_hit      INTEGER NOT NULL DEFAULT 0,
                username       TEXT
            );
            CREATE INDEX idx_usage_ymd_endpoint_cache ON usage(date_ymd, endpoint, cache_hit);
            CREATE INDEX idx_usage_endpoint ON usage(endpoint);

            CREATE TABLE usage_actual (
                date         TEXT PRIMARY KEY,
                tweet_count  INTEGER NOT NULL,
                synced_at    INTEGER NOT NULL
            );

            CREATE TABLE migrations_meta (
                key   TEXT PRIMARY KEY,
                value TEXT
            );",
        ),
        // Migration 3: rename account_username → username in bookmarks (xurl alignment)
        M::up("ALTER TABLE bookmarks RENAME COLUMN account_username TO username;"),
    ])
}

// -- BirdDb --

/// Entity store: tweets, users, bookmarks, raw responses, and usage tracking.
/// Single connection per CLI invocation -- no pool needed (short-lived process).
pub struct BirdDb {
    pub(crate) conn: Connection,
    pub(crate) write_count: u32,
    pub(crate) max_bytes: u64,
}

impl BirdDb {
    /// Open (or create) the entity store at the given path.
    /// Sets process umask before opening to ensure SQLite sidecar files inherit restrictive permissions.
    pub fn open(path: &Path, max_size_mb: u64) -> Result<Self, rusqlite::Error> {
        Self::ensure_file_permissions(path);

        let mut conn = Connection::open(path)?;

        // Enforce 0o600 on existing DB file (not just creation)
        #[cfg(unix)]
        Self::enforce_permissions(path);

        let mmap_cap = std::cmp::min(max_size_mb * 1048576, 67108864); // cap at 64MB
        conn.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = {};",
            mmap_cap
        ))?;

        // Enforce 0o600 on WAL/SHM sidecar files after enabling WAL mode
        #[cfg(unix)]
        {
            let path_str = path.display().to_string();
            Self::enforce_permissions(Path::new(&format!("{}-wal", path_str)));
            Self::enforce_permissions(Path::new(&format!("{}-shm", path_str)));
        }

        // Reject tampered databases with triggers, views, or virtual tables
        let tamper_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type IN ('trigger', 'view')",
            [],
            |r| r.get(0),
        )?;
        if tamper_count > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some("database contains unexpected triggers or views".into()),
            ));
        }

        migrations().to_latest(&mut conn).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("migration failed: {}", e)),
            )
        })?;

        let db = Self {
            conn,
            write_count: 0,
            max_bytes: max_size_mb * 1024 * 1024,
        };

        Ok(db)
    }

    /// Pre-create file with 0o600 permissions so WAL/SHM sidecars inherit restrictive permissions.
    fn ensure_file_permissions(path: &Path) {
        if path.exists() {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)
                .and_then(|mut f| f.write_all(b""));
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path);
        }
    }

    /// Enforce 0o600 on an existing file. No-op on non-Unix or if file doesn't exist.
    #[cfg(unix)]
    fn enforce_permissions(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }

    /// Attempt to migrate usage data from the old cache.db on first open.
    /// Idempotent: checks a sentinel row in migrations_meta.
    pub fn migrate_usage_from_cache(&self, cache_db_path: &Path, quiet: bool) {
        if !cache_db_path.exists() {
            return;
        }

        // Check idempotency sentinel
        let already_migrated: bool = self
            .conn
            .query_row(
                "SELECT count(*) FROM migrations_meta WHERE key = 'cache_usage_migrated'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if already_migrated {
            return;
        }

        let cache_path_str = cache_db_path.display().to_string();

        // Validate source DB has expected tables before ATTACH
        let has_tables = (|| -> Result<bool, rusqlite::Error> {
            let probe = Connection::open_with_flags(
                cache_db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let count: i64 = probe.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('usage', 'usage_actual')",
                [],
                |r| r.get(0),
            )?;
            Ok(count == 2)
        })();

        match has_tables {
            Ok(true) => {}
            Ok(false) => {
                diag!(
                    quiet,
                    "[store] warning: cache.db missing expected tables, skipping usage migration"
                );
                return;
            }
            Err(e) => {
                diag!(
                    quiet,
                    "[store] warning: could not probe cache.db for migration: {}",
                    e
                );
                return;
            }
        }

        // ATTACH + copy in transaction
        let result = (|| -> Result<(), rusqlite::Error> {
            self.conn.execute_batch(&format!(
                "ATTACH DATABASE '{}' AS old_cache",
                cache_path_str.replace('\'', "''")
            ))?;

            let tx = self.conn.unchecked_transaction()?;
            tx.execute_batch(
                "INSERT OR IGNORE INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
                   SELECT timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username FROM old_cache.usage;
                 INSERT OR IGNORE INTO usage_actual SELECT * FROM old_cache.usage_actual;"
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO migrations_meta (key, value) VALUES ('cache_usage_migrated', datetime('now'))",
                [],
            )?;
            tx.commit()?;
            self.conn.execute_batch("DETACH DATABASE old_cache")?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                diag!(quiet, "[store] migrated usage data from cache.db");
            }
            Err(e) => {
                diag!(quiet, "[store] warning: usage migration failed: {}", e);
                let _ = self.conn.execute_batch("DETACH DATABASE old_cache");
            }
        }
    }

    // -- Tweet operations --

    #[cfg(test)]
    pub fn upsert_tweet(&self, tweet: &TweetRow) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO tweets (id, author_id, conversation_id, raw_json, last_refreshed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 author_id = excluded.author_id,
                 conversation_id = excluded.conversation_id,
                 raw_json = excluded.raw_json,
                 last_refreshed_at = excluded.last_refreshed_at",
        )?;
        stmt.execute(params![
            tweet.id,
            tweet.author_id,
            tweet.conversation_id,
            tweet.raw_json,
            tweet.last_refreshed_at,
        ])?;
        Ok(())
    }

    #[cfg(test)]
    pub fn upsert_user(&self, user: &UserRow) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO users (id, username, raw_json, last_refreshed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 username = excluded.username,
                 raw_json = excluded.raw_json,
                 last_refreshed_at = excluded.last_refreshed_at",
        )?;
        stmt.execute(params![
            user.id,
            user.username,
            user.raw_json,
            user.last_refreshed_at,
        ])?;
        Ok(())
    }

    /// Batch upsert entities in a single transaction for performance.
    /// Upserts users first (parents), then tweets (children) -- logical ordering, no FK enforcement.
    pub fn upsert_entities(
        &self,
        tweets: &[TweetRow],
        users: &[UserRow],
    ) -> Result<(), rusqlite::Error> {
        if tweets.is_empty() && users.is_empty() {
            return Ok(());
        }
        debug_assert!(
            self.conn.is_autocommit(),
            "upsert_entities called inside an existing transaction"
        );
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut user_stmt = tx.prepare_cached(
                "INSERT INTO users (id, username, raw_json, last_refreshed_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     username = excluded.username,
                     raw_json = excluded.raw_json,
                     last_refreshed_at = excluded.last_refreshed_at",
            )?;
            for user in users {
                user_stmt.execute(params![
                    user.id,
                    user.username,
                    user.raw_json,
                    user.last_refreshed_at,
                ])?;
            }

            let mut tweet_stmt = tx.prepare_cached(
                "INSERT INTO tweets (id, author_id, conversation_id, raw_json, last_refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                     author_id = excluded.author_id,
                     conversation_id = excluded.conversation_id,
                     raw_json = excluded.raw_json,
                     last_refreshed_at = excluded.last_refreshed_at",
            )?;
            for tweet in tweets {
                tweet_stmt.execute(params![
                    tweet.id,
                    tweet.author_id,
                    tweet.conversation_id,
                    tweet.raw_json,
                    tweet.last_refreshed_at,
                ])?;
            }
        }
        tx.commit()
    }

    pub fn get_tweet(&self, id: &str) -> Result<Option<TweetRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, author_id, conversation_id, raw_json, last_refreshed_at
             FROM tweets WHERE id = ?1",
        )?;
        let result = stmt.query_row(params![id], |row| {
            Ok(TweetRow {
                id: row.get(0)?,
                author_id: row.get(1)?,
                conversation_id: row.get(2)?,
                raw_json: row.get(3)?,
                last_refreshed_at: row.get(4)?,
            })
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, username, raw_json, last_refreshed_at
             FROM users WHERE username = ?1",
        )?;
        let result = stmt.query_row(params![username], |row| {
            Ok(UserRow {
                id: row.get(0)?,
                username: row.get(1)?,
                raw_json: row.get(2)?,
                last_refreshed_at: row.get(3)?,
            })
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Pure function: returns true if the entity should be re-fetched from the API.
    /// An entity is stale if its last refresh was on a prior UTC calendar day.
    pub fn is_stale(last_refreshed_at: i64, now: chrono::DateTime<chrono::Utc>) -> bool {
        let refreshed_date = chrono::DateTime::from_timestamp(last_refreshed_at, 0)
            .map(|dt| dt.date_naive())
            .unwrap_or(chrono::NaiveDate::MIN);
        refreshed_date < now.date_naive()
    }

    /// Partition a list of tweet IDs into (from_store, ids_to_fetch).
    /// `from_store` contains fresh TweetRows; `ids_to_fetch` contains stale or missing IDs.
    pub fn partition_ids(
        &self,
        ids: &[&str],
    ) -> Result<(Vec<TweetRow>, Vec<String>), rusqlite::Error> {
        if ids.is_empty() {
            return Ok((vec![], vec![]));
        }

        let placeholders: String = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, author_id, conversation_id, raw_json, last_refreshed_at
             FROM tweets WHERE id IN ({})",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(ids.iter());
        let rows: Vec<TweetRow> = stmt
            .query_map(params, |row| {
                Ok(TweetRow {
                    id: row.get(0)?,
                    author_id: row.get(1)?,
                    conversation_id: row.get(2)?,
                    raw_json: row.get(3)?,
                    last_refreshed_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let now = chrono::Utc::now();
        let from_store: Vec<TweetRow> = rows
            .into_iter()
            .filter(|row| !Self::is_stale(row.last_refreshed_at, now))
            .collect();

        let fresh_ids: std::collections::HashSet<&str> =
            from_store.iter().map(|r| r.id.as_str()).collect();

        let ids_to_fetch: Vec<String> = ids
            .iter()
            .filter(|id| !fresh_ids.contains(**id))
            .map(|id| id.to_string())
            .collect();

        Ok((from_store, ids_to_fetch))
    }

    // -- Raw response operations --

    pub fn upsert_raw_response(
        &self,
        key: &str,
        url: &str,
        status: u16,
        body: &[u8],
    ) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let body_size = body.len() as i64;
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO raw_responses (key, url, status_code, body, body_size, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
                 url = excluded.url,
                 status_code = excluded.status_code,
                 body = excluded.body,
                 body_size = excluded.body_size,
                 created_at = excluded.created_at",
        )?;
        stmt.execute(params![key, url, status as i64, body, body_size, now])?;
        Ok(())
    }

    pub fn get_raw_response(&self, key: &str) -> Result<Option<RawResponseRow>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT status_code, body FROM raw_responses WHERE key = ?1")?;
        let result = stmt.query_row(params![key], |row| {
            Ok(RawResponseRow {
                status_code: row.get(0)?,
                body: row.get(1)?,
            })
        });
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // -- Bookmark operations --

    /// Replace all bookmarks for a user. Wraps in transaction: DELETE all, INSERT new.
    pub fn replace_bookmarks(
        &self,
        username: &str,
        bookmarks: &[BookmarkRow],
    ) -> Result<(), rusqlite::Error> {
        debug_assert!(
            self.conn.is_autocommit(),
            "replace_bookmarks called inside an existing transaction"
        );
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM bookmarks WHERE username = ?1",
            params![username],
        )?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO bookmarks (username, tweet_id, position, refreshed_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for bm in bookmarks {
                stmt.execute(params![
                    bm.username,
                    bm.tweet_id,
                    bm.position,
                    bm.refreshed_at,
                ])?;
            }
        }
        tx.commit()
    }

    #[cfg(test)]
    pub fn get_bookmarks(&self, username: &str) -> Result<Vec<BookmarkRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT username, tweet_id, position, refreshed_at
             FROM bookmarks WHERE username = ?1
             ORDER BY position ASC",
        )?;
        let rows = stmt.query_map(params![username], |row| {
            Ok(BookmarkRow {
                username: row.get(0)?,
                tweet_id: row.get(1)?,
                position: row.get(2)?,
                refreshed_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -- Stats and maintenance --

    /// Entity store statistics: counts by type, total live size.
    /// Uses (page_count - freelist_count) * page_size for accurate live data size.
    pub fn stats(&self) -> Result<StoreStats, rusqlite::Error> {
        let tweet_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM tweets", [], |r| r.get(0))?;
        let user_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let bookmark_count: i64 =
            self.conn
                .query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0))?;
        let raw_response_count: i64 =
            self.conn
                .query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;

        let total_size = self.live_size_bytes()?;

        Ok(StoreStats {
            tweet_count: tweet_count as u64,
            user_count: user_count as u64,
            bookmark_count: bookmark_count as u64,
            raw_response_count: raw_response_count as u64,
            total_size_bytes: total_size,
            max_size_bytes: self.max_bytes,
        })
    }

    /// O(1) live data size: (page_count - freelist_count) * page_size.
    /// Excludes free pages from deleted rows to avoid re-triggering pruning after deletions.
    fn live_size_bytes(&self) -> Result<u64, rusqlite::Error> {
        let page_count: i64 = self.conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let freelist_count: i64 = self
            .conn
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let page_size: i64 = self.conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(((page_count - freelist_count) * page_size).max(0) as u64)
    }

    /// Clear all entity data + raw_responses (preserves usage tables).
    pub fn clear(&self) -> Result<u64, rusqlite::Error> {
        let tweet_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM tweets", [], |r| r.get(0))?;
        let user_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let raw_count: i64 =
            self.conn
                .query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;
        self.conn.execute_batch(
            "DELETE FROM tweets;
             DELETE FROM users;
             DELETE FROM bookmarks;
             DELETE FROM raw_responses;",
        )?;
        Ok((tweet_count + user_count + raw_count) as u64)
    }

    /// Prune old data when over size limit.
    /// Always prunes raw_responses older than 7 days.
    /// Prunes entity tables by last_refreshed_at when over size limit, targeting 80% of max.
    pub fn prune_if_needed(&self) -> Result<(), rusqlite::Error> {
        let now = unix_now();

        // Always prune raw_responses older than 7 days
        let seven_days_ago = now - 7 * 86400;
        self.conn.execute(
            "DELETE FROM raw_responses WHERE created_at < ?1",
            params![seven_days_ago],
        )?;

        // Check live size against limit
        let live_size = self.live_size_bytes()?;
        if live_size <= self.max_bytes {
            return Ok(());
        }

        // Prune to 80% of limit (hysteresis)
        let target_bytes = (self.max_bytes as f64 * 0.8) as i64;

        // Delete oldest tweets by last_refreshed_at
        loop {
            let current = self.live_size_bytes()? as i64;
            if current <= target_bytes {
                break;
            }
            let deleted = self.conn.execute(
                "DELETE FROM tweets WHERE id IN (
                    SELECT id FROM tweets ORDER BY last_refreshed_at ASC LIMIT 100
                )",
                [],
            )?;
            if deleted == 0 {
                break;
            }
        }

        // Delete oldest users by last_refreshed_at if still over
        loop {
            let current = self.live_size_bytes()? as i64;
            if current <= target_bytes {
                break;
            }
            let deleted = self.conn.execute(
                "DELETE FROM users WHERE id IN (
                    SELECT id FROM users ORDER BY last_refreshed_at ASC LIMIT 100
                )",
                [],
            )?;
            if deleted == 0 {
                break;
            }
        }

        Ok(())
    }

    /// Expose the DB file path for stats display.
    pub fn path(&self) -> Option<PathBuf> {
        self.conn.path().map(PathBuf::from)
    }
}

impl Drop for BirdDb {
    fn drop(&mut self) {
        // 0x10002: analyze all tables, even if not recently queried (optimal for short-lived CLI)
        let _ = self
            .conn
            .execute_batch("PRAGMA optimize(0x10002); PRAGMA wal_checkpoint(PASSIVE);");
    }
}

/// Create an in-memory BirdDb for testing.
#[cfg(test)]
pub(crate) fn in_memory_db() -> BirdDb {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )
    .unwrap();
    migrations().to_latest(&mut conn).unwrap();
    BirdDb {
        conn,
        write_count: 0,
        max_bytes: 100 * 1024 * 1024, // 100MB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().unwrap();
    }

    #[test]
    fn upsert_tweet_and_get() {
        let db = in_memory_db();
        let tweet = TweetRow {
            id: "123".into(),
            author_id: Some("456".into()),
            conversation_id: Some("789".into()),
            raw_json: r#"{"id":"123","text":"hello"}"#.into(),
            last_refreshed_at: unix_now(),
        };
        db.upsert_tweet(&tweet).unwrap();
        let got = db.get_tweet("123").unwrap().expect("should find tweet");
        assert_eq!(got.id, "123");
        assert_eq!(got.author_id.as_deref(), Some("456"));
        assert_eq!(got.raw_json, tweet.raw_json);
    }

    #[test]
    fn upsert_tweet_updates_on_conflict() {
        let db = in_memory_db();
        let tweet1 = TweetRow {
            id: "123".into(),
            author_id: Some("456".into()),
            conversation_id: None,
            raw_json: r#"{"id":"123","text":"v1"}"#.into(),
            last_refreshed_at: 1000,
        };
        db.upsert_tweet(&tweet1).unwrap();

        let tweet2 = TweetRow {
            id: "123".into(),
            author_id: Some("456".into()),
            conversation_id: None,
            raw_json: r#"{"id":"123","text":"v2"}"#.into(),
            last_refreshed_at: 2000,
        };
        db.upsert_tweet(&tweet2).unwrap();

        let got = db.get_tweet("123").unwrap().unwrap();
        assert!(got.raw_json.contains("v2"), "should have updated raw_json");
        assert_eq!(got.last_refreshed_at, 2000);
    }

    #[test]
    fn get_tweet_missing() {
        let db = in_memory_db();
        assert!(db.get_tweet("nonexistent").unwrap().is_none());
    }

    #[test]
    fn upsert_user_and_get_by_username() {
        let db = in_memory_db();
        let user = UserRow {
            id: "456".into(),
            username: Some("alice".into()),
            raw_json: r#"{"id":"456","username":"alice"}"#.into(),
            last_refreshed_at: unix_now(),
        };
        db.upsert_user(&user).unwrap();
        let got = db
            .get_user_by_username("alice")
            .unwrap()
            .expect("should find user");
        assert_eq!(got.id, "456");
        assert_eq!(got.username.as_deref(), Some("alice"));
    }

    #[test]
    fn upsert_entities_batch() {
        let db = in_memory_db();
        let users = vec![UserRow {
            id: "u1".into(),
            username: Some("bob".into()),
            raw_json: r#"{"id":"u1"}"#.into(),
            last_refreshed_at: unix_now(),
        }];
        let tweets = vec![
            TweetRow {
                id: "t1".into(),
                author_id: Some("u1".into()),
                conversation_id: None,
                raw_json: r#"{"id":"t1"}"#.into(),
                last_refreshed_at: unix_now(),
            },
            TweetRow {
                id: "t2".into(),
                author_id: Some("u1".into()),
                conversation_id: None,
                raw_json: r#"{"id":"t2"}"#.into(),
                last_refreshed_at: unix_now(),
            },
        ];
        db.upsert_entities(&tweets, &users).unwrap();
        assert!(db.get_tweet("t1").unwrap().is_some());
        assert!(db.get_tweet("t2").unwrap().is_some());
        assert!(db.get_user_by_username("bob").unwrap().is_some());
    }

    #[test]
    fn upsert_entities_with_missing_author() {
        // X API doesn't guarantee includes.users contains all referenced authors.
        // This must NOT fail (no SQL foreign keys).
        let db = in_memory_db();
        let tweets = vec![TweetRow {
            id: "t1".into(),
            author_id: Some("nonexistent_user".into()),
            conversation_id: None,
            raw_json: r#"{"id":"t1"}"#.into(),
            last_refreshed_at: unix_now(),
        }];
        // No users -- author_id references a user not in the DB
        db.upsert_entities(&tweets, &[]).unwrap();
        assert!(db.get_tweet("t1").unwrap().is_some());
    }

    #[test]
    fn is_stale_different_day() {
        use chrono::TimeZone;
        // Refreshed yesterday at 23:59 UTC
        let yesterday_2359 = chrono::Utc
            .with_ymd_and_hms(2026, 2, 17, 23, 59, 59)
            .unwrap();
        let now_today_0001 = chrono::Utc.with_ymd_and_hms(2026, 2, 18, 0, 0, 1).unwrap();
        assert!(BirdDb::is_stale(yesterday_2359.timestamp(), now_today_0001));
    }

    #[test]
    fn is_stale_same_day() {
        use chrono::TimeZone;
        let morning = chrono::Utc.with_ymd_and_hms(2026, 2, 18, 6, 0, 0).unwrap();
        let evening = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 23, 59, 59)
            .unwrap();
        assert!(!BirdDb::is_stale(morning.timestamp(), evening));
    }

    #[test]
    fn is_stale_midnight_boundary() {
        use chrono::TimeZone;
        // Refreshed at exactly midnight = start of new day = same day as anything later that day
        let midnight = chrono::Utc.with_ymd_and_hms(2026, 2, 18, 0, 0, 0).unwrap();
        let later = chrono::Utc.with_ymd_and_hms(2026, 2, 18, 12, 0, 0).unwrap();
        assert!(!BirdDb::is_stale(midnight.timestamp(), later));
    }

    #[test]
    fn is_stale_zero_timestamp() {
        // Epoch 0 (1970-01-01) should always be stale
        let now = chrono::Utc::now();
        assert!(BirdDb::is_stale(0, now));
    }

    #[test]
    fn partition_ids_mixed() {
        let db = in_memory_db();

        // Insert a fresh tweet (refreshed now)
        let fresh = TweetRow {
            id: "fresh1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"fresh1"}"#.into(),
            last_refreshed_at: unix_now(),
        };
        db.upsert_tweet(&fresh).unwrap();

        // Insert a stale tweet (refreshed long ago)
        let stale = TweetRow {
            id: "stale1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"stale1"}"#.into(),
            last_refreshed_at: 1000, // epoch 1970 -- definitely stale
        };
        db.upsert_tweet(&stale).unwrap();

        let ids = vec!["fresh1", "stale1", "missing1"];
        let (from_store, to_fetch) = db.partition_ids(&ids).unwrap();

        assert_eq!(from_store.len(), 1);
        assert_eq!(from_store[0].id, "fresh1");
        assert_eq!(to_fetch.len(), 2);
        assert!(to_fetch.contains(&"stale1".to_string()));
        assert!(to_fetch.contains(&"missing1".to_string()));
    }

    #[test]
    fn partition_ids_empty() {
        let db = in_memory_db();
        let (from_store, to_fetch) = db.partition_ids(&[]).unwrap();
        assert!(from_store.is_empty());
        assert!(to_fetch.is_empty());
    }

    #[test]
    fn raw_response_round_trip() {
        let db = in_memory_db();
        db.upsert_raw_response("key1", "https://api.x.com/test", 200, b"hello")
            .unwrap();
        let got = db
            .get_raw_response("key1")
            .unwrap()
            .expect("should find response");
        assert_eq!(got.status_code, 200);
        assert_eq!(got.body, b"hello");
    }

    #[test]
    fn replace_bookmarks_removes_old() {
        let db = in_memory_db();
        let now = unix_now();

        // Insert initial bookmarks
        let initial = vec![
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t1".into(),
                position: 0,
                refreshed_at: now,
            },
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t2".into(),
                position: 1,
                refreshed_at: now,
            },
        ];
        db.replace_bookmarks("alice", &initial).unwrap();
        assert_eq!(db.get_bookmarks("alice").unwrap().len(), 2);

        // Replace with different set
        let replacement = vec![BookmarkRow {
            username: "alice".into(),
            tweet_id: "t3".into(),
            position: 0,
            refreshed_at: now,
        }];
        db.replace_bookmarks("alice", &replacement).unwrap();

        let got = db.get_bookmarks("alice").unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].tweet_id, "t3");
    }

    #[test]
    fn bookmarks_ordered_by_position() {
        let db = in_memory_db();
        let now = unix_now();
        let bookmarks = vec![
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t3".into(),
                position: 2,
                refreshed_at: now,
            },
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t1".into(),
                position: 0,
                refreshed_at: now,
            },
            BookmarkRow {
                username: "alice".into(),
                tweet_id: "t2".into(),
                position: 1,
                refreshed_at: now,
            },
        ];
        db.replace_bookmarks("alice", &bookmarks).unwrap();
        let got = db.get_bookmarks("alice").unwrap();
        assert_eq!(got[0].tweet_id, "t1");
        assert_eq!(got[1].tweet_id, "t2");
        assert_eq!(got[2].tweet_id, "t3");
    }

    #[test]
    fn stats_reports_counts() {
        let db = in_memory_db();
        db.upsert_tweet(&TweetRow {
            id: "t1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: "{}".into(),
            last_refreshed_at: unix_now(),
        })
        .unwrap();
        db.upsert_user(&UserRow {
            id: "u1".into(),
            username: Some("alice".into()),
            raw_json: "{}".into(),
            last_refreshed_at: unix_now(),
        })
        .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.tweet_count, 1);
        assert_eq!(stats.user_count, 1);
        assert!(stats.healthy());
    }

    #[test]
    fn clear_preserves_usage() {
        let db = in_memory_db();
        db.upsert_tweet(&TweetRow {
            id: "t1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: "{}".into(),
            last_refreshed_at: unix_now(),
        })
        .unwrap();
        // Insert a usage row directly
        db.conn
            .execute(
                "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
                 VALUES (1000, 20260218, '/2/tweets', 'GET', 1, 0.005, 0)",
                [],
            )
            .unwrap();

        let count = db.clear().unwrap();
        assert_eq!(count, 1); // 1 tweet cleared

        // Usage should still be there
        let usage_count: i64 = db
            .conn
            .query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .unwrap();
        assert_eq!(usage_count, 1);
    }

    #[test]
    fn tweet_from_api_json() {
        let json = serde_json::json!({
            "id": "123",
            "text": "hello world",
            "author_id": "456",
            "conversation_id": "789",
            "created_at": "2026-02-18T12:00:00Z"
        });
        let tweet = TweetRow::from_api_json(&json).unwrap();
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.author_id.as_deref(), Some("456"));
        assert_eq!(tweet.conversation_id.as_deref(), Some("789"));
        // raw_json should contain all fields
        assert!(tweet.raw_json.contains("hello world"));
        assert!(tweet.raw_json.contains("created_at"));
    }

    #[test]
    fn user_from_api_json() {
        let json = serde_json::json!({
            "id": "456",
            "username": "alice",
            "name": "Alice",
            "created_at": "2020-01-01T00:00:00Z"
        });
        let user = UserRow::from_api_json(&json).unwrap();
        assert_eq!(user.id, "456");
        assert_eq!(user.username.as_deref(), Some("alice"));
        assert!(user.raw_json.contains("Alice"));
    }

    #[test]
    fn tweet_from_api_json_missing_id() {
        let json = serde_json::json!({"text": "no id"});
        assert!(TweetRow::from_api_json(&json).is_none());
    }

    #[test]
    fn anti_tamper_rejects_views() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;").unwrap();
        migrations().to_latest(&mut conn).unwrap();
        // Create a view (tamper)
        conn.execute_batch("CREATE VIEW evil AS SELECT * FROM tweets")
            .unwrap();

        // Open should fail with expanded anti-tamper check
        let tmpdir = tempfile::tempdir().unwrap();
        let db_path = tmpdir.path().join("test.db");
        // Write the tampered DB to disk
        {
            let mut disk_conn = Connection::open(&db_path).unwrap();
            disk_conn
                .execute_batch("PRAGMA journal_mode = WAL;")
                .unwrap();
            migrations().to_latest(&mut disk_conn).unwrap();
            disk_conn
                .execute_batch("CREATE VIEW evil AS SELECT * FROM tweets")
                .unwrap();
        }
        let result = BirdDb::open(&db_path, 100);
        assert!(result.is_err(), "should reject database with views");
    }

    #[test]
    fn usage_migration_idempotent() {
        let db = in_memory_db();
        // Write a sentinel to simulate already-migrated state
        db.conn
            .execute(
                "INSERT INTO migrations_meta (key, value) VALUES ('cache_usage_migrated', 'test')",
                [],
            )
            .unwrap();
        // Should be a no-op (doesn't crash)
        db.migrate_usage_from_cache(Path::new("/nonexistent/path"), false);
    }

    #[cfg(unix)]
    #[test]
    fn file_permissions_enforced() {
        use std::os::unix::fs::PermissionsExt;
        let tmpdir = tempfile::tempdir().unwrap();
        let db_path = tmpdir.path().join("test.db");

        let _db = BirdDb::open(&db_path, 100).unwrap();

        let perms = std::fs::metadata(&db_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "DB file should have 0o600 permissions"
        );
    }

    #[test]
    fn pruning_raw_responses_by_age() {
        let db = in_memory_db();
        let old = unix_now() - 8 * 86400; // 8 days ago
        db.conn
            .execute(
                "INSERT INTO raw_responses (key, url, status_code, body, body_size, created_at)
                 VALUES ('old', 'http://test', 200, X'00', 1, ?1)",
                params![old],
            )
            .unwrap();
        db.upsert_raw_response("fresh", "http://test", 200, b"data")
            .unwrap();

        db.prune_if_needed().unwrap();

        assert!(
            db.get_raw_response("old").unwrap().is_none(),
            "old response should be pruned"
        );
        assert!(
            db.get_raw_response("fresh").unwrap().is_some(),
            "fresh response should remain"
        );
    }
}
