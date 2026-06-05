//! Usage command: API cost visibility from local SQLite + X API sync (default).
//! Reads the `usage` table for estimated costs; fetches actuals from GET /2/usage/tweets by default.
//! Use `--local` to skip the API and show only local estimates.

mod pretty;
mod sync;

use crate::db::{ActualUsageDay, BirdClient, DailyUsage, EndpointUsage, UsageSummary};

/// Parse --since into a YYYYMMDD integer for date_ymd column filtering.
fn parse_since(since: Option<&str>) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    match since {
        Some(date_str) => {
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| format!("invalid date '{}': {} (expected YYYY-MM-DD)", date_str, e))?;
            Ok(date
                .format("%Y%m%d")
                .to_string()
                .parse::<i64>()
                .expect("invariant: chrono '%Y%m%d' format yields a valid i64"))
        }
        None => {
            let now = chrono::Utc::now();
            let thirty_days_ago = now - chrono::TimeDelta::days(30);
            Ok(thirty_days_ago
                .format("%Y%m%d")
                .to_string()
                .parse::<i64>()
                .expect("invariant: chrono '%Y%m%d' format yields a valid i64"))
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

/// Compute the usage report (with optional API sync) and stream it to the
/// injected stdout writer via a local `BufWriter`. The buffer is flushed at
/// end and on every early-return path. Mid-stream errors propagate `?`; the
/// BufWriter is dropped on unwind, flushing buffered bytes in debug builds.
/// In release builds bird uses `panic = "abort"`, so mid-stream aborts may
/// truncate — accepted property of streaming output.
pub fn run_usage(
    client: &mut BirdClient,
    cfg: &crate::output::OutputConfig,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
    since: Option<&str>,
    local: bool,
    pretty: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    let mut out = std::io::BufWriter::new(stdout);
    let quiet = cfg.suppress_diag();
    let since_ymd = parse_since(since)?;

    // Check DB availability (graceful degradation per D5)
    if client.db().is_none() {
        let msg = if client.db_disabled() {
            "Usage tracking requires the store. Remove --no-cache to enable."
        } else {
            "Store database is unavailable. Run `bird cache clear` to reset."
        };
        if !quiet {
            writeln!(stderr, "[usage] {}", msg).ok();
        }
        if !pretty {
            writeln!(out, "{}", serde_json::to_string(&empty_report(since_ymd))?)?;
        }
        out.flush()?;
        return Ok(());
    }

    // Query local data (db() is Some, verified above; re-borrow scoped to avoid API call below)
    let (summary, daily, top_endpoints) = {
        let db = client
            .db()
            .expect("invariant: db().is_none() short-circuits above");
        (
            db.query_usage_summary(since_ymd)?,
            db.query_daily_usage(since_ymd)?,
            db.query_top_endpoints(since_ymd)?,
        )
    };

    if summary.total_calls == 0 && local && !quiet {
        writeln!(
            stderr,
            "[usage] No usage data recorded yet. Run some API commands first."
        )
        .ok();
    }

    // Fetch actual usage from X API (default; skipped with --local)
    let mut sync_status = if local { "skipped" } else { "failed" };
    let (actuals, cap, per_app) = if !local {
        // Validate --since with API sync: warn if older than 90 days
        let now = chrono::Utc::now().date_naive();
        let since_date = chrono::NaiveDate::from_ymd_opt(
            (since_ymd / 10000) as i32,
            ((since_ymd % 10000) / 100) as u32,
            (since_ymd % 100) as u32,
        );
        if let Some(since_date) = since_date {
            let days_back = (now - since_date).num_days();
            if days_back > 90 && !quiet {
                writeln!(
                    stderr,
                    "[usage] warning: X API only returns 90 days of history; --since may exceed that range"
                )
                .ok();
            }
        }

        match sync::sync_actual_usage(client, stderr, quiet)? {
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
        pretty::print_usage_pretty(&mut out, &report)?;
    } else {
        writeln!(out, "{}", serde_json::to_string(&report)?)?;
    }
    out.flush()?;
    Ok(())
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

#[cfg(all(test, not(feature = "embedded-xurl")))]
mod tests {
    use super::*;
    use crate::db::BirdClient;
    use crate::db::store::in_memory_db;
    use crate::output::{OutputConfig, OutputFormat};
    use crate::transport::tests::MockTransport;

    fn sync_client(responses: Vec<serde_json::Value>) -> BirdClient {
        let mock = MockTransport::new(responses.into_iter().map(Ok).collect());
        BirdClient::new_test(Box::new(mock), in_memory_db())
    }

    fn quiet_cfg() -> OutputConfig {
        OutputConfig {
            format: OutputFormat::Text,
            use_color: false,
            quiet: true,
            raw: false,
        }
    }

    #[test]
    fn parse_since_valid_date() {
        let ymd = parse_since(Some("2026-02-01")).expect("test");
        assert_eq!(ymd, 20260201);
    }

    #[test]
    fn parse_since_none_defaults_to_30_days_ago() {
        let ymd = parse_since(None).expect("test");
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
    fn run_usage_local_skips_api() {
        let mut client = sync_client(vec![]);
        let cfg = quiet_cfg();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        run_usage(
            &mut client,
            &cfg,
            &mut stdout,
            &mut stderr,
            None,
            true,
            false,
        )
        .expect("test");
    }

    #[test]
    fn run_usage_default_calls_api() {
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
        let cfg = quiet_cfg();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        run_usage(
            &mut client,
            &cfg,
            &mut stdout,
            &mut stderr,
            None,
            false,
            false,
        )
        .expect("test");
    }

    #[test]
    fn run_usage_default_with_empty_mock_errors() {
        let mut client = sync_client(vec![]);
        let cfg = quiet_cfg();
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        let result = run_usage(
            &mut client,
            &cfg,
            &mut stdout,
            &mut stderr,
            None,
            false,
            false,
        );
        assert!(result.is_err(), "local=false should attempt API call");
    }
}
