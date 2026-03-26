//! Usage command: API cost visibility from local SQLite + optional X API sync.
//! Reads the `usage` table for estimated costs; `--sync` fetches actuals from GET /2/usage/tweets.

use crate::db::{
    ActualUsageDay, BirdClient, DailyUsage, EndpointUsage, RequestContext, UsageSummary,
};
use crate::diag;
use crate::output;
use crate::requirements::AuthType;

/// Parse --since into a YYYYMMDD integer for date_ymd column filtering.
fn parse_since(since: Option<&str>) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    match since {
        Some(date_str) => {
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| format!("invalid date '{}': {} (expected YYYY-MM-DD)", date_str, e))?;
            Ok(date.format("%Y%m%d").to_string().parse::<i64>().unwrap())
        }
        None => {
            let now = chrono::Utc::now();
            let thirty_days_ago = now - chrono::TimeDelta::days(30);
            Ok(thirty_days_ago
                .format("%Y%m%d")
                .to_string()
                .parse::<i64>()
                .unwrap())
        }
    }
}

/// Format a YYYYMMDD integer back to YYYY-MM-DD for display.
fn ymd_to_display(ymd: i64) -> String {
    format!(
        "{}-{:02}-{:02}",
        ymd / 10000,
        (ymd % 10000) / 100,
        ymd % 100
    )
}

pub fn run_usage(
    client: &mut BirdClient,
    since: Option<&str>,
    sync: bool,
    pretty: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let since_ymd = parse_since(since)?;

    // Check DB availability (graceful degradation per D5)
    if client.db().is_none() {
        let msg = if client.db_disabled() {
            "Usage tracking requires the store. Remove --no-cache to enable."
        } else {
            "Store database is unavailable. Run `bird cache clear` to reset."
        };
        diag!(quiet, "[usage] {}", msg);
        if !pretty {
            println!("{}", serde_json::to_string(&empty_report(since_ymd))?);
        }
        return Ok(());
    }

    // Query local data (db() is Some, verified above; re-borrow scoped to avoid conflict with sync)
    let (summary, daily, top_endpoints) = {
        let db = client.db().unwrap();
        (
            db.query_usage_summary(since_ymd)?,
            db.query_daily_usage(since_ymd)?,
            db.query_top_endpoints(since_ymd)?,
        )
    };

    if summary.total_calls == 0 && !sync {
        diag!(
            quiet,
            "[usage] No usage data recorded yet. Run some API commands first."
        );
    }

    // Optionally: sync actual usage from X API
    let mut sync_status = if sync { "failed" } else { "skipped" };
    let actuals = if sync {
        // Validate --since with --sync: warn if older than 90 days
        let now = chrono::Utc::now().date_naive();
        let since_date = chrono::NaiveDate::from_ymd_opt(
            (since_ymd / 10000) as i32,
            ((since_ymd % 10000) / 100) as u32,
            (since_ymd % 100) as u32,
        );
        if let Some(since_date) = since_date {
            let days_back = (now - since_date).num_days();
            if days_back > 90 {
                diag!(
                    quiet,
                    "[usage] warning: X API only returns 90 days of history; --since may exceed that range"
                );
            }
        }

        match sync_actual_usage(client, quiet)? {
            Some(actuals) => {
                sync_status = "success";
                Some(actuals)
            }
            None => client
                .db()
                .and_then(|db| db.query_actual_usage(since_ymd).ok())
                .flatten(),
        }
    } else {
        client
            .db()
            .and_then(|db| db.query_actual_usage(since_ymd).ok())
            .flatten()
    };

    let since_display = since
        .map(String::from)
        .unwrap_or_else(|| ymd_to_display(since_ymd));
    let until_display = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let report = UsageReport {
        since: since_display,
        until: until_display,
        summary,
        daily,
        top_endpoints,
        comparison: actuals,
        sync_status,
    };

    if pretty {
        print_usage_pretty(&report);
    } else {
        println!("{}", serde_json::to_string(&report)?);
    }
    Ok(())
}

fn print_usage_pretty(report: &UsageReport) {
    println!("API Usage ({} to {})", report.since, report.until);
    println!("{}", "-".repeat(45));

    let total_calls = report.summary.total_calls;
    let cache_rate = if total_calls > 0 {
        (report.summary.cache_hits as f64 / total_calls as f64 * 100.0) as i64
    } else {
        0
    };

    println!("Total estimated cost:  ${:.2}", report.summary.total_cost);
    println!("Total API calls:       {}", total_calls);
    println!("Cache hit rate:        {}%", cache_rate);
    println!(
        "Estimated savings:     ~${:.2}",
        report.summary.estimated_savings
    );

    if !report.daily.is_empty() {
        println!("\nDaily breakdown:");
        for day in &report.daily {
            let day_calls = day.calls;
            let day_cache_pct = if day_calls > 0 {
                (day.cache_hits as f64 / day_calls as f64 * 100.0) as i64
            } else {
                0
            };
            println!(
                "  {}  ${:.2}  ({} calls, {}% cached)",
                ymd_to_display(day.date_ymd),
                day.cost,
                day_calls,
                day_cache_pct
            );
        }
    }

    if !report.top_endpoints.is_empty() {
        println!("\nTop endpoints:");
        for ep in &report.top_endpoints {
            println!("  {}  ${:.2}  ({} calls)", ep.endpoint, ep.cost, ep.calls);
        }
    }

    if let Some(ref actuals) = report.comparison
        && !actuals.is_empty()
    {
        let synced_at = actuals
            .first()
            .and_then(|a| a.synced_at)
            .map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        println!("\nEstimated vs Actual (synced {})", synced_at);
        println!("{}", "-".repeat(50));
        println!(
            "  {:<12} {:<14} {:<8} Diff",
            "Date", "Est. tweets", "Actual"
        );
        for actual in actuals {
            println!(
                "  {:<12} {:<14} {:<8}",
                actual.date, "-", actual.tweet_count
            );
        }
    }
}

/// Parse a JSON value that may be an integer or a string-encoded integer.
fn parse_usage_count(v: &serde_json::Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// Sync actual usage from X API via xurl with `--auth app` (Bearer token).
fn sync_actual_usage(
    client: &mut BirdClient,
    quiet: bool,
) -> Result<Option<Vec<ActualUsageDay>>, Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://api.x.com/2/usage/tweets?usage.fields=daily_project_usage";

    // Usage sync requires Bearer (app-only) auth
    let auth_type = AuthType::Bearer;
    let ctx = RequestContext {
        auth_type: &auth_type,
        username: None,
    };

    let response = client.get(url, &ctx)?;

    // Graceful degradation: show local data on sync failure (D5)
    if !response.is_success() {
        let msg = output::sanitize_for_stderr(&response.body, 200);
        if response.body.contains("429") || response.body.contains("Too Many") {
            diag!(quiet, "[usage] Rate limited. Showing local data only.");
        } else {
            diag!(
                quiet,
                "[usage] Sync failed: {}. Showing local data only.",
                msg
            );
        }
        return Ok(None);
    }

    let body = response.json.ok_or("invalid JSON from /2/usage/tweets")?;
    let daily = body
        .pointer("/data/daily_project_usage/usage")
        .and_then(|d| d.as_array())
        .ok_or("unexpected response from /2/usage/tweets (missing daily_project_usage.usage)")?;

    let db = match client.db() {
        Some(db) => db,
        None => {
            diag!(
                quiet,
                "[usage] Cache database unavailable for storing actuals. Showing local data only."
            );
            return Ok(None);
        }
    };

    let mut results = Vec::new();
    for day_entry in daily {
        let date_str = day_entry
            .get("date")
            .and_then(|d| d.as_str())
            .ok_or("missing date field in usage response")?;
        // Parse "2026-02-11T00:00:00.000Z" to "2026-02-11"
        let date = &date_str[..10.min(date_str.len())];

        let usage_count = day_entry.get("usage").map(parse_usage_count).unwrap_or(0);

        db.upsert_actual_usage(date, usage_count)?;
        results.push(ActualUsageDay {
            date: date.to_string(),
            tweet_count: usage_count,
            synced_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            ),
        });
    }

    diag!(
        quiet,
        "[usage] synced {} days of actual usage from X API",
        results.len()
    );
    Ok(Some(results))
}

/// Build an empty report for machine consumers when DB is unavailable.
fn empty_report(since_ymd: i64) -> UsageReport {
    let since_display = ymd_to_display(since_ymd);
    let until_display = chrono::Utc::now().format("%Y-%m-%d").to_string();
    UsageReport {
        since: since_display,
        until: until_display,
        summary: UsageSummary {
            total_cost: 0.0,
            total_calls: 0,
            cache_hits: 0,
            estimated_savings: 0.0,
        },
        daily: vec![],
        top_endpoints: vec![],
        comparison: None,
        sync_status: "skipped",
    }
}

#[derive(serde::Serialize)]
struct UsageReport {
    since: String,
    until: String,
    summary: UsageSummary,
    daily: Vec<DailyUsage>,
    top_endpoints: Vec<EndpointUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comparison: Option<Vec<ActualUsageDay>>,
    /// Machine-readable sync status: "success", "failed", or "skipped".
    sync_status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_since_valid_date() {
        let ymd = parse_since(Some("2026-02-01")).unwrap();
        assert_eq!(ymd, 20260201);
    }

    #[test]
    fn parse_since_none_defaults_to_30_days_ago() {
        let ymd = parse_since(None).unwrap();
        // Should be approximately 30 days ago; just verify it's a valid YYYYMMDD
        assert!(ymd > 20200101);
        assert!(ymd < 30000101);
    }

    #[test]
    fn parse_since_invalid_date() {
        let result = parse_since(Some("not-a-date"));
        assert!(result.is_err());
    }

    #[test]
    fn parse_since_invalid_format() {
        let result = parse_since(Some("02/01/2026"));
        assert!(result.is_err());
    }

    #[test]
    fn ymd_to_display_format() {
        assert_eq!(ymd_to_display(20260211), "2026-02-11");
        assert_eq!(ymd_to_display(20260101), "2026-01-01");
    }

    #[test]
    fn parse_usage_count_integer() {
        let v = serde_json::json!(42);
        assert_eq!(parse_usage_count(&v), 42);
    }

    #[test]
    fn parse_usage_count_string() {
        let v = serde_json::json!("42");
        assert_eq!(parse_usage_count(&v), 42);
    }

    #[test]
    fn parse_usage_count_null() {
        let v = serde_json::json!(null);
        assert_eq!(parse_usage_count(&v), 0);
    }
}
