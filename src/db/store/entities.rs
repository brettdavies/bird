//! Joint tweet + user batch upsert and shared `from_api_json` decoders.

use rusqlite::params;

use super::BirdDb;
use super::tweets::TweetRow;
use super::users::UserRow;
use crate::db::unix_now;

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

impl BirdDb {
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
        let conn = self.conn();
        debug_assert!(
            conn.is_autocommit(),
            "upsert_entities called inside an existing transaction"
        );
        let tx = conn.unchecked_transaction()?;
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
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;
    use super::*;
    use crate::db::unix_now;

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
        db.upsert_entities(&tweets, &users).expect("test");
        assert!(db.get_tweet("t1").expect("test").is_some());
        assert!(db.get_tweet("t2").expect("test").is_some());
        assert!(db.get_user_by_username("bob").expect("test").is_some());
    }

    #[test]
    fn upsert_entities_with_missing_author() {
        // X API doesn't guarantee includes.users contains all referenced authors.
        let db = in_memory_db();
        let tweets = vec![TweetRow {
            id: "t1".into(),
            author_id: Some("nonexistent_user".into()),
            conversation_id: None,
            raw_json: r#"{"id":"t1"}"#.into(),
            last_refreshed_at: unix_now(),
        }];
        db.upsert_entities(&tweets, &[]).expect("test");
        assert!(db.get_tweet("t1").expect("test").is_some());
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
        let tweet = TweetRow::from_api_json(&json).expect("test");
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.author_id.as_deref(), Some("456"));
        assert_eq!(tweet.conversation_id.as_deref(), Some("789"));
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
        let user = UserRow::from_api_json(&json).expect("test");
        assert_eq!(user.id, "456");
        assert_eq!(user.username.as_deref(), Some("alice"));
        assert!(user.raw_json.contains("Alice"));
    }

    #[test]
    fn tweet_from_api_json_missing_id() {
        let json = serde_json::json!({"text": "no id"});
        assert!(TweetRow::from_api_json(&json).is_none());
    }
}
