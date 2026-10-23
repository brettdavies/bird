use std::time::SystemTime;

use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::json;

use super::*;

const TWEET: &str = "https://api.x.com/2/tweets/1849";

fn account(app: &str, user: &str) -> Account {
    Account {
        app: app.into(),
        user: user.into(),
    }
}

fn alice() -> Account {
    account("default", "alice")
}

fn store() -> (tempfile::TempDir, Responses) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let responses = Responses::new(tmp.path().join("responses"));
    (tmp, responses)
}

fn set_mtime(path: &Path, at: DateTime<Utc>) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open")
        .set_modified(SystemTime::from(at))
        .expect("set mtime");
}

fn body() -> serde_json::Value {
    json!({"data": {"id": "1849", "text": "hi"}, "errors": [{"detail": "partial"}]})
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_same_day_response_answers_the_same_request() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("write");
    let hit = r.read("GET", TWEET, &alice(), Utc::now()).expect("hit");
    assert_eq!(hit.status, 200);
    assert_eq!(hit.body, body());
}

#[test]
fn request_key_is_deterministic_and_method_aware() {
    let url = "https://api.x.com/2/users/me";
    assert_eq!(request_key("GET", url), request_key("GET", url));
    assert_ne!(request_key("GET", url), request_key("POST", url));
    assert_eq!(request_key("GET", url).len(), 64);
    assert_eq!(account_key(&alice()).len(), 16);
}

#[test]
fn query_order_does_not_change_the_key() {
    assert_eq!(
        request_key("GET", "https://api.x.com/2/tweets?a=1&b=2"),
        request_key("GET", "https://api.x.com/2/tweets?b=2&a=1")
    );
}

#[test]
fn an_encoded_ampersand_does_not_collide() {
    for (encoded, split) in [("?q=a%26b%3Dc", "?q=a&b=c"), ("?a=1%26b%3D2", "?a=1&b=2")] {
        assert_ne!(
            request_key("GET", &format!("https://api.x.com/2/tweets{encoded}")),
            request_key("GET", &format!("https://api.x.com/2/tweets{split}")),
            "{encoded} and {split} must not share a key"
        );
    }
}

#[test]
fn id_list_order_is_part_of_the_key() {
    assert_ne!(
        request_key("GET", "https://api.x.com/2/tweets?ids=3,1,2"),
        request_key("GET", "https://api.x.com/2/tweets?ids=1,2,3")
    );
    assert_ne!(
        request_key("GET", "https://api.x.com/2/users/by?usernames=b,a"),
        request_key("GET", "https://api.x.com/2/users/by?usernames=a,b")
    );
}

#[test]
fn another_account_gets_no_answer() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("write");
    let now = Utc::now();
    assert!(
        r.read("GET", TWEET, &account("default", "bob"), now)
            .is_none()
    );
    assert!(
        r.read("GET", TWEET, &account("other", "alice"), now)
            .is_none()
    );
}

#[test]
fn app_only_requests_key_on_an_empty_user() {
    let (_tmp, r) = store();
    let app_only = Account::for_request("default", true, Some("alice"), Some("alice"))
        .expect("app-only account");
    assert_eq!(app_only.user, "");
    r.write("GET", TWEET, &app_only, 200, &body())
        .expect("write");
    assert!(r.read("GET", TWEET, &app_only, Utc::now()).is_some());
    assert!(r.read("GET", TWEET, &alice(), Utc::now()).is_none());
}

#[test]
fn an_unnamed_user_context_request_is_not_cached() {
    let (_tmp, r) = store();
    let app_only = Account::for_request("default", true, None, None).expect("app-only");
    r.write("GET", TWEET, &app_only, 200, &body())
        .expect("write");
    assert_eq!(Account::for_request("default", false, None, None), None);
    assert_eq!(
        Account::for_request("default", false, Some(""), Some("")),
        None
    );
    assert_eq!(
        Account::for_request("default", false, None, Some("alice")),
        Some(alice()),
        "the app's default user names the account"
    );
    assert_eq!(
        Account::for_request("default", false, Some("bob"), Some("alice")),
        Some(account("default", "bob")),
        "a requested username wins over the default"
    );
}

#[test]
fn yesterdays_file_is_a_miss() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("write");
    let path = r.path_for("GET", TWEET, &alice());
    let midnight = Utc
        .with_ymd_and_hms(2026, 10, 22, 0, 0, 0)
        .single()
        .expect("date");
    let now = midnight + Duration::seconds(1);
    set_mtime(&path, midnight - Duration::seconds(1));
    assert!(r.read("GET", TWEET, &alice(), now).is_none());
    set_mtime(&path, midnight);
    assert!(r.read("GET", TWEET, &alice(), now).is_some());
}

#[test]
fn a_file_for_another_request_under_the_same_name_is_a_miss() {
    let (_tmp, r) = store();
    let other = "https://api.x.com/2/tweets/1850";
    r.write("GET", other, &alice(), 200, &body())
        .expect("write");
    std::fs::rename(
        r.path_for("GET", other, &alice()),
        r.path_for("GET", TWEET, &alice()),
    )
    .expect("rename");
    assert!(r.read("GET", TWEET, &alice(), Utc::now()).is_none());
}

#[test]
fn an_unreadable_file_is_a_miss() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("write");
    let path = r.path_for("GET", TWEET, &alice());
    std::fs::write(&path, br#"{"format":1,"method":"GET","ur"#).expect("truncate");
    assert!(r.read("GET", TWEET, &alice(), Utc::now()).is_none());
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("rewrite");
    assert!(r.read("GET", TWEET, &alice(), Utc::now()).is_some());
}

#[test]
fn non_2xx_bodies_are_not_stored() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 404, &body())
        .expect("write");
    assert!(!r.path_for("GET", TWEET, &alice()).exists());
    assert!(!r.dir().exists());
}

#[test]
fn non_get_requests_are_not_stored() {
    let (_tmp, r) = store();
    let url = "https://api.x.com/2/users/12/bookmarks";
    assert!(!is_answerable("POST", url));
    r.write("POST", url, &alice(), 200, &body()).expect("write");
    assert!(!r.dir().exists());
    assert!(r.read("POST", url, &alice(), Utc::now()).is_none());
}

#[test]
fn state_families_are_never_stored() {
    for url in [
        "https://api.x.com/2/dm_events",
        "https://api.x.com/2/usage/credits",
        "https://api.x.com/2/media/upload?command=STATUS&media_id=1",
    ] {
        let (_tmp, r) = store();
        assert!(!is_answerable("GET", url), "{url}");
        r.write("GET", url, &alice(), 200, &body()).expect("write");
        assert!(!r.dir().exists(), "{url} wrote a file");
        assert!(r.read("GET", url, &alice(), Utc::now()).is_none(), "{url}");
    }
    assert!(is_answerable(
        "GET",
        "https://api.x.com/2/users/12/liked_tweets"
    ));
}

#[test]
fn a_write_clears_its_accounts_responses() {
    let (_tmp, r) = store();
    let bob = account("default", "bob");
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("alice");
    r.write("GET", TWEET, &bob, 200, &body()).expect("bob");
    r.invalidate(&alice()).expect("invalidate");
    assert!(!r.dir().join(account_key(&alice())).exists());
    assert!(r.read("GET", TWEET, &bob, Utc::now()).is_some());
    r.invalidate(&alice()).expect("a missing directory is fine");
}

#[test]
fn pruning_runs_once_a_day() {
    let (_tmp, r) = store();
    let bob = account("default", "bob");
    let stale = "https://api.x.com/2/tweets/1";
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("today");
    r.write("GET", stale, &alice(), 200, &body())
        .expect("stale");
    r.write("GET", stale, &bob, 200, &body())
        .expect("bob stale");
    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    set_mtime(&r.path_for("GET", stale, &alice()), yesterday);
    set_mtime(&r.path_for("GET", stale, &bob), yesterday);

    assert!(r.prune_if_due(now).expect("prune"));
    assert!(r.path_for("GET", TWEET, &alice()).exists());
    assert!(!r.path_for("GET", stale, &alice()).exists());
    assert!(
        !r.dir().join(account_key(&bob)).exists(),
        "the emptied account directory goes"
    );

    r.write("GET", stale, &alice(), 200, &body())
        .expect("again");
    set_mtime(&r.path_for("GET", stale, &alice()), yesterday);
    assert!(!r.prune_if_due(now).expect("second prune"));
    assert!(
        r.path_for("GET", stale, &alice()).exists(),
        "the second prune that day scans nothing"
    );
}

#[test]
fn clear_removes_only_bird_files() {
    let (_tmp, r) = store();
    r.write("GET", TWEET, &alice(), 200, &body())
        .expect("write");
    let size = std::fs::metadata(r.path_for("GET", TWEET, &alice()))
        .expect("stat")
        .len();
    std::fs::write(r.dir().join("notes.txt"), b"mine").expect("stray");
    let account_dir = r.dir().join(account_key(&alice()));
    std::fs::write(account_dir.join(".bird-abc123.tmp"), b"xy").expect("temp");

    let cleared = r.clear().expect("clear");
    assert_eq!(cleared.files, 2);
    assert_eq!(cleared.bytes, size + 2);
    assert_eq!(names(r.dir()), ["notes.txt"]);
}

#[test]
fn clearing_a_missing_directory_removes_nothing() {
    let (_tmp, r) = store();
    assert_eq!(r.clear().expect("clear"), Cleared::default());
    assert!(!r.prune_if_due(Utc::now()).expect("prune"));
}
