use std::path::Path;
use std::time::{Duration, SystemTime};

use chrono::{TimeZone, Utc};

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0)
        .single()
        .expect("date")
}

fn touch(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, bytes).expect("write");
}

fn set_mtime(path: &Path, at: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open")
        .set_modified(at)
        .expect("set mtime");
}

#[test]
fn stats_count_each_note_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = StorePaths::under(dir.path());
    let archive = &paths.archive_dir;
    for name in [
        "posts/1.md",
        "posts/2.md",
        "posts/3.md",
        "users/12.md",
        "users/13.md",
    ] {
        touch(&archive.join(name), b"---\n---\n");
    }
    touch(&archive.join("bookmarks/12/1.md"), b"x");
    touch(&archive.join("likes/12/2.md"), b"x");
    touch(&archive.join("my-own-note.md"), b"not bird's");
    let report = StoreReport::gather(&paths, now());
    assert_eq!(report.archive.posts, 3);
    assert_eq!(report.archive.users, 2);
    assert_eq!(report.archive.bookmarks, 1);
    assert_eq!(report.archive.likes, 1);
    assert_eq!(report.archive.source, ArchiveSource::Default);
    assert!(report.archive.exists);
}

#[test]
fn stats_report_fresh_and_stale_responses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = StorePaths::under(dir.path());
    let account = paths.responses_dir.join("0123456789abcdef");
    let names = ["a", "b", "c"].map(|c| account.join(format!("{}.json", c.repeat(64))));
    for name in &names {
        touch(name, b"{}");
    }
    let today = SystemTime::from(now());
    set_mtime(&names[0], today);
    set_mtime(&names[1], today);
    set_mtime(&names[2], today - Duration::from_secs(13 * 3600));
    touch(&paths.responses_dir.join(".pruned"), b"");
    touch(&account.join("notes.txt"), b"stray");
    let report = StoreReport::gather(&paths, now());
    assert_eq!(report.responses.files, 3);
    assert_eq!(report.responses.fresh, 2);
}

#[test]
fn a_missing_store_is_healthy_when_creatable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = StorePaths::under(&dir.path().join("fresh-home"));
    let report = StoreReport::gather(&paths, now());
    assert!(!report.archive.exists);
    assert!(!report.responses.exists);
    assert!(!report.ledger.exists);
    assert!(report.archive.writable);
    assert!(report.healthy);
    assert_eq!(report.archive.posts, 0);
    assert_eq!(report.ledger.size_mb, 0.0);
}

#[cfg(unix)]
#[test]
fn a_read_only_archive_is_unhealthy() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = StorePaths::under(dir.path());
    std::fs::create_dir_all(&paths.archive_dir).expect("archive");
    std::fs::set_permissions(&paths.archive_dir, std::fs::Permissions::from_mode(0o500))
        .expect("chmod");
    let report = StoreReport::gather(&paths, now());
    std::fs::set_permissions(&paths.archive_dir, std::fs::Permissions::from_mode(0o700))
        .expect("chmod back");
    assert!(!report.archive.writable);
    assert!(report.responses.writable);
    assert!(!report.healthy);
}

#[test]
fn the_report_serializes_its_three_locations() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = StorePaths::under(dir.path());
    let value = serde_json::to_value(StoreReport::gather(&paths, now())).expect("json");
    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut keys: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(keys(&value), ["archive", "healthy", "ledger", "responses"]);
    assert_eq!(
        keys(&value["archive"]),
        [
            "bookmarks",
            "exists",
            "likes",
            "path",
            "posts",
            "size_mb",
            "source",
            "users",
            "writable"
        ]
    );
    assert_eq!(
        keys(&value["responses"]),
        ["exists", "files", "fresh", "path", "size_mb", "writable"]
    );
    assert_eq!(keys(&value["ledger"]), ["exists", "path", "size_mb"]);
    assert_eq!(value["archive"]["source"], "default");
}
