//! Usage command: API cost visibility from local SQLite + X API sync (default).
//! Reads the `usage` table for estimated costs; fetches actuals from GET /2/usage/tweets by default.
//! Use `--local` to skip the API and show only local estimates.

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
    local: bool,
    pretty: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let since_ymd = parse_since(since)?;
    let sync = !local;

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

    if summary.total_calls == 0 && local {
        diag!(
            quiet,
            "[usage] No usage data recorded yet. Run some API commands first."
        );
    }

    // Sync actual usage from X API (default; skipped with --local)
    let mut sync_status = if sync { "failed" } else { "skipped" };
    let (actuals, cap, per_app) = if sync {
        // Validate --since with API sync: warn if older than 90 days
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
            Some(sync_data) => {
                sync_status = "success";
                (Some(sync_data.daily), sync_data.cap, sync_data.per_app)
            }
            None => {
                let fallback = client
                    .db()
                    .and_then(|db| db.query_actual_usage(since_ymd).ok())
                    .flatten();
                (fallback, None, vec![])
            }
        }
    } else {
        let fallback = client
            .db()
            .and_then(|db| db.query_actual_usage(since_ymd).ok())
            .flatten();
        (fallback, None, vec![])
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
        cap,
        per_app,
        sync_status,
    };

    if pretty {
        print_usage_pretty(&report);
    } else {
        println!("{}", serde_json::to_string(&report)?);
    }
    Ok(())
}

/// Format a u64 with comma separators (e.g., 2000000 -> "2,000,000").
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Format a day-of-month with ordinal suffix (e.g., 1 -> "1st", 19 -> "19th").
fn ordinal_day(day: u32) -> String {
    let suffix = match (day % 10, day % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{}{}", day, suffix)
}

fn print_usage_pretty(report: &UsageReport) {
    println!("API Usage ({} to {})", report.since, report.until);
    println!("{}", "-".repeat(45));

    if let Some(ref cap) = report.cap {
        let pct = if cap.project_cap > 0 {
            cap.project_usage as f64 / cap.project_cap as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "Project cap:           {} / {} ({:.2}%)",
            format_number(cap.project_usage),
            format_number(cap.project_cap),
            pct
        );
        println!("Cap reset day:         {}", ordinal_day(cap.cap_reset_day));
    }

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

    if !report.per_app.is_empty() {
        println!("\nPer-app breakdown:");
        for entry in &report.per_app {
            println!(
                "  app={}  {}  {} tweets",
                entry.client_app_id,
                entry.date,
                format_number(entry.tweet_count)
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
) -> Result<Option<SyncData>, Box<dyn std::error::Error + Send + Sync>> {
    let url =
        "https://api.x.com/2/usage/tweets?usage.fields=daily_project_usage,daily_client_app_usage";

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
    let data = body.get("data");
    let daily = data
        .and_then(|d| d.pointer("/daily_project_usage/usage"))
        .and_then(|d| d.as_array())
        .ok_or("unexpected response from /2/usage/tweets (missing daily_project_usage.usage)")?;

    // Extract project cap info (optional — not all responses include it)
    let cap = data.and_then(|d| {
        let project_usage = d.get("project_usage").map(parse_usage_count)?;
        let project_cap = d.get("project_cap").map(parse_usage_count)?;
        let cap_reset_day = d
            .get("cap_reset_day")
            .map(|v| parse_usage_count(v) as u32)?;
        Some(ProjectCap {
            project_usage,
            project_cap,
            cap_reset_day,
        })
    });

    // Extract per-app daily usage (optional)
    let mut per_app = Vec::new();
    if let Some(apps) = data
        .and_then(|d| d.get("daily_client_app_usage"))
        .and_then(|a| a.as_array())
    {
        for app in apps {
            let app_id = app
                .get("client_app_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            if let Some(usage_arr) = app.get("usage").and_then(|u| u.as_array()) {
                for entry in usage_arr {
                    let date_str = entry.get("date").and_then(|d| d.as_str()).unwrap_or("");
                    let date = &date_str[..10.min(date_str.len())];
                    let count = entry.get("usage").map(parse_usage_count).unwrap_or(0);
                    per_app.push(AppDailyUsage {
                        client_app_id: app_id.to_string(),
                        date: date.to_string(),
                        tweet_count: count,
                    });
                }
            }
        }
    }

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
    Ok(Some(SyncData {
        daily: results,
        cap,
        per_app,
    }))
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
        cap: None,
        per_app: vec![],
        sync_status: "skipped",
    }
}

#[derive(Debug, serde::Serialize)]
struct ProjectCap {
    project_usage: u64,
    project_cap: u64,
    cap_reset_day: u32,
}

#[derive(Debug, serde::Serialize)]
struct AppDailyUsage {
    client_app_id: String,
    date: String,
    tweet_count: u64,
}

#[derive(Debug)]
struct SyncData {
    daily: Vec<ActualUsageDay>,
    cap: Option<ProjectCap>,
    per_app: Vec<AppDailyUsage>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    cap: Option<ProjectCap>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    per_app: Vec<AppDailyUsage>,
    /// Machine-readable sync status: "success", "failed", or "skipped".
    sync_status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::BirdClient;
    use crate::db::db::in_memory_db;
    use crate::transport::tests::MockTransport;

    /// Build a BirdClient backed by MockTransport + in-memory DB.
    fn sync_client(responses: Vec<serde_json::Value>) -> BirdClient {
        let mock = MockTransport::new(responses.into_iter().map(Ok).collect());
        BirdClient::new_test(Box::new(mock), in_memory_db())
    }

    /// Helper: call sync_actual_usage with a mock client.
    fn do_sync(
        client: &mut BirdClient,
    ) -> Result<Option<SyncData>, Box<dyn std::error::Error + Send + Sync>> {
        sync_actual_usage(client, true)
    }

    // -- Regression tests for the bug we fixed --

    #[test]
    fn sync_parses_live_api_response_shape() {
        // Real response shape from X API /2/usage/tweets
        let api_response = serde_json::json!({
            "data": {
                "project_cap": "2000000",
                "project_id": "2020044302890438656",
                "project_usage": "399",
                "cap_reset_day": 19,
                "daily_project_usage": {
                    "project_id": "2020044302890438656",
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "299"},
                        {"date": "2026-03-26T00:00:00.000Z", "usage": "100"}
                    ]
                },
                "daily_client_app_usage": [
                    {"client_app_id": "32371675", "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "299"}
                    ], "usage_result_count": 1}
                ]
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        // Daily
        assert_eq!(sync_data.daily.len(), 2);
        assert_eq!(sync_data.daily[0].date, "2026-03-25");
        assert_eq!(sync_data.daily[0].tweet_count, 299);
        assert_eq!(sync_data.daily[1].date, "2026-03-26");
        assert_eq!(sync_data.daily[1].tweet_count, 100);
        // Cap
        let cap = sync_data.cap.unwrap();
        assert_eq!(cap.project_usage, 399);
        assert_eq!(cap.project_cap, 2_000_000);
        assert_eq!(cap.cap_reset_day, 19);
        // Per-app
        assert_eq!(sync_data.per_app.len(), 1);
        assert_eq!(sync_data.per_app[0].client_app_id, "32371675");
        assert_eq!(sync_data.per_app[0].date, "2026-03-25");
        assert_eq!(sync_data.per_app[0].tweet_count, 299);
    }

    #[test]
    fn sync_usage_as_integer_not_string() {
        // API could return usage as integer instead of string
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": 42}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert_eq!(sync_data.daily.len(), 1);
        assert_eq!(sync_data.daily[0].tweet_count, 42);
    }

    #[test]
    fn sync_empty_usage_array() {
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": []
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert!(sync_data.daily.is_empty());
    }

    #[test]
    fn sync_missing_daily_project_usage_returns_error() {
        // API returns data but no daily_project_usage key
        let api_response = serde_json::json!({
            "data": {
                "project_usage": "399"
            }
        });
        let mut client = sync_client(vec![api_response]);
        let err = do_sync(&mut client).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing daily_project_usage.usage")
        );
    }

    #[test]
    fn sync_missing_data_key_returns_error() {
        // API returns JSON but no data wrapper (e.g., error response)
        let api_response = serde_json::json!({
            "errors": [{"message": "something went wrong"}]
        });
        let mut client = sync_client(vec![api_response]);
        let err = do_sync(&mut client).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing daily_project_usage.usage")
        );
    }

    #[test]
    fn sync_null_usage_treated_as_zero() {
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": null}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert_eq!(sync_data.daily.len(), 1);
        assert_eq!(sync_data.daily[0].tweet_count, 0);
    }

    #[test]
    fn sync_missing_usage_field_treated_as_zero() {
        // Day entry has date but no usage field at all
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert_eq!(sync_data.daily.len(), 1);
        assert_eq!(sync_data.daily[0].tweet_count, 0);
    }

    #[test]
    fn sync_missing_date_field_returns_error() {
        // Day entry has usage but no date — should fail
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"usage": "299"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let err = do_sync(&mut client).unwrap_err();
        assert!(err.to_string().contains("missing date field"));
    }

    #[test]
    fn sync_short_date_truncated_safely() {
        // Date shorter than 10 chars — code uses min(10, len)
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03", "usage": "10"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert_eq!(sync_data.daily[0].date, "2026-03");
        assert_eq!(sync_data.daily[0].tweet_count, 10);
    }

    #[test]
    fn sync_persists_to_db() {
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "299"},
                        {"date": "2026-03-26T00:00:00.000Z", "usage": "100"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        do_sync(&mut client).unwrap();

        // Verify data was persisted to DB
        let actuals = client.db().unwrap().query_actual_usage(20260301).unwrap();
        assert!(actuals.is_some());
        let days = actuals.unwrap();
        assert_eq!(days.len(), 2);
        let mut counts: Vec<u64> = days.iter().map(|d| d.tweet_count).collect();
        counts.sort();
        assert_eq!(counts, vec![100, 299]);
    }

    // -- parse_usage_count edge cases from red-team --

    #[test]
    fn parse_usage_count_float_treated_as_zero() {
        let v = serde_json::json!(42.5);
        assert_eq!(parse_usage_count(&v), 0);
    }

    #[test]
    fn parse_usage_count_negative_treated_as_zero() {
        let v = serde_json::json!(-5);
        assert_eq!(parse_usage_count(&v), 0);
    }

    #[test]
    fn parse_usage_count_non_numeric_string_treated_as_zero() {
        let v = serde_json::json!("not-a-number");
        assert_eq!(parse_usage_count(&v), 0);
    }

    #[test]
    fn parse_usage_count_bool_treated_as_zero() {
        let v = serde_json::json!(true);
        assert_eq!(parse_usage_count(&v), 0);
    }

    #[test]
    fn parse_usage_count_large_string_number() {
        let v = serde_json::json!("999999999");
        assert_eq!(parse_usage_count(&v), 999999999);
    }

    // -- Existing tests --

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

    // -- Cap and per-app tests --

    #[test]
    fn sync_extracts_cap_info() {
        let api_response = serde_json::json!({
            "data": {
                "project_cap": "2000000",
                "project_usage": "399",
                "cap_reset_day": 19,
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "100"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        let cap = sync_data.cap.unwrap();
        assert_eq!(cap.project_usage, 399);
        assert_eq!(cap.project_cap, 2_000_000);
        assert_eq!(cap.cap_reset_day, 19);
    }

    #[test]
    fn sync_extracts_per_app_usage() {
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "399"}
                    ]
                },
                "daily_client_app_usage": [
                    {"client_app_id": "32371675", "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "299"},
                        {"date": "2026-03-26T00:00:00.000Z", "usage": "100"}
                    ], "usage_result_count": 2},
                    {"client_app_id": "99999999", "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "50"}
                    ], "usage_result_count": 1}
                ]
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert_eq!(sync_data.per_app.len(), 3);
        assert_eq!(sync_data.per_app[0].client_app_id, "32371675");
        assert_eq!(sync_data.per_app[0].date, "2026-03-25");
        assert_eq!(sync_data.per_app[0].tweet_count, 299);
        assert_eq!(sync_data.per_app[1].client_app_id, "32371675");
        assert_eq!(sync_data.per_app[1].date, "2026-03-26");
        assert_eq!(sync_data.per_app[1].tweet_count, 100);
        assert_eq!(sync_data.per_app[2].client_app_id, "99999999");
        assert_eq!(sync_data.per_app[2].tweet_count, 50);
    }

    #[test]
    fn sync_missing_cap_fields_returns_none() {
        // Response has daily_project_usage but no cap fields
        let api_response = serde_json::json!({
            "data": {
                "daily_project_usage": {
                    "usage": [
                        {"date": "2026-03-25T00:00:00.000Z", "usage": "100"}
                    ]
                }
            }
        });
        let mut client = sync_client(vec![api_response]);
        let sync_data = do_sync(&mut client).unwrap().unwrap();
        assert!(sync_data.cap.is_none());
        assert!(sync_data.per_app.is_empty());
    }

    // -- format_number and ordinal_day tests --

    #[test]
    fn format_number_with_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(2_000_000), "2,000,000");
        assert_eq!(format_number(123_456_789), "123,456,789");
    }

    #[test]
    fn ordinal_day_suffixes() {
        assert_eq!(ordinal_day(1), "1st");
        assert_eq!(ordinal_day(2), "2nd");
        assert_eq!(ordinal_day(3), "3rd");
        assert_eq!(ordinal_day(4), "4th");
        assert_eq!(ordinal_day(11), "11th");
        assert_eq!(ordinal_day(12), "12th");
        assert_eq!(ordinal_day(13), "13th");
        assert_eq!(ordinal_day(19), "19th");
        assert_eq!(ordinal_day(21), "21st");
        assert_eq!(ordinal_day(22), "22nd");
        assert_eq!(ordinal_day(23), "23rd");
        assert_eq!(ordinal_day(31), "31st");
    }
}
