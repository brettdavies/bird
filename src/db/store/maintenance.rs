//! Aggregate stats, clear, file path, and live-size accounting.

use std::path::PathBuf;

use super::BirdDb;

/// Entity store statistics for `bird cache stats` and `bird doctor`.
#[derive(Debug, serde::Serialize)]
pub struct StoreStats {
    pub post_count: u64,
    pub user_count: u64,
    pub bookmark_count: u64,
    pub raw_response_count: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
    /// The store's schema version: the last migration applied.
    pub schema_version: i64,
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

impl BirdDb {
    /// Entity store statistics: counts by type, total live size.
    /// Uses (page_count - freelist_count) * page_size for accurate live data size.
    pub fn stats(&self) -> Result<StoreStats, rusqlite::Error> {
        let conn = self.conn();
        let post_count: i64 = conn.query_row("SELECT count(*) FROM posts", [], |r| r.get(0))?;
        let user_count: i64 = conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let bookmark_count: i64 =
            conn.query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0))?;
        let raw_response_count: i64 =
            conn.query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;
        let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        drop(conn);

        let total_size = self.live_size_bytes()?;

        Ok(StoreStats {
            post_count: post_count as u64,
            user_count: user_count as u64,
            bookmark_count: bookmark_count as u64,
            raw_response_count: raw_response_count as u64,
            total_size_bytes: total_size,
            max_size_bytes: self.max_bytes,
            schema_version,
        })
    }

    /// O(1) live data size: (page_count - freelist_count) * page_size.
    fn live_size_bytes(&self) -> Result<u64, rusqlite::Error> {
        let conn = self.conn();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(((page_count - freelist_count) * page_size).max(0) as u64)
    }

    /// Clear all entity data + raw_responses (preserves usage tables).
    pub fn clear(&self) -> Result<u64, rusqlite::Error> {
        let conn = self.conn();
        let post_count: i64 = conn.query_row("SELECT count(*) FROM posts", [], |r| r.get(0))?;
        let user_count: i64 = conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let raw_count: i64 =
            conn.query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;
        conn.execute_batch(
            "DELETE FROM posts;
             DELETE FROM users;
             DELETE FROM bookmarks;
             DELETE FROM raw_responses;",
        )?;
        Ok((post_count + user_count + raw_count) as u64)
    }

    /// Expose the DB file path for stats display.
    pub fn path(&self) -> Option<PathBuf> {
        self.conn().path().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;

    fn seed_post_and_user(db: &super::BirdDb) {
        db.conn()
            .execute_batch(
                "INSERT INTO posts (id, raw_json, last_refreshed_at) VALUES ('t1', '{}', 1);
                 INSERT INTO users (id, username, raw_json, last_refreshed_at)
                 VALUES ('u1', 'alice', '{}', 1);",
            )
            .expect("test");
    }

    #[test]
    fn stats_reports_counts() {
        let db = in_memory_db();
        seed_post_and_user(&db);
        let stats = db.stats().expect("test");
        assert_eq!(stats.post_count, 1);
        assert_eq!(stats.user_count, 1);
        assert!(stats.healthy());
    }

    #[test]
    fn clear_preserves_usage() {
        let db = in_memory_db();
        seed_post_and_user(&db);
        db.conn()
            .execute(
                "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
                 VALUES (1000, 20260218, '/2/tweets', 'GET', 1, 0.005, 0)",
                [],
            )
            .expect("test");

        let count = db.clear().expect("test");
        assert_eq!(count, 2);

        let usage_count: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .expect("test");
        assert_eq!(usage_count, 1);
    }
}
