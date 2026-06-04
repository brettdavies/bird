//! User row and username lookup.

use rusqlite::params;

use super::BirdDb;

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: String,
    pub username: Option<String>,
    pub raw_json: String,
    pub last_refreshed_at: i64,
}

impl BirdDb {
    #[cfg(test)]
    pub fn upsert_user(&self, user: &UserRow) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserRow>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;
    use super::*;
    use crate::db::unix_now;

    #[test]
    fn upsert_user_and_get_by_username() {
        let db = in_memory_db();
        let user = UserRow {
            id: "456".into(),
            username: Some("alice".into()),
            raw_json: r#"{"id":"456","username":"alice"}"#.into(),
            last_refreshed_at: unix_now(),
        };
        db.upsert_user(&user).expect("test");
        let got = db
            .get_user_by_username("alice")
            .expect("test")
            .expect("should find user");
        assert_eq!(got.id, "456");
        assert_eq!(got.username.as_deref(), Some("alice"));
    }
}
