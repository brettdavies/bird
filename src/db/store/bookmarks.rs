//! Bookmark row and per-user replace/read.

use rusqlite::params;

use super::BirdDb;

#[derive(Debug, Clone)]
pub struct BookmarkRow {
    pub username: String,
    pub tweet_id: String,
    pub position: i64,
    pub refreshed_at: i64,
}

impl BirdDb {
    /// Replace all bookmarks for a user. Wraps in transaction: DELETE all, INSERT new.
    pub fn replace_bookmarks(
        &self,
        username: &str,
        bookmarks: &[BookmarkRow],
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        debug_assert!(
            conn.is_autocommit(),
            "replace_bookmarks called inside an existing transaction"
        );
        let tx = conn.unchecked_transaction()?;
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
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;
    use super::*;
    use crate::db::unix_now;

    #[test]
    fn replace_bookmarks_removes_old() {
        let db = in_memory_db();
        let now = unix_now();

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
        db.replace_bookmarks("alice", &initial).expect("test");
        assert_eq!(db.get_bookmarks("alice").expect("test").len(), 2);

        let replacement = vec![BookmarkRow {
            username: "alice".into(),
            tweet_id: "t3".into(),
            position: 0,
            refreshed_at: now,
        }];
        db.replace_bookmarks("alice", &replacement).expect("test");

        let got = db.get_bookmarks("alice").expect("test");
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
        db.replace_bookmarks("alice", &bookmarks).expect("test");
        let got = db.get_bookmarks("alice").expect("test");
        assert_eq!(got[0].tweet_id, "t1");
        assert_eq!(got[1].tweet_id, "t2");
        assert_eq!(got[2].tweet_id, "t3");
    }
}
