use std::path::Path;

use serde_json::{Value, json};

use super::note::{Kind, REGION_BEGIN, REGION_END};
use super::yaml::Field;
use super::*;

const API: &str = "https://api.x.com";

fn paths(notes: &[Note]) -> Vec<String> {
    notes
        .iter()
        .map(|n| n.path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn note_at<'a>(notes: &'a [Note], path: &str) -> &'a Note {
    notes
        .iter()
        .find(|n| n.path == Path::new(path))
        .unwrap_or_else(|| panic!("no note at {path} in {:?}", paths(notes)))
}

fn two_posts() -> Value {
    json!({
        "data": [
            {"id": "1849", "text": "hello rust", "author_id": "12"},
            {"id": "1850", "text": "hello world", "author_id": "13"}
        ],
        "includes": {"users": [
            {"id": "12", "username": "jack", "name": "Jack"},
            {"id": "13", "username": "ev", "name": "Ev"}
        ]}
    })
}

fn frontmatter(rendered: &str) -> Value {
    let inner = rendered
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("---\n"))
        .map(|(fm, _)| fm)
        .expect("frontmatter");
    serde_saphyr::from_str(inner).expect("frontmatter parses")
}

fn key_names(rendered: &str) -> Vec<String> {
    let inner = rendered
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("---\n"))
        .map(|(fm, _)| fm)
        .expect("frontmatter");
    inner
        .lines()
        .filter(|l| !l.starts_with(' '))
        .filter_map(|l| l.split_once(':').map(|(k, _)| k.to_string()))
        .collect()
}

#[test]
fn a_search_page_archives_posts_and_their_authors() {
    let (notes, bad) = plan(
        &format!("{API}/2/tweets/search/recent?query=rust"),
        &two_posts(),
        None,
    );
    assert!(bad.is_empty());
    let mut got = paths(&notes);
    got.sort();
    assert_eq!(
        got,
        [
            "posts/1849.md",
            "posts/1850.md",
            "users/12.md",
            "users/13.md"
        ]
    );
    assert_eq!(
        note_at(&notes, "posts/1849.md").get("author"),
        Some(&Field::Str("jack".into()))
    );
}

#[test]
fn a_bookmarks_page_adds_bookmark_notes_for_the_path_user() {
    let (notes, _) = plan(
        &format!("{API}/2/users/12/bookmarks?max_results=2"),
        &two_posts(),
        Some("jack"),
    );
    let got = paths(&notes);
    for path in [
        "posts/1849.md",
        "posts/1850.md",
        "bookmarks/12/1849.md",
        "bookmarks/12/1850.md",
    ] {
        assert!(
            got.contains(&path.to_string()),
            "{path} missing from {got:?}"
        );
    }
    let bookmark = note_at(&notes, "bookmarks/12/1849.md");
    assert_eq!(bookmark.kind, Kind::Bookmark);
    assert_eq!(bookmark.get("account"), Some(&Field::Str("jack".into())));
    assert!(
        bookmark.region.contains("(../../posts/1849.md)"),
        "{}",
        bookmark.region
    );
    assert!(
        bookmark.region.contains("(../../users/12.md)"),
        "{}",
        bookmark.region
    );
}

#[test]
fn a_liked_posts_page_files_posts_not_users() {
    let (notes, _) = plan(
        &format!("{API}/2/users/12/liked_tweets"),
        &two_posts(),
        None,
    );
    let got = paths(&notes);
    assert!(got.contains(&"posts/1849.md".to_string()), "{got:?}");
    assert!(got.contains(&"likes/12/1849.md".to_string()), "{got:?}");
    assert!(got.contains(&"likes/12/1850.md".to_string()), "{got:?}");
    assert!(
        !got.contains(&"users/1849.md".to_string()),
        "a post filed as a user: {got:?}"
    );
    assert!(
        !got.contains(&"users/1850.md".to_string()),
        "a post filed as a user: {got:?}"
    );
    assert_eq!(
        note_at(&notes, "likes/12/1849.md").get("type"),
        Some(&Field::Str("x-like".into()))
    );
}

#[test]
fn a_timeline_page_files_posts() {
    let (notes, _) = plan(
        &format!("{API}/2/users/12/timelines/reverse_chronological"),
        &two_posts(),
        None,
    );
    let got = paths(&notes);
    assert!(got.contains(&"posts/1849.md".to_string()), "{got:?}");
    assert!(!got.contains(&"users/1849.md".to_string()), "{got:?}");
}

#[test]
fn shapes_write_parameters_as_braces() {
    for (url, shape) in [
        (
            "/2/users/12/bookmarks?max_results=5",
            "/2/users/{}/bookmarks",
        ),
        ("/2/users/by/username/jack", "/2/users/by/username/{}"),
        ("/2/users/me", "/2/users/me"),
        ("/2/tweets/search/recent?query=1", "/2/tweets/search/recent"),
        ("/2/lists/77/members", "/2/lists/{}/members"),
    ] {
        assert_eq!(endpoint_shape(&format!("{API}{url}")), shape, "{url}");
    }
}

#[test]
fn an_unknown_shape_archives_nothing() {
    let (notes, bad) = plan(
        &format!("{API}/2/spaces/1"),
        &json!({"data": {"id": "1"}}),
        None,
    );
    assert!(notes.is_empty() && bad.is_empty());
}

#[test]
fn a_user_page_files_users() {
    let body = json!({"data": {"id": "12", "username": "jack"}});
    for url in [
        "/2/users/me",
        "/2/users/by/username/jack",
        "/2/users/12",
        "/2/users/12/followers",
    ] {
        let (notes, _) = plan(&format!("{API}{url}"), &body, None);
        assert_eq!(paths(&notes), ["users/12.md"], "{url}");
    }
}

#[test]
fn included_posts_get_their_own_notes() {
    let body = json!({
        "data": [{"id": "1849", "text": "look", "referenced_posts": [{"type": "quoted", "id": "1700"}]}],
        "includes": {"tweets": [{"id": "1700", "text": "original"}]}
    });
    let (notes, _) = plan(&format!("{API}/2/tweets/1849"), &body, None);
    let quoting = note_at(&notes, "posts/1849.md");
    assert_eq!(quoting.get("quotes"), Some(&Field::Str("1700".into())));
    assert!(
        quoting.region.contains("Quoting [post 1700](1700.md)"),
        "{}",
        quoting.region
    );
    assert!(note_at(&notes, "posts/1700.md").region.contains("original"));
}

#[test]
fn a_non_numeric_id_is_never_a_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let stderr: Arc<std::sync::Mutex<dyn std::io::Write + Send>> = buf.clone();
    let status = StoreStatus::new(false, stderr);
    let archive = Archive::new(dir.path().join("archive"), Arc::clone(&status));
    let body = json!({"data": [{"id": "../x", "text": "escape"}]});
    archive.observe(&format!("{API}/2/tweets"), &body, None, chrono::Utc::now());
    assert!(!dir.path().join("archive").exists());
    assert!(!dir.path().join("x").exists());
    let warned = String::from_utf8(buf.lock().expect("lock").clone()).expect("utf8");
    assert!(
        warned.starts_with("[store] warning: archive id check failed"),
        "{warned}"
    );
    assert!(warned.contains("../x"), "{warned}");
}

#[test]
fn legacy_and_current_metric_names_render_alike() {
    let current = json!({"data": {"id": "1", "public_metrics": {"repost_count": 3}}});
    let legacy = json!({"data": {"id": "1", "public_metrics": {"retweet_count": 3}}});
    for body in [current, legacy] {
        let (notes, _) = plan(&format!("{API}/2/tweets/1"), &body, None);
        assert_eq!(notes[0].get("repost_count"), Some(&Field::Int(3)));
    }
    let user = json!({"data": {"id": "2", "public_metrics": {"tweet_count": 9}}});
    let (notes, _) = plan(&format!("{API}/2/users/2"), &user, None);
    assert_eq!(notes[0].get("post_count"), Some(&Field::Int(9)));
}

#[test]
fn absent_fields_write_no_keys() {
    let (notes, _) = plan(
        &format!("{API}/2/tweets/1"),
        &json!({"data": {"id": "1", "text": "hi"}}),
        None,
    );
    let rendered = notes[0].render("2026-10-22T00:00:00Z");
    assert_eq!(
        key_names(&rendered),
        [
            "type",
            "id",
            "url",
            "archived_at",
            "updated_at",
            "bird_format"
        ]
    );
}

#[test]
fn ids_render_as_strings() {
    let body = json!({"data": {"id": "1849000000000000000", "author_id": "12", "conversation_id": "1849000000000000000"}});
    let (notes, _) = plan(&format!("{API}/2/tweets/1849000000000000000"), &body, None);
    let fm = frontmatter(&notes[0].render("2026-10-22T00:00:00Z"));
    assert_eq!(fm["id"], "1849000000000000000");
    assert_eq!(fm["author_id"], "12");
    assert_eq!(fm["conversation_id"], "1849000000000000000");
    assert_eq!(fm["bird_format"], 1);
}

#[test]
fn every_kind_writes_its_keys_in_order() {
    let post = json!({
        "data": {
            "id": "1849", "text": "t", "author_id": "12", "created_at": "2026-10-22T14:03:00.000Z",
            "conversation_id": "1800", "lang": "en",
            "referenced_posts": [
                {"type": "replied_to", "id": "1801"}, {"type": "quoted", "id": "1802"},
                {"type": "retweeted", "id": "1803"}
            ],
            "public_metrics": {"like_count": 1, "repost_count": 2, "reply_count": 3, "quote_count": 4,
                               "bookmark_count": 5, "impression_count": 6},
            "entities": {"hashtags": [{"tag": "rust"}], "mentions": [{"username": "ev"}],
                         "urls": [{"url": "https://t.co/a", "expanded_url": "https://e.com"}]},
            "attachments": {"media_keys": ["3_1"]}
        },
        "includes": {"users": [{"id": "12", "username": "jack"}],
                     "media": [{"media_key": "3_1", "type": "photo", "url": "https://pbs.twimg.com/a.jpg"}]}
    });
    let (notes, _) = plan(&format!("{API}/2/tweets/1849"), &post, None);
    let rendered = note_at(&notes, "posts/1849.md").render("2026-10-22T00:00:00Z");
    assert_eq!(key_names(&rendered), Kind::Post.keys());

    let user = json!({"data": {
        "id": "12", "username": "jack", "name": "Jack", "description": "bio", "location": "SF",
        "entities": {"url": {"urls": [{"expanded_url": "https://jack.com"}]}},
        "created_at": "2006-03-21T20:50:14.000Z", "verified": true, "protected": false,
        "profile_image_url": "https://pbs.twimg.com/p.jpg",
        "public_metrics": {"followers_count": 1, "following_count": 2, "tweet_count": 3,
                           "listed_count": 4, "like_count": 5, "media_count": 6},
        "pinned_tweet_id": "20"
    }});
    let (notes, _) = plan(&format!("{API}/2/users/12"), &user, None);
    let rendered = notes[0].render("2026-10-22T00:00:00Z");
    assert_eq!(key_names(&rendered), Kind::User.keys());
    assert_eq!(frontmatter(&rendered)["aliases"], json!(["@jack"]));

    let (notes, _) = plan(
        &format!("{API}/2/users/12/bookmarks"),
        &two_posts(),
        Some("jack"),
    );
    let rendered = note_at(&notes, "bookmarks/12/1849.md").render("2026-10-22T00:00:00Z");
    assert_eq!(key_names(&rendered), Kind::Bookmark.keys());
}

#[test]
fn post_and_user_urls_use_the_id_forms() {
    for body in [
        json!({"data": {"id": "1849", "author_id": "12"}, "includes": {"users": [{"id": "12", "username": "jack"}]}}),
        json!({"data": {"id": "1849"}}),
    ] {
        let (notes, _) = plan(&format!("{API}/2/tweets/1849"), &body, None);
        let post = note_at(&notes, "posts/1849.md");
        assert_eq!(
            post.get("url"),
            Some(&Field::Str("https://x.com/i/status/1849".into()))
        );
    }
    let (notes, _) = plan(
        &format!("{API}/2/users/12"),
        &json!({"data": {"id": "12", "username": "jack"}}),
        None,
    );
    assert_eq!(
        notes[0].get("url"),
        Some(&Field::Str("https://x.com/i/user/12".into()))
    );
}

#[test]
fn media_render_as_links() {
    let body = json!({
        "data": {"id": "1", "text": "pic", "attachments": {"media_keys": ["3_1", "7_2"]}},
        "includes": {"media": [
            {"media_key": "3_1", "type": "photo", "url": "https://pbs.twimg.com/a.jpg"},
            {"media_key": "7_2", "type": "video", "preview_image_url": "https://pbs.twimg.com/v.jpg"}
        ]}
    });
    let (notes, _) = plan(&format!("{API}/2/tweets/1"), &body, None);
    let post = &notes[0];
    assert_eq!(
        post.get("media"),
        Some(&Field::List(vec![
            "https://pbs.twimg.com/a.jpg".into(),
            "https://pbs.twimg.com/v.jpg".into()
        ]))
    );
    assert!(
        post.region
            .contains("\n[photo](https://pbs.twimg.com/a.jpg)"),
        "{}",
        post.region
    );
    assert!(
        post.region
            .contains("\n[video](https://pbs.twimg.com/v.jpg)"),
        "{}",
        post.region
    );
    assert!(!post.region.contains("!["), "an embed: {}", post.region);
}

#[test]
fn bios_and_names_are_escaped() {
    let body = json!({"data": {"id": "12", "username": "jack", "name": "```dataviewjs",
                               "description": "<script>alert(1)</script>"}});
    let (notes, _) = plan(&format!("{API}/2/users/12"), &body, None);
    let region = &notes[0].region;
    assert!(region.contains("\\`\\`\\`dataviewjs"), "{region}");
    assert!(region.contains("\\<script>"), "{region}");
    assert!(!region.contains("\n```"), "{region}");
}

#[test]
fn a_post_region_links_its_author_and_time() {
    let body = json!({
        "data": {"id": "1849", "text": "a &amp; b", "author_id": "12", "created_at": "2026-10-22T14:03:00.000Z"},
        "includes": {"users": [{"id": "12", "username": "jack"}]}
    });
    let (notes, _) = plan(&format!("{API}/2/tweets/1849"), &body, None);
    let rendered = note_at(&notes, "posts/1849.md").render("2026-10-22T00:00:00Z");
    let body_part = rendered
        .split_once("\n---\n")
        .map(|(_, b)| b)
        .expect("body");
    assert_eq!(
        body_part,
        format!(
            "{REGION_BEGIN}\na & b\n\n[@jack](../users/12.md) · 2026-10-22 14:03 UTC · [x.com](https://x.com/i/status/1849)\n{REGION_END}\n"
        )
    );
}

#[test]
fn observe_writes_new_notes_under_the_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = Archive::new(dir.path().to_path_buf(), StoreStatus::silent());
    archive.observe(
        &format!("{API}/2/tweets/search/recent"),
        &two_posts(),
        None,
        chrono::Utc::now(),
    );
    for path in [
        "posts/1849.md",
        "posts/1850.md",
        "users/12.md",
        "users/13.md",
    ] {
        let text = std::fs::read_to_string(dir.path().join(path)).expect("note written");
        assert!(text.starts_with("---\ntype: "), "{text}");
    }
    assert_eq!(archive.root(), Path::new(dir.path()));
}
