//! The 0.2.0 SQLite store and its migrations. Single connection per CLI
//! invocation; bird is synchronous, so blocking calls are fine.

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use std::path::Path;

pub mod maintenance;

pub use maintenance::StoreStats;

// -- Schema migrations --

fn migrations() -> Migrations<'static> {
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
        // Migration 4: X's post vocabulary. Renames the tweet-named table,
        // columns, and indexes, rewrites stored usage object types, and
        // drops every cached entity and response, which carry the legacy
        // vocabulary; usage history and bookmark positions are kept.
        M::up(
            "ALTER TABLE tweets RENAME TO posts;
            DROP INDEX idx_tweets_conversation_id;
            DROP INDEX idx_tweets_last_refreshed_at;
            CREATE INDEX idx_posts_conversation_id ON posts(conversation_id);
            CREATE INDEX idx_posts_last_refreshed_at ON posts(last_refreshed_at);

            ALTER TABLE bookmarks RENAME COLUMN tweet_id TO post_id;
            DROP INDEX idx_bookmarks_tweet_id;
            CREATE INDEX idx_bookmarks_post_id ON bookmarks(post_id);

            ALTER TABLE usage_actual RENAME COLUMN tweet_count TO post_count;

            UPDATE usage SET object_type = 'post' WHERE object_type = 'tweet';
            UPDATE usage SET object_type = 'posts' WHERE object_type = 'tweets';

            DELETE FROM posts;
            DELETE FROM users;
            DELETE FROM raw_responses;",
        ),
    ])
}

// -- BirdDb --

/// The 0.2.0 store, opened only so `bird cache` and `bird doctor` can report
/// and clear it.
///
/// `conn` is wrapped in `std::sync::Mutex` so `BirdDb` (and the enclosing
/// `BirdClient`) satisfy `Sync`.
pub struct BirdDb {
    pub(crate) conn: std::sync::Mutex<Connection>,
    pub(crate) max_bytes: u64,
}

impl BirdDb {
    /// Open (or create) the store at the given path, with private file modes
    /// on the database and its sidecars.
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
            conn: std::sync::Mutex::new(conn),
            max_bytes: max_size_mb * 1024 * 1024,
        };

        Ok(db)
    }

    /// Acquire the connection lock. Panics if the mutex was poisoned by a
    /// prior panic — bird is single-threaded today, so poisoning indicates a
    /// programming bug worth a hard fail.
    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("BirdDb conn mutex poisoned")
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
}

impl Drop for BirdDb {
    fn drop(&mut self) {
        // 0x10002: analyze all tables, even if not recently queried (optimal for short-lived CLI)
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute_batch("PRAGMA optimize(0x10002); PRAGMA wal_checkpoint(PASSIVE);");
        }
    }
}

/// Create an in-memory BirdDb for testing.
#[cfg(test)]
pub(crate) fn in_memory_db() -> BirdDb {
    let mut conn = Connection::open_in_memory().expect("test");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )
    .expect("test");
    migrations().to_latest(&mut conn).expect("test");
    BirdDb {
        conn: std::sync::Mutex::new(conn),
        max_bytes: 100 * 1024 * 1024, // 100MB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().expect("test");
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).expect("test")
    }

    /// A store bird 0.2.0 wrote: every migration through M3, a row in every
    /// table, in the legacy vocabulary.
    fn seed_v3_store(path: &Path) {
        let mut conn = Connection::open(path).expect("test");
        migrations().to_version(&mut conn, 3).expect("test");
        conn.execute_batch(
            r#"INSERT INTO tweets (id, author_id, conversation_id, raw_json, last_refreshed_at)
                 VALUES ('1', '9', '1', '{"id":"1","referenced_tweets":[{"type":"quoted","id":"2"}]}', 1);
               INSERT INTO users (id, username, raw_json, last_refreshed_at)
                 VALUES ('9', 'jack', '{"id":"9"}', 1);
               INSERT INTO bookmarks (username, tweet_id, position, refreshed_at)
                 VALUES ('jack', '1', 0, 1);
               INSERT INTO raw_responses (key, url, status_code, body, body_size, created_at)
                 VALUES ('k', 'https://api.x.com/2/tweets/1', 200, x'7b7d', 2, 1);
               INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit)
                 VALUES (1, 20260101, '/2/tweets/:id', 'GET', 'tweet', 1, 0.005, 0),
                        (2, 20260101, '/2/tweets/search/recent', 'GET', 'tweets', 2, 0.01, 0),
                        (3, 20260101, '/2/users/me', 'GET', 'user', 1, 0.01, 0);
               INSERT INTO usage_actual (date, tweet_count, synced_at) VALUES ('2026-01-01', 7, 1);"#,
        )
        .expect("test");
    }

    #[test]
    fn m4_moves_a_v3_store_to_the_post_vocabulary_and_purges_entities() {
        let tmpdir = tempfile::tempdir().expect("test");
        let path = tmpdir.path().join("bird.db");
        seed_v3_store(&path);

        let db = BirdDb::open(&path, 100).expect("test: open migrates");
        let conn = db.conn();
        assert_eq!(count(&conn, "PRAGMA user_version"), 4);
        assert_eq!(count(&conn, "SELECT count(*) FROM posts"), 0);
        assert_eq!(count(&conn, "SELECT count(*) FROM users"), 0);
        assert_eq!(count(&conn, "SELECT count(*) FROM raw_responses"), 0);
        assert_eq!(count(&conn, "SELECT count(*) FROM usage"), 3);
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM usage WHERE object_type = 'post'"
            ),
            1
        );
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM usage WHERE object_type = 'posts'"
            ),
            1
        );
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM usage WHERE object_type = 'user'"
            ),
            1
        );
        assert_eq!(
            count(&conn, "SELECT count(*) FROM bookmarks WHERE post_id = '1'"),
            1
        );
        assert_eq!(count(&conn, "SELECT post_count FROM usage_actual"), 7);
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM sqlite_master WHERE sql LIKE '%tweet%' OR name LIKE '%tweet%'"
            ),
            0,
            "no tweet-named table, column, or index survives"
        );
    }

    #[test]
    fn m4_runs_once() {
        let tmpdir = tempfile::tempdir().expect("test");
        let path = tmpdir.path().join("bird.db");
        seed_v3_store(&path);
        drop(BirdDb::open(&path, 100).expect("test"));
        {
            let conn = Connection::open(&path).expect("test");
            conn.execute_batch(
                "INSERT INTO posts (id, raw_json, last_refreshed_at) VALUES ('5', '{}', 1);",
            )
            .expect("test");
        }
        let db = BirdDb::open(&path, 100).expect("test: reopen");
        assert_eq!(count(&db.conn(), "SELECT count(*) FROM posts"), 1);
    }

    #[test]
    fn a_fresh_store_has_no_tweet_identifiers() {
        let db = in_memory_db();
        let conn = db.conn();
        assert_eq!(count(&conn, "PRAGMA user_version"), 4);
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM sqlite_master WHERE sql LIKE '%tweet%' OR name LIKE '%tweet%'"
            ),
            0
        );
    }

    #[test]
    fn anti_tamper_rejects_views() {
        let mut conn = Connection::open_in_memory().expect("test");
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .expect("test");
        migrations().to_latest(&mut conn).expect("test");
        conn.execute_batch("CREATE VIEW evil AS SELECT * FROM posts")
            .expect("test");

        let tmpdir = tempfile::tempdir().expect("test");
        let db_path = tmpdir.path().join("test.db");
        {
            let mut disk_conn = Connection::open(&db_path).expect("test");
            disk_conn
                .execute_batch("PRAGMA journal_mode = WAL;")
                .expect("test");
            migrations().to_latest(&mut disk_conn).expect("test");
            disk_conn
                .execute_batch("CREATE VIEW evil AS SELECT * FROM posts")
                .expect("test");
        }
        let result = BirdDb::open(&db_path, 100);
        assert!(result.is_err(), "should reject database with views");
    }

    #[cfg(unix)]
    #[test]
    fn file_permissions_enforced() {
        use std::os::unix::fs::PermissionsExt;
        let tmpdir = tempfile::tempdir().expect("test");
        let db_path = tmpdir.path().join("test.db");

        let _db = BirdDb::open(&db_path, 100).expect("test");

        let perms = std::fs::metadata(&db_path).expect("test").permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "DB file should have 0o600 permissions"
        );
    }
}
