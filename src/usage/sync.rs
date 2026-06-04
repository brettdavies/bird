//! `/2/usage/tweets` sync: pull actual usage from X API, persist to DB, return shaped data.

use super::{AppDailyUsage, ProjectCap, SyncData};
use crate::db::{ActualUsageDay, BirdClient, RequestContext};
use crate::output;
use crate::requirements::AuthType;

/// Parse a JSON value that may be an integer or a string-encoded integer.
fn parse_usage_count(v: &serde_json::Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// Sync actual usage from X API via xurl with `--auth app` (Bearer token).
pub(super) fn sync_actual_usage(
    client: &mut BirdClient,
    stderr: &mut dyn std::io::Write,
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
        if !quiet {
            let body = response.body();
            if body.contains("429") || body.contains("Too Many") {
                writeln!(stderr, "[usage] Rate limited. Showing local data only.").ok();
            } else {
                let msg = output::sanitize_for_stderr(&body, 200);
                writeln!(
                    stderr,
                    "[usage] Sync failed: {}. Showing local data only.",
                    msg
                )
                .ok();
            }
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
            if !quiet {
                writeln!(
                    stderr,
                    "[usage] Cache database unavailable for storing actuals. Showing local data only."
                )
                .ok();
            }
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
                    .expect("invariant: system clock is past UNIX_EPOCH")
                    .as_secs() as i64,
            ),
        });
    }

    if !quiet {
        writeln!(
            stderr,
            "[usage] synced {} days of actual usage from X API",
            results.len()
        )
        .ok();
    }
    Ok(Some(SyncData {
        daily: results,
        cap,
        per_app,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::BirdClient;
    use crate::db::store::in_memory_db;
    use crate::transport::tests::MockTransport;

    /// Build a BirdClient backed by MockTransport + in-memory DB.
    fn sync_client(responses: Vec<serde_json::Value>) -> BirdClient {
        let mock = MockTransport::new(responses.into_iter().map(Ok).collect());
        BirdClient::new_test(Box::new(mock), in_memory_db())
    }

    fn do_sync(
        client: &mut BirdClient,
    ) -> Result<Option<SyncData>, Box<dyn std::error::Error + Send + Sync>> {
        let mut stderr: Vec<u8> = Vec::new();
        sync_actual_usage(client, &mut stderr, true)
    }

    #[test]
    fn sync_parses_live_api_response_shape() {
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        assert_eq!(sync_data.daily.len(), 2);
        assert_eq!(sync_data.daily[0].date, "2026-03-25");
        assert_eq!(sync_data.daily[0].tweet_count, 299);
        assert_eq!(sync_data.daily[1].date, "2026-03-26");
        assert_eq!(sync_data.daily[1].tweet_count, 100);
        let cap = sync_data.cap.expect("test");
        assert_eq!(cap.project_usage, 399);
        assert_eq!(cap.project_cap, 2_000_000);
        assert_eq!(cap.cap_reset_day, 19);
        assert_eq!(sync_data.per_app.len(), 1);
        assert_eq!(sync_data.per_app[0].client_app_id, "32371675");
        assert_eq!(sync_data.per_app[0].date, "2026-03-25");
        assert_eq!(sync_data.per_app[0].tweet_count, 299);
    }

    #[test]
    fn sync_usage_as_integer_not_string() {
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        assert!(sync_data.daily.is_empty());
    }

    #[test]
    fn sync_missing_daily_project_usage_returns_error() {
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        assert_eq!(sync_data.daily.len(), 1);
        assert_eq!(sync_data.daily[0].tweet_count, 0);
    }

    #[test]
    fn sync_missing_usage_field_treated_as_zero() {
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        assert_eq!(sync_data.daily.len(), 1);
        assert_eq!(sync_data.daily[0].tweet_count, 0);
    }

    #[test]
    fn sync_missing_date_field_returns_error() {
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
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
        do_sync(&mut client).expect("test");

        let actuals = client
            .db()
            .expect("test")
            .query_actual_usage(20260301)
            .expect("test");
        assert!(actuals.is_some());
        let days = actuals.expect("test");
        assert_eq!(days.len(), 2);
        let mut counts: Vec<u64> = days.iter().map(|d| d.tweet_count).collect();
        counts.sort();
        assert_eq!(counts, vec![100, 299]);
    }

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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        let cap = sync_data.cap.expect("test");
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
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
        let sync_data = do_sync(&mut client).expect("test").expect("test");
        assert!(sync_data.cap.is_none());
        assert!(sync_data.per_app.is_empty());
    }

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
