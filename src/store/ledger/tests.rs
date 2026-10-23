use std::sync::Arc;

use chrono::{DateTime, TimeDelta, TimeZone, Utc};

use super::*;

pub(crate) fn request(ts: DateTime<Utc>, endpoint: &str, cost: f64, cache_hit: bool) -> Record {
    Record::Request(RequestRecord {
        v: FORMAT,
        ts: ts.to_rfc3339(),
        method: "GET".into(),
        endpoint: endpoint.into(),
        object_type: "post".into(),
        object_count: 1,
        estimated_cost: cost,
        cache_hit,
        app: String::new(),
        username: String::new(),
    })
}

pub(crate) fn actual(date: &str, count: i64, synced_at: DateTime<Utc>) -> Record {
    Record::Actual(ActualRecord {
        v: FORMAT,
        date: date.into(),
        post_count: count,
        synced_at: synced_at.to_rfc3339(),
    })
}

fn ledger_in(dir: &tempfile::TempDir) -> Ledger {
    Ledger::at(
        dir.path().join("spend.jsonl"),
        dir.path().join("spend.lock"),
    )
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0)
        .single()
        .expect("date")
}

#[test]
fn a_record_round_trips_as_one_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = ledger_in(&dir);
    let record = request(now(), "/2/tweets/:id", 0.005, false);
    ledger.append(&record, now()).expect("append");
    let text = std::fs::read_to_string(ledger.path()).expect("read");
    assert_eq!(text.lines().count(), 1);
    let line: serde_json::Value = serde_json::from_str(text.trim()).expect("json");
    assert_eq!(line["kind"], "request");
    assert_eq!(line["v"], 1);
    assert_eq!(ledger.read().expect("read").records, vec![record]);
}

#[test]
fn records_older_than_90_days_are_compacted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let old = now() - TimeDelta::days(91);
    let lines = [
        request(old, "/2/old", 0.1, false),
        request(old + TimeDelta::hours(1), "/2/old2", 0.1, false),
        request(now() - TimeDelta::days(3), "/2/kept", 0.2, false),
    ];
    let mut text = String::new();
    for line in &lines {
        text.push_str(&serde_json::to_string(line).expect("json"));
        text.push('\n');
    }
    std::fs::write(dir.path().join("spend.jsonl"), text).expect("seed");
    let ledger = ledger_in(&dir);
    let fresh = request(now(), "/2/new", 0.3, false);
    ledger.append(&fresh, now()).expect("append");
    let records = ledger.read().expect("read").records;
    assert_eq!(records, vec![lines[2].clone(), fresh]);
}

#[test]
fn compaction_keeps_each_dates_latest_actual() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = ledger_in(&dir);
    let first = actual("2026-10-21", 5, now() - TimeDelta::hours(2));
    let other = actual("2026-10-20", 9, now() - TimeDelta::hours(2));
    let later = actual("2026-10-21", 7, now());
    for record in [&first, &other, &later] {
        ledger.append(record, now()).expect("append");
    }
    ledger.compact(now()).expect("compact");
    assert_eq!(ledger.read().expect("read").records, vec![other, later]);
}

#[test]
fn an_unreadable_line_is_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = ledger_in(&dir);
    let a = request(now(), "/2/a", 0.1, false);
    let b = request(now(), "/2/b", 0.1, true);
    ledger.append(&a, now()).expect("append");
    append_line(ledger.path(), b"{\"kind\":\"request\",\"v\":1,\"ts\":\n").expect("torn");
    ledger.append(&b, now()).expect("append");
    let contents = ledger.read().expect("read");
    assert_eq!(contents.records, vec![a, b]);
    assert_eq!(contents.skipped, 1);
}

#[test]
fn an_empty_ledger_reads_as_no_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let contents = ledger_in(&dir).read().expect("read");
    assert!(contents.records.is_empty());
    assert_eq!(contents.skipped, 0);
}

#[test]
fn parallel_appends_lose_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = Arc::new(ledger_in(&dir));
    let writers: Vec<_> = (0..2)
        .map(|t| {
            let ledger = Arc::clone(&ledger);
            std::thread::spawn(move || {
                for i in 0..500 {
                    let record = request(now(), &format!("/2/t{t}/{i}"), 0.001, false);
                    ledger.append(&record, now()).expect("append");
                    if t == 0 && i == 250 {
                        ledger.compact(now()).expect("compact");
                    }
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("writer");
    }
    let contents = ledger.read().expect("read");
    assert_eq!(contents.records.len(), 1000);
    assert_eq!(contents.skipped, 0);
}

#[cfg(unix)]
#[test]
fn the_ledger_and_its_lock_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = ledger_in(&dir);
    ledger
        .append(&request(now(), "/2/a", 0.1, false), now())
        .expect("append");
    for name in ["spend.jsonl", "spend.lock"] {
        let mode = std::fs::metadata(dir.path().join(name))
            .expect("stat")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}
