//! BirdDb: SQLite database layer for cache storage.
//! Single connection per CLI invocation — no pool needed (short-lived process).
//! Blocking SQLite calls are fine: single-threaded tokio runtime.

use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use std::path::{Path, PathBuf};

use super::unix_now;

pub(crate) fn migrations() -> Migrations<'static> {
    // IMPORTANT: Never modify existing migrations. Only append new ones.
    Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS cache (
                key         TEXT PRIMARY KEY,
                url         TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                body        BLOB NOT NULL,
                body_size   INTEGER NOT NULL,
                created_at  INTEGER NOT NULL,
                ttl_seconds INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cache_created ON cache(created_at);",
        ),
        // Migration 2: usage tracking table (per-API-call cost logging)
        M::up(
            "CREATE TABLE IF NOT EXISTS usage (
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
            CREATE INDEX IF NOT EXISTS idx_usage_ymd_endpoint_cache ON usage(date_ymd, endpoint, cache_hit);
            CREATE INDEX IF NOT EXISTS idx_usage_endpoint ON usage(endpoint);",
        ),
        // Migration 3: actual usage from X API (for --sync comparison)
        M::up(
            "CREATE TABLE IF NOT EXISTS usage_actual (
                date         TEXT PRIMARY KEY,
                tweet_count  INTEGER NOT NULL,
                synced_at    INTEGER NOT NULL
            );",
        ),
    ])
}

/// Application database: cache storage + usage tracking.
/// Single connection per CLI invocation — no pool needed (short-lived process).
/// Blocking SQLite calls are fine: single-threaded tokio runtime.
pub struct BirdDb {
    pub(crate) conn: Connection,
    pub(crate) write_count: u32,
    pub(crate) max_bytes: u64,
}

impl BirdDb {
    /// Open (or create) the cache database at the given path.
    /// Pre-creates the file with 0o600 permissions, then opens with rusqlite.
    pub fn open(path: &Path, max_size_mb: u64) -> Result<Self, rusqlite::Error> {
        Self::ensure_file_permissions(path);

        let mut conn = Connection::open(path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA temp_store = MEMORY;",
        )?;

        // Reject tampered databases with triggers
        let trigger_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
            [],
            |r| r.get(0),
        )?;
        if trigger_count > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some("database contains unexpected triggers".into()),
            ));
        }

        migrations().to_latest(&mut conn).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("migration failed: {}", e)),
            )
        })?;

        Ok(Self {
            conn,
            write_count: 0,
            max_bytes: max_size_mb * 1024 * 1024,
        })
    }

    /// Pre-create file with 0o600 so WAL/SHM sidecars inherit restrictive permissions.
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

    /// Look up a cached response by key. Returns None if not found or expired.
    pub fn get(&self, key: &str) -> Result<Option<CacheEntry>, rusqlite::Error> {
        let now = unix_now();
        let mut stmt = self.conn.prepare_cached(
            "SELECT url, status_code, body, created_at, ttl_seconds
             FROM cache WHERE key = ?1 AND (created_at + ttl_seconds) > ?2",
        )?;
        let result = stmt.query_row(params![key, now], |row| {
            Ok(CacheEntry {
                url: row.get(0)?,
                status_code: row.get(1)?,
                body: row.get(2)?,
                created_at: row.get(3)?,
                ttl_seconds: row.get(4)?,
            })
        });
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Insert or replace a cache entry.
    pub fn put(
        &mut self,
        key: &str,
        url: &str,
        status_code: u16,
        body: &[u8],
        ttl_seconds: i64,
    ) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let body_size = body.len() as i64;
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO cache (key, url, status_code, body, body_size, created_at, ttl_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        stmt.execute(params![
            key,
            url,
            status_code,
            body,
            body_size,
            now,
            ttl_seconds
        ])?;

        self.write_count += 1;
        if self.write_count.is_multiple_of(20) {
            self.prune_if_needed()?;
        }
        Ok(())
    }

    /// Remove entries past their TTL.
    pub fn delete_expired(&self) -> Result<usize, rusqlite::Error> {
        let now = unix_now();
        self.conn.execute(
            "DELETE FROM cache WHERE (created_at + ttl_seconds) <= ?1",
            params![now],
        )
    }

    /// Delete oldest entries until total body_size is under the limit.
    fn prune_if_needed(&self) -> Result<(), rusqlite::Error> {
        // First expire anything past TTL
        self.delete_expired()?;

        let total: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(body_size), 0) FROM cache", [], |r| {
                    r.get(0)
                })?;

        if total as u64 <= self.max_bytes {
            return Ok(());
        }

        // Delete oldest entries until under 90% of limit
        let target = (self.max_bytes as f64 * 0.9) as i64;
        self.conn.execute(
            "DELETE FROM cache WHERE key IN (
                SELECT key FROM cache ORDER BY created_at ASC
                LIMIT (SELECT count(*) FROM cache) -
                    (SELECT count(*) FROM cache WHERE key IN (
                        SELECT key FROM (
                            SELECT key, SUM(body_size) OVER (ORDER BY created_at DESC) AS running
                            FROM cache
                        ) WHERE running <= ?1
                    ))
            )",
            params![target],
        )?;
        Ok(())
    }

    /// Cache statistics for `bird cache stats` and `bird doctor`.
    pub fn stats(&self) -> Result<CacheStats, rusqlite::Error> {
        let entry_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM cache", [], |r| r.get(0))?;
        let total_size: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(body_size), 0) FROM cache", [], |r| {
                    r.get(0)
                })?;
        let now = unix_now();
        let oldest: Option<i64> = self
            .conn
            .query_row("SELECT MIN(created_at) FROM cache", [], |r| r.get(0))
            .ok();
        let newest: Option<i64> = self
            .conn
            .query_row("SELECT MAX(created_at) FROM cache", [], |r| r.get(0))
            .ok();

        Ok(CacheStats {
            entry_count: entry_count as u64,
            total_size_bytes: total_size as u64,
            max_size_bytes: self.max_bytes,
            oldest_seconds_ago: oldest.map(|t| (now - t).max(0)),
            newest_seconds_ago: newest.map(|t| (now - t).max(0)),
        })
    }

    /// Delete all cache entries and reclaim space.
    pub fn clear(&self) -> Result<u64, rusqlite::Error> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM cache", [], |r| r.get(0))?;
        self.conn.execute("DELETE FROM cache", [])?;
        let _ = self.conn.execute_batch("PRAGMA incremental_vacuum;");
        Ok(count as u64)
    }

    /// Expose the DB file path for stats display.
    pub fn path(&self) -> Option<PathBuf> {
        self.conn.path().map(PathBuf::from)
    }
}

impl Drop for BirdDb {
    fn drop(&mut self) {
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    }
}

/// A cached response entry.
#[derive(Debug)]
pub struct CacheEntry {
    #[allow(dead_code)] // used for debugging/logging
    pub url: String,
    pub status_code: i64,
    pub body: Vec<u8>,
    #[allow(dead_code)] // available for future cache inspection
    pub created_at: i64,
    #[allow(dead_code)] // available for future cache inspection
    pub ttl_seconds: i64,
}

/// Cache statistics.
#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub entry_count: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
    pub oldest_seconds_ago: Option<i64>,
    pub newest_seconds_ago: Option<i64>,
}

impl CacheStats {
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

/// Create an in-memory BirdDb for testing (shared by db and usage tests).
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
    fn put_and_get() {
        let mut db = in_memory_db();
        db.put(
            "key1",
            "https://api.x.com/2/tweets/123",
            200,
            b"hello",
            3600,
        )
        .unwrap();
        let entry = db.get("key1").unwrap().expect("should find entry");
        assert_eq!(entry.status_code, 200);
        assert_eq!(entry.body, b"hello");
        assert_eq!(entry.ttl_seconds, 3600);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let db = in_memory_db();
        assert!(db.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn expired_entries_not_returned() {
        let mut db = in_memory_db();
        // Insert with TTL of 0 (already expired)
        db.put("expired", "https://api.x.com/test", 200, b"old", 0)
            .unwrap();
        assert!(db.get("expired").unwrap().is_none());
    }

    #[test]
    fn delete_expired() {
        let mut db = in_memory_db();
        db.put("expired", "https://api.x.com/test", 200, b"old", 0)
            .unwrap();
        db.put("fresh", "https://api.x.com/test2", 200, b"new", 3600)
            .unwrap();
        let deleted = db.delete_expired().unwrap();
        assert_eq!(deleted, 1);
        assert!(db.get("fresh").unwrap().is_some());
    }

    #[test]
    fn clear_removes_all() {
        let mut db = in_memory_db();
        db.put("a", "https://api.x.com/1", 200, b"data1", 3600)
            .unwrap();
        db.put("b", "https://api.x.com/2", 200, b"data2", 3600)
            .unwrap();
        let count = db.clear().unwrap();
        assert_eq!(count, 2);
        assert!(db.get("a").unwrap().is_none());
        assert!(db.get("b").unwrap().is_none());
    }

    #[test]
    fn stats_reports_correctly() {
        let mut db = in_memory_db();
        db.put("a", "https://api.x.com/1", 200, b"hello", 3600)
            .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.total_size_bytes, 5);
        assert!(stats.healthy());
    }

    #[test]
    fn pruning_respects_size_limit() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .unwrap();
        migrations().to_latest(&mut conn).unwrap();
        let mut db = BirdDb {
            conn,
            write_count: 0,
            max_bytes: 100, // 100 bytes limit
        };

        // Insert entries totaling > 100 bytes
        db.put("a", "https://api.x.com/1", 200, &[0u8; 40], 3600)
            .unwrap();
        db.put("b", "https://api.x.com/2", 200, &[0u8; 40], 3600)
            .unwrap();
        db.put("c", "https://api.x.com/3", 200, &[0u8; 40], 3600)
            .unwrap();

        // Manually trigger prune
        db.prune_if_needed().unwrap();

        let stats = db.stats().unwrap();
        assert!(
            stats.total_size_bytes <= 100,
            "should be under limit after prune"
        );
    }
}
