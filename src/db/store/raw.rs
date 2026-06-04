//! raw_responses CRUD (content-addressed cache for non-entity endpoints).

use rusqlite::params;

use super::BirdDb;
use crate::db::unix_now;

#[derive(Debug, Clone)]
pub struct RawResponseRow {
    pub status_code: i64,
    pub body: Vec<u8>,
}

impl BirdDb {
    pub fn upsert_raw_response(
        &self,
        key: &str,
        url: &str,
        status: u16,
        body: &[u8],
    ) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let body_size = body.len() as i64;
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
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
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT status_code, body FROM raw_responses WHERE key = ?1")?;
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
}

#[cfg(test)]
mod tests {
    use super::super::in_memory_db;

    #[test]
    fn raw_response_round_trip() {
        let db = in_memory_db();
        db.upsert_raw_response("key1", "https://api.x.com/test", 200, b"hello")
            .expect("test");
        let got = db
            .get_raw_response("key1")
            .expect("test")
            .expect("should find response");
        assert_eq!(got.status_code, 200);
        assert_eq!(got.body, b"hello");
    }
}
