//! Tweet row, single + batch lookup, freshness, and partition.

use rusqlite::params;

use super::BirdDb;

#[derive(Debug, Clone)]
pub struct TweetRow {
    pub id: String,
    pub author_id: Option<String>,
    pub conversation_id: Option<String>,
    pub raw_json: String,
    pub last_refreshed_at: i64,
}

impl BirdDb {
    #[cfg(test)]
    pub fn upsert_tweet(&self, tweet: &TweetRow) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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

    pub fn get_tweet(&self, id: &str) -> Result<Option<TweetRow>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
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
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;
    use super::*;
    use crate::db::unix_now;

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
        db.upsert_tweet(&tweet).expect("test");
        let got = db
            .get_tweet("123")
            .expect("test")
            .expect("should find tweet");
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
        db.upsert_tweet(&tweet1).expect("test");

        let tweet2 = TweetRow {
            id: "123".into(),
            author_id: Some("456".into()),
            conversation_id: None,
            raw_json: r#"{"id":"123","text":"v2"}"#.into(),
            last_refreshed_at: 2000,
        };
        db.upsert_tweet(&tweet2).expect("test");

        let got = db.get_tweet("123").expect("test").expect("test");
        assert!(got.raw_json.contains("v2"), "should have updated raw_json");
        assert_eq!(got.last_refreshed_at, 2000);
    }

    #[test]
    fn get_tweet_missing() {
        let db = in_memory_db();
        assert!(db.get_tweet("nonexistent").expect("test").is_none());
    }

    #[test]
    fn is_stale_different_day() {
        use chrono::TimeZone;
        let yesterday_2359 = chrono::Utc
            .with_ymd_and_hms(2026, 2, 17, 23, 59, 59)
            .single()
            .expect("test");
        let now_today_0001 = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 0, 0, 1)
            .single()
            .expect("test");
        assert!(BirdDb::is_stale(yesterday_2359.timestamp(), now_today_0001));
    }

    #[test]
    fn is_stale_same_day() {
        use chrono::TimeZone;
        let morning = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 6, 0, 0)
            .single()
            .expect("test");
        let evening = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 23, 59, 59)
            .single()
            .expect("test");
        assert!(!BirdDb::is_stale(morning.timestamp(), evening));
    }

    #[test]
    fn is_stale_midnight_boundary() {
        use chrono::TimeZone;
        let midnight = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 0, 0, 0)
            .single()
            .expect("test");
        let later = chrono::Utc
            .with_ymd_and_hms(2026, 2, 18, 12, 0, 0)
            .single()
            .expect("test");
        assert!(!BirdDb::is_stale(midnight.timestamp(), later));
    }

    #[test]
    fn is_stale_zero_timestamp() {
        let now = chrono::Utc::now();
        assert!(BirdDb::is_stale(0, now));
    }

    #[test]
    fn partition_ids_mixed() {
        let db = in_memory_db();

        let fresh = TweetRow {
            id: "fresh1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"fresh1"}"#.into(),
            last_refreshed_at: unix_now(),
        };
        db.upsert_tweet(&fresh).expect("test");

        let stale = TweetRow {
            id: "stale1".into(),
            author_id: None,
            conversation_id: None,
            raw_json: r#"{"id":"stale1"}"#.into(),
            last_refreshed_at: 1000,
        };
        db.upsert_tweet(&stale).expect("test");

        let ids = vec!["fresh1", "stale1", "missing1"];
        let (from_store, to_fetch) = db.partition_ids(&ids).expect("test");

        assert_eq!(from_store.len(), 1);
        assert_eq!(from_store[0].id, "fresh1");
        assert_eq!(to_fetch.len(), 2);
        assert!(to_fetch.contains(&"stale1".to_string()));
        assert!(to_fetch.contains(&"missing1".to_string()));
    }

    #[test]
    fn partition_ids_empty() {
        let db = in_memory_db();
        let (from_store, to_fetch) = db.partition_ids(&[]).expect("test");
        assert!(from_store.is_empty());
        assert!(to_fetch.is_empty());
    }
}
