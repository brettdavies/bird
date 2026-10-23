//! The aggregates `bird spend` prints, computed from ledger records.
//!
//! Cost excludes answered requests, calls count them, savings sum the
//! answered requests' cost, and the top endpoints leave them out.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Utc};

use super::ledger::{ActualRecord, Record, RequestRecord};

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

/// How many endpoints the top list holds.
const TOP_ENDPOINTS: usize = 10;

/// A request record's UTC date as `YYYYMMDD`.
fn date_ymd(record: &RequestRecord) -> Option<i64> {
    let ts = DateTime::parse_from_rfc3339(&record.ts)
        .ok()?
        .with_timezone(&Utc);
    let d = ts.date_naive();
    Some(i64::from(d.year()) * 10000 + i64::from(d.month()) * 100 + i64::from(d.day()))
}

/// The request records dated on or after `since_ymd`, with their dates.
fn requests_since(
    records: &[Record],
    since_ymd: i64,
) -> impl Iterator<Item = (i64, &RequestRecord)> {
    records.iter().filter_map(move |record| match record {
        Record::Request(r) => date_ymd(r).filter(|d| *d >= since_ymd).map(|d| (d, r)),
        Record::Actual(_) => None,
    })
}

/// Totals since `since_ymd`.
pub fn summary(records: &[Record], since_ymd: i64) -> UsageSummary {
    let mut s = UsageSummary {
        total_cost: 0.0,
        total_calls: 0,
        cache_hits: 0,
        estimated_savings: 0.0,
    };
    for (_, r) in requests_since(records, since_ymd) {
        s.total_calls += 1;
        if r.cache_hit {
            s.cache_hits += 1;
            s.estimated_savings += r.estimated_cost;
        } else {
            s.total_cost += r.estimated_cost;
        }
    }
    s
}

/// One row per UTC date since `since_ymd`, newest first.
pub fn daily(records: &[Record], since_ymd: i64) -> Vec<DailyUsage> {
    let mut days: BTreeMap<i64, DailyUsage> = BTreeMap::new();
    for (date, r) in requests_since(records, since_ymd) {
        let day = days.entry(date).or_insert(DailyUsage {
            date_ymd: date,
            cost: 0.0,
            calls: 0,
            cache_hits: 0,
        });
        day.calls += 1;
        if r.cache_hit {
            day.cache_hits += 1;
        } else {
            day.cost += r.estimated_cost;
        }
    }
    days.into_values().rev().collect()
}

/// The ten costliest endpoints since `since_ymd`, sent requests only.
pub fn top_endpoints(records: &[Record], since_ymd: i64) -> Vec<EndpointUsage> {
    let mut by_endpoint: BTreeMap<&str, EndpointUsage> = BTreeMap::new();
    for (_, r) in requests_since(records, since_ymd).filter(|(_, r)| !r.cache_hit) {
        let row = by_endpoint
            .entry(r.endpoint.as_str())
            .or_insert_with(|| EndpointUsage {
                endpoint: r.endpoint.clone(),
                cost: 0.0,
                calls: 0,
            });
        row.cost += r.estimated_cost;
        row.calls += 1;
    }
    let mut rows: Vec<EndpointUsage> = by_endpoint.into_values().collect();
    rows.sort_by(|a, b| b.cost.total_cmp(&a.cost));
    rows.truncate(TOP_ENDPOINTS);
    rows
}

/// X's reported days on or after `since_ymd`, newest first, each date's
/// latest record winning; `None` when there are none.
pub fn actuals(records: &[Record], since_ymd: i64) -> Option<Vec<ActualUsageDay>> {
    let since = format!(
        "{}-{:02}-{:02}",
        since_ymd / 10000,
        (since_ymd % 10000) / 100,
        since_ymd % 100
    );
    let mut days: BTreeMap<&str, &ActualRecord> = BTreeMap::new();
    for record in records {
        if let Record::Actual(a) = record
            && a.date.as_str() >= since.as_str()
        {
            days.insert(a.date.as_str(), a);
        }
    }
    if days.is_empty() {
        return None;
    }
    Some(
        days.into_values()
            .rev()
            .map(|a| ActualUsageDay {
                date: a.date.clone(),
                tweet_count: u64::try_from(a.post_count).unwrap_or(0),
                synced_at: DateTime::parse_from_rfc3339(&a.synced_at)
                    .ok()
                    .map(|t| t.timestamp()),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
