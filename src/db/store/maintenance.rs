//! Aggregate stats, clear, prune, file path, and live-size accounting.

use rusqlite::params;
use std::path::PathBuf;

use super::BirdDb;
use crate::db::unix_now;

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

impl BirdDb {
    /// Entity store statistics: counts by type, total live size.
    /// Uses (page_count - freelist_count) * page_size for accurate live data size.
    pub fn stats(&self) -> Result<StoreStats, rusqlite::Error> {
        let conn = self.conn();
        let tweet_count: i64 = conn.query_row("SELECT count(*) FROM tweets", [], |r| r.get(0))?;
        let user_count: i64 = conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let bookmark_count: i64 =
            conn.query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0))?;
        let raw_response_count: i64 =
            conn.query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;
        drop(conn);

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
        let conn = self.conn();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        Ok(((page_count - freelist_count) * page_size).max(0) as u64)
    }

    /// Clear all entity data + raw_responses (preserves usage tables).
    pub fn clear(&self) -> Result<u64, rusqlite::Error> {
        let conn = self.conn();
        let tweet_count: i64 = conn.query_row("SELECT count(*) FROM tweets", [], |r| r.get(0))?;
        let user_count: i64 = conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        let raw_count: i64 =
            conn.query_row("SELECT count(*) FROM raw_responses", [], |r| r.get(0))?;
        conn.execute_batch(
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

        let seven_days_ago = now - 7 * 86400;
        self.conn().execute(
            "DELETE FROM raw_responses WHERE created_at < ?1",
            params![seven_days_ago],
        )?;

        let live_size = self.live_size_bytes()?;
        if live_size <= self.max_bytes {
            return Ok(());
        }

        let target_bytes = (self.max_bytes as f64 * 0.8) as i64;

        loop {
            let current = self.live_size_bytes()? as i64;
            if current <= target_bytes {
                break;
            }
            let deleted = self.conn().execute(
                "DELETE FROM tweets WHERE id IN (
                    SELECT id FROM tweets ORDER BY last_refreshed_at ASC LIMIT 100
                )",
                [],
            )?;
            if deleted == 0 {
                break;
            }
        }

        loop {
            let current = self.live_size_bytes()? as i64;
            if current <= target_bytes {
                break;
            }
            let deleted = self.conn().execute(
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
        self.conn().path().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;
    use super::super::tweets::TweetRow;
    use super::super::users::UserRow;
    use crate::db::unix_now;

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
        .expect("test");
        db.upsert_user(&UserRow {
            id: "u1".into(),
            username: Some("alice".into()),
            raw_json: "{}".into(),
            last_refreshed_at: unix_now(),
        })
        .expect("test");
        let stats = db.stats().expect("test");
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
        .expect("test");
        db.conn()
            .execute(
                "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
                 VALUES (1000, 20260218, '/2/tweets', 'GET', 1, 0.005, 0)",
                [],
            )
            .expect("test");

        let count = db.clear().expect("test");
        assert_eq!(count, 1);

        let usage_count: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .expect("test");
        assert_eq!(usage_count, 1);
    }

    #[test]
    fn pruning_raw_responses_by_age() {
        let db = in_memory_db();
        let old = unix_now() - 8 * 86400;
        db.conn()
            .execute(
                "INSERT INTO raw_responses (key, url, status_code, body, body_size, created_at)
                 VALUES ('old', 'http://test', 200, X'00', 1, ?1)",
                rusqlite::params![old],
            )
            .expect("test");
        db.upsert_raw_response("fresh", "http://test", 200, b"data")
            .expect("test");

        db.prune_if_needed().expect("test");

        assert!(
            db.get_raw_response("old").expect("test").is_none(),
            "old response should be pruned"
        );
        assert!(
            db.get_raw_response("fresh").expect("test").is_some(),
            "fresh response should remain"
        );
    }
}
