//! bird's search, bookmarks, and profile commands on scripted responses write
//! an archive whose every note is well formed: its path follows the id
//! layout, its frontmatter parses, and its region renders as plain text.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pulldown_cmark::{Event, Parser, Tag};

use super::{BirdClient, CacheOpts, StoreSetup};
use crate::cli::dispatch::ListFlags;
use crate::output::{OutputConfig, OutputFormat};
use crate::store::StoreAccount;
use crate::xurl_client::mock::MockXurlClient;

fn scripted_client(root: &Path, mock: &MockXurlClient) -> BirdClient {
    let mut setup = StoreSetup::under(root);
    setup.account = StoreAccount {
        app: "default".into(),
        default_user: Some("alice".into()),
    };
    BirdClient::new(
        Box::new(mock.clone_handle()),
        setup,
        CacheOpts::default(),
        None,
        true,
        Arc::new(Mutex::new(std::io::sink())),
    )
}

fn json_out() -> OutputConfig {
    OutputConfig {
        format: OutputFormat::Json,
        use_color: false,
        quiet: true,
        raw: true,
    }
}

fn notes(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read archive") {
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

fn is_id(segment: &str) -> bool {
    (1..=20).contains(&segment.len()) && segment.bytes().all(|b| b.is_ascii_digit())
}

/// The path under the archive matches `posts/<id>.md`, `users/<id>.md`,
/// `bookmarks/<id>/<id>.md`, or `likes/<id>/<id>.md`.
fn follows_the_id_layout(relative: &Path) -> bool {
    let parts: Vec<String> = relative
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let note = |name: &str| name.strip_suffix(".md").is_some_and(is_id);
    match parts.as_slice() {
        [kind, name] => (kind == "posts" || kind == "users") && note(name),
        [kind, account, name] => {
            (kind == "bookmarks" || kind == "likes") && is_id(account) && note(name)
        }
        _ => false,
    }
}

fn frontmatter_and_region(text: &str) -> (&str, &str) {
    let rest = text
        .strip_prefix("---\n")
        .expect("frontmatter opens the note");
    let end = rest.find("\n---\n").expect("frontmatter closes");
    let body = &rest[end + 5..];
    let begin = body.find("<!-- bird:begin -->").expect("region begins");
    let close = body.find("<!-- bird:end -->").expect("region ends");
    (
        &rest[..end],
        &body[begin + "<!-- bird:begin -->".len()..close],
    )
}

#[test]
fn a_scripted_session_writes_a_valid_archive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mock = MockXurlClient::new();
    mock.push_value(
        "send_request",
        serde_json::json!({
            "data": [
                {"id": "1849000000000000000", "text": "rust # not a heading **bold** [x](y)",
                 "author_id": "12", "created_at": "2026-10-22T12:00:00.000Z",
                 "public_metrics": {"like_count": 3, "retweet_count": 1},
                 "referenced_tweets": [{"type": "quoted", "id": "1848"}]},
                {"id": "1850", "text": "<script>alert(1)</script> | a | b |", "author_id": "13"}
            ],
            "includes": {
                "users": [
                    {"id": "12", "username": "jack", "name": "Jack"},
                    {"id": "13", "username": "jill", "name": "```dataviewjs"}
                ],
                "tweets": [{"id": "1848", "text": "the quoted post", "author_id": "13"}]
            },
            "meta": {"result_count": 2}
        }),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {"id": "42", "username": "alice", "name": "Alice"}}),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": [{"id": "1850", "text": "bookmarked", "author_id": "13"}]}),
    );
    mock.push_value(
        "send_request",
        serde_json::json!({"data": {
            "id": "12", "username": "jack", "name": "Jack", "description": "bio with [[link]]",
            "public_metrics": {"followers_count": 10, "tweet_count": 4}
        }}),
    );

    let mut client = scripted_client(dir.path(), &mock);
    let out = json_out();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let flags = ListFlags {
        limit: None,
        cursor: None,
    };
    crate::cli::commands::search::run(
        &mut client,
        &out,
        &mut stdout,
        &mut stderr,
        "rust".into(),
        false,
        "recent".into(),
        None,
        None,
        None,
        &flags,
    )
    .expect("search");
    crate::cli::commands::bookmarks::run(
        &mut client,
        &out,
        &mut stdout,
        &mut stderr,
        false,
        &flags,
    )
    .expect("bookmarks");
    crate::cli::commands::profile::run(
        &mut client,
        &out,
        &mut stdout,
        &mut stderr,
        "jack".into(),
        false,
    )
    .expect("profile");

    let archive = dir.path().join("archive");
    let notes = notes(&archive);
    let relative: Vec<PathBuf> = notes
        .iter()
        .map(|p| {
            p.strip_prefix(&archive)
                .expect("under archive")
                .to_path_buf()
        })
        .collect();
    for expected in [
        "posts/1849000000000000000.md",
        "posts/1850.md",
        "posts/1848.md",
        "users/12.md",
        "users/13.md",
        "users/42.md",
        "bookmarks/42/1850.md",
    ] {
        assert!(
            relative.contains(&PathBuf::from(expected)),
            "{expected} missing from {relative:?}"
        );
    }
    for (path, rel) in notes.iter().zip(&relative) {
        assert!(
            follows_the_id_layout(rel),
            "{} breaks the layout",
            rel.display()
        );
        let text = std::fs::read_to_string(path).expect("note");
        let (frontmatter, region) = frontmatter_and_region(&text);
        let parsed: serde_json::Value =
            serde_saphyr::from_str(frontmatter).expect("frontmatter parses");
        assert!(parsed.get("type").is_some(), "{}: {parsed}", rel.display());
        for event in Parser::new(region) {
            assert!(
                !matches!(
                    event,
                    Event::Html(_)
                        | Event::InlineHtml(_)
                        | Event::Code(_)
                        | Event::Start(Tag::CodeBlock(_))
                        | Event::Start(Tag::Table(_))
                ),
                "{} region renders {event:?}",
                rel.display()
            );
        }
    }
}
