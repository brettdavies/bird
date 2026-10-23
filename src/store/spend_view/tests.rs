use chrono::{DateTime, TimeZone, Utc};

use super::*;
use crate::store::ledger::tests::{actual, request};

fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, 12, 0, 0)
        .single()
        .expect("date")
}

#[test]
fn summary_matches_the_sql_semantics() {
    let records = [
        request(at(2026, 2, 11), "/2/tweets/search/recent", 0.015, false),
        request(at(2026, 2, 11), "/2/tweets/search/recent", 0.015, true),
    ];
    let s = summary(&records, 0);
    assert_eq!(s.total_calls, 2);
    assert_eq!(s.cache_hits, 1);
    assert!((s.total_cost - 0.015).abs() < f64::EPSILON);
    assert!((s.estimated_savings - 0.015).abs() < f64::EPSILON);
}

#[test]
fn days_group_by_utc_date_newest_first() {
    let records = [
        request(at(2026, 2, 10), "/2/tweets/search/recent", 0.005, false),
        request(at(2026, 2, 11), "/2/tweets/search/recent", 0.010, false),
        request(at(2026, 2, 11), "/2/users/me", 0.010, true),
        request(at(2026, 2, 9), "/2/users/me", 0.010, false),
    ];
    let days = daily(&records, 20260210);
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].date_ymd, 20260211);
    assert_eq!(days[0].calls, 2);
    assert_eq!(days[0].cache_hits, 1);
    assert!((days[0].cost - 0.010).abs() < f64::EPSILON);
    assert_eq!(days[1].date_ymd, 20260210);
    assert_eq!(days[1].calls, 1);
}

#[test]
fn top_endpoints_exclude_cache_hits() {
    let mut records: Vec<_> = (0..3)
        .map(|_| request(at(2026, 2, 11), "/2/tweets/search/recent", 0.005, false))
        .collect();
    records.push(request(at(2026, 2, 11), "/2/users/me", 0.010, false));
    records.push(request(at(2026, 2, 11), "/2/users/:id", 0.050, true));
    let top = top_endpoints(&records, 20260210);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].endpoint, "/2/tweets/search/recent");
    assert_eq!(top[0].calls, 3);
    assert!((top[0].cost - 0.015).abs() < f64::EPSILON);
    assert_eq!(top[1].endpoint, "/2/users/me");
}

#[test]
fn the_latest_actual_per_date_wins() {
    let records = [
        actual("2026-10-21", 5, at(2026, 10, 21)),
        actual("2026-10-20", 9, at(2026, 10, 21)),
        actual("2026-10-21", 7, at(2026, 10, 22)),
        actual("2026-09-01", 1, at(2026, 9, 2)),
    ];
    let days = actuals(&records, 20261001).expect("actuals");
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].date, "2026-10-21");
    assert_eq!(days[0].tweet_count, 7);
    assert_eq!(days[0].synced_at, Some(at(2026, 10, 22).timestamp()));
    assert_eq!(days[1].date, "2026-10-20");
}

#[test]
fn an_empty_ledger_reports_zeros() {
    let s = summary(&[], 0);
    assert_eq!(s.total_calls, 0);
    assert_eq!(s.total_cost, 0.0);
    assert_eq!(s.cache_hits, 0);
    assert_eq!(s.estimated_savings, 0.0);
    assert!(daily(&[], 0).is_empty());
    assert!(top_endpoints(&[], 0).is_empty());
    assert!(actuals(&[], 0).is_none());
}
