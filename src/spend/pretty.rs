//! Human-readable text rendering of a `UsageReport`. Pure view: no DB, no network.

use super::{UsageReport, ymd_to_display};

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

pub(super) fn print_usage_pretty(
    out: &mut impl std::io::Write,
    report: &UsageReport,
) -> std::io::Result<()> {
    writeln!(out, "API Usage ({} to {})", report.since, report.until)?;
    writeln!(out, "{}", "-".repeat(45))?;

    if let Some(ref cap) = report.cap {
        let pct = if cap.project_cap > 0 {
            cap.project_usage as f64 / cap.project_cap as f64 * 100.0
        } else {
            0.0
        };
        writeln!(
            out,
            "Project cap:           {} / {} ({:.2}%)",
            format_number(cap.project_usage),
            format_number(cap.project_cap),
            pct
        )?;
        writeln!(
            out,
            "Cap reset day:         {}",
            ordinal_day(cap.cap_reset_day)
        )?;
    }

    let total_calls = report.summary.total_calls;
    let cache_rate = if total_calls > 0 {
        (report.summary.cache_hits as f64 / total_calls as f64 * 100.0) as i64
    } else {
        0
    };

    writeln!(
        out,
        "Total estimated cost:  ${:.2}",
        report.summary.total_cost
    )?;
    writeln!(out, "Total API calls:       {}", total_calls)?;
    writeln!(out, "Cache hit rate:        {}%", cache_rate)?;
    writeln!(
        out,
        "Estimated savings:     ~${:.2}",
        report.summary.estimated_savings
    )?;

    if !report.daily.is_empty() {
        writeln!(out, "\nDaily breakdown:")?;
        for day in &report.daily {
            let day_calls = day.calls;
            let day_cache_pct = if day_calls > 0 {
                (day.cache_hits as f64 / day_calls as f64 * 100.0) as i64
            } else {
                0
            };
            writeln!(
                out,
                "  {}  ${:.2}  ({} calls, {}% cached)",
                ymd_to_display(day.date_ymd),
                day.cost,
                day_calls,
                day_cache_pct
            )?;
        }
    }

    if !report.top_endpoints.is_empty() {
        writeln!(out, "\nTop endpoints:")?;
        for ep in &report.top_endpoints {
            writeln!(
                out,
                "  {}  ${:.2}  ({} calls)",
                ep.endpoint, ep.cost, ep.calls
            )?;
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

        writeln!(out, "\nEstimated vs Actual (synced {})", synced_at)?;
        writeln!(out, "{}", "-".repeat(50))?;
        writeln!(
            out,
            "  {:<12} {:<14} {:<8} Diff",
            "Date", "Est. tweets", "Actual"
        )?;
        for actual in actuals {
            writeln!(
                out,
                "  {:<12} {:<14} {:<8}",
                actual.date, "-", actual.tweet_count
            )?;
        }
    }

    if !report.per_app.is_empty() {
        writeln!(out, "\nPer-app breakdown:")?;
        for entry in &report.per_app {
            writeln!(
                out,
                "  app={}  {}  {} tweets",
                entry.client_app_id,
                entry.date,
                format_number(entry.tweet_count)
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
