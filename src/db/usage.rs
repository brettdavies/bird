//! Usage tracking: per-API-call cost logging and query methods on BirdDb.

use rusqlite::params;

use super::db::BirdDb;
use super::unix_now;

// -- Usage data structures --

/// Entry for logging an API call to the usage table.
pub struct UsageLogEntry<'a> {
    pub endpoint: &'a str,
    pub method: &'a str,
    pub object_type: &'a str,
    pub object_count: i64,
    pub estimated_cost: f64,
    pub cache_hit: bool,
    pub username: Option<&'a str>,
}

#[derive(Debug, serde::Serialize)]
pub struct UsageSummary {
    pub total_cost: f64,
    pub total_calls: i64,
    pub cache_hits: i64,
    pub estimated_savings: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct DailyUsage {
    pub date_ymd: i64,
    pub cost: f64,
    pub calls: i64,
    pub cache_hits: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct EndpointUsage {
    pub endpoint: String,
    pub cost: f64,
    pub calls: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct ActualUsageDay {
    pub date: String,
    pub tweet_count: u64,
    pub synced_at: Option<i64>,
}

// -- BirdDb usage methods --

impl BirdDb {
    /// Log an API call to the usage table for cost tracking.
    pub fn log_usage(&mut self, entry: &UsageLogEntry<'_>) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let date_ymd = {
            let dt = chrono::DateTime::from_timestamp(now, 0).unwrap();
            let d = dt.date_naive();
            use chrono::Datelike;
            d.year() as i64 * 10000 + d.month() as i64 * 100 + d.day() as i64
        };
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        stmt.execute(params![
            now,
            date_ymd,
            entry.endpoint,
            entry.method,
            entry.object_type,
            entry.object_count,
            entry.estimated_cost,
            entry.cache_hit as i32,
            entry.username
        ])?;

        self.write_count += 1;
        if self.write_count.is_multiple_of(50) {
            self.prune_old_usage(now)?;
        }
        Ok(())
    }

    /// Delete usage rows older than 90 days.
    fn prune_old_usage(&self, now_ts: i64) -> Result<(), rusqlite::Error> {
        let cutoff = now_ts - (90 * 24 * 60 * 60);
        self.conn
            .execute("DELETE FROM usage WHERE timestamp < ?1", [cutoff])?;
        Ok(())
    }

    /// Query usage summary (totals) since a given YYYYMMDD date.
    pub fn query_usage_summary(&self, since_ymd: i64) -> Result<UsageSummary, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT
                COALESCE(SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END), 0.0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ELSE 0 END), 0.0)
             FROM usage WHERE date_ymd >= ?1",
        )?;
        stmt.query_row([since_ymd], |row| {
            Ok(UsageSummary {
                total_cost: row.get(0)?,
                total_calls: row.get(1)?,
                cache_hits: row.get(2)?,
                estimated_savings: row.get(3)?,
            })
        })
    }

    /// Query daily usage breakdown since a given YYYYMMDD date.
    pub fn query_daily_usage(&self, since_ymd: i64) -> Result<Vec<DailyUsage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT
                date_ymd,
                SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END),
                COUNT(*),
                SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END)
             FROM usage
             WHERE date_ymd >= ?1
             GROUP BY date_ymd
             ORDER BY date_ymd DESC",
        )?;
        let rows = stmt.query_map([since_ymd], |row| {
            Ok(DailyUsage {
                date_ymd: row.get(0)?,
                cost: row.get(1)?,
                calls: row.get(2)?,
                cache_hits: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Query top endpoints by cost since a given YYYYMMDD date.
    pub fn query_top_endpoints(
        &self,
        since_ymd: i64,
    ) -> Result<Vec<EndpointUsage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT endpoint, SUM(estimated_cost), COUNT(*)
             FROM usage
             WHERE date_ymd >= ?1 AND cache_hit = 0
             GROUP BY endpoint
             ORDER BY SUM(estimated_cost) DESC
             LIMIT 10",
        )?;
        let rows = stmt.query_map([since_ymd], |row| {
            Ok(EndpointUsage {
                endpoint: row.get(0)?,
                cost: row.get(1)?,
                calls: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Upsert actual usage from X API sync.
    pub fn upsert_actual_usage(&self, date: &str, tweet_count: u64) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO usage_actual (date, tweet_count, synced_at)
             VALUES (?1, ?2, ?3)",
        )?;
        stmt.execute(params![date, tweet_count as i64, now])?;
        Ok(())
    }

    /// Query actual usage data (from previous API syncs).
    pub fn query_actual_usage(
        &self,
        since_ymd: i64,
    ) -> Result<Option<Vec<ActualUsageDay>>, rusqlite::Error> {
        let since_date = format!(
            "{}-{:02}-{:02}",
            since_ymd / 10000,
            (since_ymd % 10000) / 100,
            since_ymd % 100
        );
        let mut stmt = self.conn.prepare_cached(
            "SELECT date, tweet_count, synced_at FROM usage_actual
             WHERE date >= ?1
             ORDER BY date DESC",
        )?;
        let rows: Vec<ActualUsageDay> = stmt
            .query_map([&since_date], |row| {
                Ok(ActualUsageDay {
                    date: row.get(0)?,
                    tweet_count: row.get::<_, i64>(1)? as u64,
                    synced_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(rows))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::db::in_memory_db;
    use super::*;

    #[test]
    fn log_usage_and_query_summary() {
        let mut db = in_memory_db();
        db.log_usage(&UsageLogEntry {
            endpoint: "/2/tweets/search/recent",
            method: "GET",
            object_type: "tweet",
            object_count: 3,
            estimated_cost: 0.015,
            cache_hit: false,
            username: Some("alice"),
        })
        .unwrap();
        db.log_usage(&UsageLogEntry {
            endpoint: "/2/tweets/search/recent",
            method: "GET",
            object_type: "tweet",
            object_count: 3,
            estimated_cost: 0.015,
            cache_hit: true,
            username: Some("alice"),
        })
        .unwrap();

        let summary = db.query_usage_summary(0).unwrap();
        assert_eq!(summary.total_calls, 2);
        assert_eq!(summary.cache_hits, 1);
        assert!((summary.total_cost - 0.015).abs() < f64::EPSILON);
        assert!((summary.estimated_savings - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn query_daily_usage_groups_by_day() {
        let db = in_memory_db();
        db.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
             VALUES (1000, 20260210, '/2/tweets/search/recent', 'GET', 1, 0.005, 0)",
            [],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
             VALUES (2000, 20260211, '/2/tweets/search/recent', 'GET', 2, 0.010, 0)",
            [],
        ).unwrap();
        db.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
             VALUES (3000, 20260211, '/2/users/me', 'GET', 1, 0.010, 1)",
            [],
        ).unwrap();

        let daily = db.query_daily_usage(20260210).unwrap();
        assert_eq!(daily.len(), 2);
        assert_eq!(daily[0].date_ymd, 20260211);
        assert_eq!(daily[0].calls, 2);
        assert_eq!(daily[0].cache_hits, 1);
        assert_eq!(daily[1].date_ymd, 20260210);
        assert_eq!(daily[1].calls, 1);
    }

    #[test]
    fn query_top_endpoints_aggregates() {
        let db = in_memory_db();
        for _ in 0..3 {
            db.conn.execute(
                "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
                 VALUES (1000, 20260211, '/2/tweets/search/recent', 'GET', 1, 0.005, 0)",
                [],
            ).unwrap();
        }
        db.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
             VALUES (2000, 20260211, '/2/users/me', 'GET', 1, 0.010, 0)",
            [],
        ).unwrap();

        let top = db.query_top_endpoints(20260210).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].endpoint, "/2/tweets/search/recent");
        assert_eq!(top[0].calls, 3);
        assert!((top[0].cost - 0.015).abs() < f64::EPSILON);
        assert_eq!(top[1].endpoint, "/2/users/me");
    }

    #[test]
    fn empty_usage_returns_zero_summary() {
        let db = in_memory_db();
        let summary = db.query_usage_summary(0).unwrap();
        assert_eq!(summary.total_calls, 0);
        assert_eq!(summary.total_cost, 0.0);
        assert_eq!(summary.cache_hits, 0);
        assert_eq!(summary.estimated_savings, 0.0);
    }

    #[test]
    fn usage_pruning_via_write_count() {
        let mut db = in_memory_db();
        db.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_count, estimated_cost, cache_hit)
             VALUES (1, 20200101, '/2/tweets/search/recent', 'GET', 1, 0.005, 0)",
            [],
        ).unwrap();

        db.write_count = 49;
        db.log_usage(&UsageLogEntry {
            endpoint: "/2/tweets/search/recent",
            method: "GET",
            object_type: "tweet",
            object_count: 1,
            estimated_cost: 0.005,
            cache_hit: false,
            username: None,
        })
        .unwrap();

        let summary = db.query_usage_summary(0).unwrap();
        assert_eq!(
            summary.total_calls, 1,
            "old entry should be pruned, leaving only the fresh one"
        );
    }

    #[test]
    fn actual_usage_round_trip() {
        let db = in_memory_db();
        db.upsert_actual_usage("2026-02-18", 42).unwrap();
        let actuals = db.query_actual_usage(20260201).unwrap().unwrap();
        assert_eq!(actuals.len(), 1);
        assert_eq!(actuals[0].date, "2026-02-18");
        assert_eq!(actuals[0].tweet_count, 42);
        assert!(actuals[0].synced_at.is_some());
    }
}
