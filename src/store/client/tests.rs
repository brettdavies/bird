use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::*;
use crate::store::ledger::Record;
use crate::xurl_client::mock::MockXurlClient;

const POST: &str = "https://api.x.com/2/tweets/1849?tweet.fields=public_metrics";
const BOOKMARKS: &str = "https://api.x.com/2/users/42/bookmarks?max_results=10";

fn oauth2() -> RequestContext<'static> {
    RequestContext {
        auth_type: &AuthType::OAuth2User,
        username: None,
    }
}

fn setup(root: &Path) -> StoreSetup {
    let mut setup = StoreSetup::under(root);
    setup.account = StoreAccount {
        app: "default".into(),
        default_user: Some("alice".into()),
    };
    setup
}

fn client(root: &Path, mock: &MockXurlClient, opts: CacheOpts) -> BirdClient {
    BirdClient::new(
        Box::new(mock.clone_handle()),
        setup(root),
        opts,
        None,
        true,
        Arc::new(Mutex::new(std::io::sink())),
    )
}

fn post(likes: u64) -> serde_json::Value {
    serde_json::json!({"data": {
        "id": "1849", "text": "hello", "author_id": "12",
        "public_metrics": {"like_count": likes}
    }})
}

fn sent(mock: &MockXurlClient) -> usize {
    mock.calls()
        .iter()
        .filter(|c| c.method == "send_request")
        .count()
}

fn records(root: &Path) -> Vec<Record> {
    let paths = StorePaths::under(root);
    crate::store::ledger::Ledger::new(&paths)
        .read()
        .expect("ledger")
        .records
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_repeat_read_the_same_day_sends_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    let mut c = client(dir.path(), &mock, CacheOpts::default());
    let first = c.get(POST, &oauth2()).expect("first");
    let second = c.get(POST, &oauth2()).expect("second");
    assert_eq!(sent(&mock), 1);
    assert!(!first.cache_hit);
    assert!(second.cache_hit);
    assert_eq!(first.json, second.json);
}

#[test]
fn another_account_is_sent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    mock.push_value("send_request", post(5));
    client(dir.path(), &mock, CacheOpts::default())
        .get(POST, &oauth2())
        .expect("alice");
    let mut bob = client(dir.path(), &mock, CacheOpts::default());
    bob.username = Some("bob".into());
    bob.get(POST, &oauth2()).expect("bob");
    assert_eq!(sent(&mock), 2);
}

#[test]
fn a_user_lookup_is_not_answered_from_a_search() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value(
        "send_request",
        serde_json::json!({
            "data": [{"id": "1849", "text": "hi", "author_id": "12"}],
            "includes": {"users": [{"id": "12", "username": "jack", "name": "Jack"}]}
        }),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {"id": "12", "username": "jack", "name": "Jack"}}),
    );
    let mut c = client(dir.path(), &mock, CacheOpts::default());
    c.get(
        "https://api.x.com/2/tweets/search/recent?query=rust",
        &oauth2(),
    )
    .expect("search");
    let lookup = c
        .get("https://api.x.com/2/users/by/username/jack", &oauth2())
        .expect("lookup");
    assert_eq!(sent(&mock), 2);
    assert!(!lookup.cache_hit);
}

#[test]
fn refresh_sends_and_rewrites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    mock.push_value("send_request", post(9));
    client(dir.path(), &mock, CacheOpts::default())
        .get(POST, &oauth2())
        .expect("first");
    let note = dir.path().join("archive").join("posts").join("1849.md");
    assert!(
        std::fs::read_to_string(&note)
            .expect("note")
            .contains("like_count: 5")
    );
    let refresh = CacheOpts {
        refresh: true,
        ..CacheOpts::default()
    };
    let again = client(dir.path(), &mock, refresh)
        .get(POST, &oauth2())
        .expect("refresh");
    assert_eq!(sent(&mock), 2);
    assert!(!again.cache_hit);
    assert!(
        std::fs::read_to_string(&note)
            .expect("note")
            .contains("like_count: 9")
    );
}

#[test]
fn no_cache_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    let no_store = CacheOpts {
        no_store: true,
        ..CacheOpts::default()
    };
    client(dir.path(), &mock, no_store)
        .get(POST, &oauth2())
        .expect("get");
    assert_eq!(files_under(dir.path()), Vec::<PathBuf>::new());
}

#[test]
fn cache_only_on_an_empty_store_is_a_cache_miss() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    let cache_only = CacheOpts {
        cache_only: true,
        ..CacheOpts::default()
    };
    let err = client(dir.path(), &mock, cache_only)
        .get(POST, &oauth2())
        .expect_err("nothing stored");
    assert!(err.is::<crate::error::CacheMiss>(), "{err}");
    assert_eq!(sent(&mock), 0);
}

#[test]
fn cache_only_answers_a_stored_response() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    client(dir.path(), &mock, CacheOpts::default())
        .get(POST, &oauth2())
        .expect("first");
    let cache_only = CacheOpts {
        cache_only: true,
        refresh: true,
        ..CacheOpts::default()
    };
    let hit = client(dir.path(), &mock, cache_only)
        .get(POST, &oauth2())
        .expect("answered");
    assert!(hit.cache_hit);
    assert_eq!(sent(&mock), 1);
}

#[test]
fn a_write_clears_the_accounts_stored_pages() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", serde_json::json!({"data": []}));
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {"bookmarked": true}}),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": [{"id": "1849"}]}),
    );
    let mut c = client(dir.path(), &mock, CacheOpts::default());
    c.get(BOOKMARKS, &oauth2()).expect("page");
    c.request(
        "POST",
        "https://api.x.com/2/users/42/bookmarks",
        &oauth2(),
        Some(r#"{"tweet_id":"1849"}"#),
    )
    .expect("bookmark");
    let page = c.get(BOOKMARKS, &oauth2()).expect("page again");
    assert_eq!(sent(&mock), 3);
    assert!(!page.cache_hit);
}

#[test]
fn a_no_cache_write_still_clears_stored_responses() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", serde_json::json!({"data": []}));
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {"bookmarked": true}}),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": [{"id": "1849"}]}),
    );
    client(dir.path(), &mock, CacheOpts::default())
        .get(BOOKMARKS, &oauth2())
        .expect("page");
    let no_store = CacheOpts {
        no_store: true,
        ..CacheOpts::default()
    };
    client(dir.path(), &mock, no_store)
        .request(
            "POST",
            "https://api.x.com/2/users/42/bookmarks",
            &oauth2(),
            Some(r#"{"tweet_id":"1849"}"#),
        )
        .expect("bookmark");
    let page = client(dir.path(), &mock, CacheOpts::default())
        .get(BOOKMARKS, &oauth2())
        .expect("page again");
    assert_eq!(sent(&mock), 3);
    assert!(!page.cache_hit);
}

#[test]
fn a_cache_hit_touches_no_note() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    let mut c = client(dir.path(), &mock, CacheOpts::default());
    c.get(POST, &oauth2()).expect("first");
    let notes = files_under(&dir.path().join("archive"));
    assert!(!notes.is_empty());
    let stamp = |paths: &[PathBuf]| -> Vec<std::time::SystemTime> {
        paths
            .iter()
            .map(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .expect("mtime")
            })
            .collect()
    };
    let before = stamp(&notes);
    c.get(POST, &oauth2()).expect("second");
    assert_eq!(stamp(&notes), before);
    let hits: Vec<bool> = records(dir.path())
        .iter()
        .map(|r| matches!(r, Record::Request(q) if q.cache_hit))
        .collect();
    assert_eq!(hits, [false, true]);
}

#[test]
fn a_dm_read_reaches_only_the_ledger() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value(
        "send_request",
        serde_json::json!({"data": [{"id": "9", "text": "secret", "event_type": "MessageCreate"}]}),
    );
    client(dir.path(), &mock, CacheOpts::default())
        .get("https://api.x.com/2/dm_events", &oauth2())
        .expect("dm");
    assert_eq!(
        files_under(&dir.path().join("responses")),
        Vec::<PathBuf>::new()
    );
    assert_eq!(
        files_under(&dir.path().join("archive")),
        Vec::<PathBuf>::new()
    );
    assert_eq!(records(dir.path()).len(), 1);
}

#[test]
fn every_request_is_priced_into_the_ledger_with_its_account() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    client(dir.path(), &mock, CacheOpts::default())
        .get(POST, &oauth2())
        .expect("get");
    let records = records(dir.path());
    let [Record::Request(r)] = records.as_slice() else {
        panic!("one request record, got {records:?}");
    };
    assert_eq!(r.method, "GET");
    assert_eq!(r.endpoint, "/2/tweets/:id");
    assert_eq!(r.app, "default");
    assert!(!r.cache_hit);
}

/// The ledger prices the caller's own bookmarks at the owned-read rate once
/// a `/2/users/me` response has named the caller.
#[test]
fn ledger_prices_own_bookmarks_as_owned_reads() {
    let mock = MockXurlClient::new();
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {"id": "42", "username": "me"}}),
    );
    let page: Vec<_> = (0..10)
        .map(|i| serde_json::json!({"id": format!("10{i}"), "text": "t"}))
        .collect();
    mock.push_value("send_request", serde_json::json!({ "data": page }));
    let mut client = BirdClient::new_test(Box::new(mock));
    client
        .get("https://api.x.com/2/users/me", &oauth2())
        .expect("me");
    client.get(BOOKMARKS, &oauth2()).expect("bookmarks");

    let records = client.store().ledger().read().expect("ledger").records;
    let summary = crate::store::spend_view::summary(&records, 0);
    assert_eq!(summary.total_calls, 2);
    assert!(
        (summary.total_cost - 0.020).abs() < 1e-9,
        "one user read plus ten owned reads, got ${}",
        summary.total_cost
    );
}

#[cfg(unix)]
#[test]
fn a_read_only_archive_keeps_the_commands_output() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("archive");
    std::fs::create_dir_all(&archive).expect("archive");
    std::fs::set_permissions(&archive, std::fs::Permissions::from_mode(0o500)).expect("chmod");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stderr: Arc<Mutex<dyn Write + Send>> = buf.clone();
    let mut c = BirdClient::new(
        Box::new(mock.clone_handle()),
        setup(dir.path()),
        CacheOpts::default(),
        None,
        false,
        stderr,
    );
    let response = c.get(POST, &oauth2()).expect("the read succeeds");
    assert_eq!(response.json, Some(post(5)));
    let text = String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8");
    assert_eq!(text.matches("[store] warning").count(), 1, "{text}");
    std::fs::set_permissions(&archive, std::fs::Permissions::from_mode(0o700)).expect("chmod");
}

/// A panic elsewhere that poisoned the shared writer must not turn a store
/// warning into a second panic.
#[test]
fn a_store_warning_survives_a_poisoned_writer() {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let poisoner = Arc::clone(&buf);
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.lock();
        panic!("poison the shared writer");
    })
    .join();
    assert!(buf.is_poisoned());

    let dir = tempfile::tempdir().expect("tempdir");
    let blocker = dir.path().join("not-a-dir");
    std::fs::write(&blocker, b"x").expect("file");
    let mock = MockXurlClient::new();
    mock.push_value("send_request", post(5));
    let stderr: Arc<Mutex<dyn Write + Send>> = buf.clone();
    let mut c = BirdClient::new(
        Box::new(mock.clone_handle()),
        setup(&blocker),
        CacheOpts::default(),
        None,
        false,
        stderr,
    );
    c.get(POST, &oauth2()).expect("the read succeeds");
    let written = buf
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let text = String::from_utf8_lossy(&written);
    assert!(
        text.contains("[store] warning"),
        "warning not written: {text:?}"
    );
}

#[test]
fn api_response_debug_redacts_body() {
    let response = ApiResponse {
        status: 200,
        cached_body: Some("sensitive data here".to_string()),
        cache_hit: true,
        json: None,
    };
    let debug = format!("{:?}", response);
    assert!(!debug.contains("sensitive data here"));
    assert!(debug.contains("body_len"));
}

#[cfg(not(debug_assertions))]
#[test]
fn empty_body_fallback_returns_empty_string() {
    let resp = ApiResponse {
        status: 200,
        cached_body: None,
        cache_hit: false,
        json: None,
    };
    assert_eq!(resp.body(), "");
    assert_eq!(resp.into_body(), "");
}

#[cfg(debug_assertions)]
#[test]
fn empty_body_triggers_debug_assert() {
    let result = std::panic::catch_unwind(|| {
        let resp = ApiResponse {
            status: 200,
            cached_body: None,
            cache_hit: false,
            json: None,
        };
        let _ = resp.body();
    });
    assert!(
        result.is_err(),
        "body() must debug_assert when no body is present"
    );
}

#[test]
fn cache_opts_default() {
    let opts = CacheOpts::default();
    assert!(!opts.no_store);
    assert!(!opts.refresh);
    assert!(!opts.cache_only);
}
