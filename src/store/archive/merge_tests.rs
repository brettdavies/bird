use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::*;

const SEARCH: &str = "https://api.x.com/2/tweets/1849";

fn at(hour: u32) -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    chrono::Utc
        .with_ymd_and_hms(2026, 10, 22, hour, 0, 0)
        .single()
        .expect("valid time")
}

fn post(likes: u64, text: &str) -> Value {
    json!({
        "data": {"id": "1849", "text": text, "author_id": "12",
                 "public_metrics": {"like_count": likes, "repost_count": 1}},
        "includes": {"users": [{"id": "12", "username": "jack"}]}
    })
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: Archive,
    warnings: Arc<Mutex<Vec<u8>>>,
    root: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("archive");
    let warnings = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stderr: Arc<Mutex<dyn std::io::Write + Send>> = warnings.clone();
    let archive = Archive::new(root.clone(), StoreStatus::new(false, stderr));
    Fixture {
        _dir: dir,
        archive,
        warnings,
        root,
    }
}

impl Fixture {
    fn note(&self) -> PathBuf {
        self.root.join("posts").join("1849.md")
    }

    fn read(&self, path: &Path) -> String {
        std::fs::read_to_string(path).expect("read note")
    }

    fn warned(&self) -> String {
        String::from_utf8(self.warnings.lock().expect("lock").clone()).expect("utf8")
    }
}

/// Inserts `extra` frontmatter lines before the closing delimiter and appends
/// `tail` after the body, as a user editing the note would.
fn annotate(text: &str, extra: &str, tail: &str) -> String {
    let (fm, body) = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---\n"))
        .expect("frontmatter");
    format!("---\n{fm}\n{extra}---\n{body}{tail}")
}

#[test]
fn user_keys_and_text_survive_a_metrics_update() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let first = f.read(&f.note());
    let annotated = annotate(
        &first,
        "tags:\n  - ai\n# my comment line\n",
        "\nMy own paragraph about this post.\n",
    );
    std::fs::write(f.note(), &annotated).expect("user edit");

    f.archive.observe(SEARCH, &post(9, "hello"), None, at(2));
    let after = f.read(&f.note());
    let expected = annotated
        .replace("like_count: 5\n", "like_count: 9\n")
        .replace(
            "updated_at: \"2026-10-22T01:00:00Z\"",
            "updated_at: \"2026-10-22T02:00:00Z\"",
        );
    assert_eq!(after, expected);
    assert!(f.warned().is_empty(), "{}", f.warned());
}

fn mtime(path: &Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .expect("mtime")
}

/// Sets `path`'s mtime an hour back, so a rewrite would visibly move it.
fn age(path: &Path) {
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open");
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    file.set_modified(past).expect("set mtime");
}

#[test]
fn an_unchanged_observation_writes_nothing() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    age(&f.note());
    let (bytes, modified) = (f.read(&f.note()), mtime(&f.note()));
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(2));
    assert_eq!(f.read(&f.note()), bytes);
    assert_eq!(mtime(&f.note()), modified);
}

#[test]
fn editor_reformatting_alone_causes_no_write() {
    let f = fixture();
    let body = json!({
        "data": {"id": "1849", "text": "hi", "entities": {"hashtags": [{"tag": "rust"}, {"tag": "ai"}]}}
    });
    f.archive.observe(SEARCH, &body, None, at(1));
    let reformatted = f
        .read(&f.note())
        .replace("type: \"x-post\"", "type: x-post")
        .replace("id: \"1849\"", "id: '1849'")
        .replace(
            "url: \"https://x.com/i/status/1849\"",
            "url: https://x.com/i/status/1849",
        )
        .replace(
            "hashtags:\n  - \"rust\"\n  - \"ai\"\n",
            "hashtags: [rust, ai]\n",
        );
    assert!(
        reformatted.contains("hashtags: [rust, ai]"),
        "{reformatted}"
    );
    std::fs::write(f.note(), &reformatted).expect("editor save");
    age(&f.note());
    let modified = mtime(&f.note());
    f.archive.observe(SEARCH, &body, None, at(2));
    assert_eq!(f.read(&f.note()), reformatted);
    assert_eq!(mtime(&f.note()), modified);
}

#[test]
fn archived_at_survives_every_rewrite() {
    let f = fixture();
    for (hour, likes) in [(1, 1), (2, 2), (3, 3), (4, 4)] {
        f.archive
            .observe(SEARCH, &post(likes, "hello"), None, at(hour));
        let fm: Value = frontmatter(&f.read(&f.note()));
        assert_eq!(fm["archived_at"], "2026-10-22T01:00:00Z");
        assert_eq!(fm["updated_at"], format!("2026-10-22T{hour:02}:00:00Z"));
        assert_eq!(fm["like_count"], likes);
    }
}

fn frontmatter(text: &str) -> Value {
    let fm = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---\n"))
        .map(|(fm, _)| fm)
        .expect("frontmatter");
    serde_saphyr::from_str(fm).expect("parses")
}

#[test]
fn a_thinner_observation_keeps_stored_keys() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let thin = json!({"data": {"id": "1849", "text": "hello, edited"}});
    f.archive.observe(SEARCH, &thin, None, at(2));
    let text = f.read(&f.note());
    let fm = frontmatter(&text);
    assert_eq!(fm["like_count"], 5);
    assert_eq!(fm["repost_count"], 1);
    assert_eq!(fm["author"], "jack");
    assert!(text.contains("hello, edited"), "{text}");
}

#[test]
fn aliases_are_merged() {
    let f = fixture();
    let url = "https://api.x.com/2/users/12";
    f.archive.observe(
        url,
        &json!({"data": {"id": "12", "username": "jack"}}),
        None,
        at(1),
    );
    let path = f.root.join("users").join("12.md");
    let edited = f
        .read(&path)
        .replace("aliases:\n  - \"@jack\"\n", "aliases: [\"Jack D\"]\n");
    std::fs::write(&path, &edited).expect("user edit");
    f.archive.observe(
        url,
        &json!({"data": {"id": "12", "username": "jack2"}}),
        None,
        at(2),
    );
    let fm = frontmatter(&f.read(&path));
    assert_eq!(fm["aliases"], json!(["@jack2", "@jack", "Jack D"]));
    assert_eq!(fm["username"], "jack2");
}

#[test]
fn a_missing_region_is_restored_above_the_users_text() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let text = f.read(&f.note());
    let (fm, _) = text.split_once("\n---\n").expect("split");
    std::fs::write(f.note(), format!("{fm}\n---\nMy notes only.\n")).expect("user edit");
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(2));
    let after = f.read(&f.note());
    let body = after.split_once("\n---\n").map(|(_, b)| b).expect("body");
    assert!(body.starts_with("<!-- bird:begin -->\nhello\n"), "{body}");
    assert!(
        body.ends_with("<!-- bird:end -->\nMy notes only.\n"),
        "{body}"
    );
}

#[test]
fn an_unparseable_frontmatter_is_left_alone() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let broken = annotate(&f.read(&f.note()), "tags: [a\n", "");
    std::fs::write(f.note(), &broken).expect("user edit");
    f.archive.observe(SEARCH, &post(9, "hello"), None, at(2));
    assert_eq!(f.read(&f.note()), broken);
    assert!(
        f.warned()
            .starts_with("[store] warning: archive merge failed for "),
        "{}",
        f.warned()
    );
}

#[test]
fn a_frontmatter_whose_lines_disagree_with_its_parse_is_left_alone() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let odd = annotate(&f.read(&f.note()), "? complex\n: key\n", "");
    std::fs::write(f.note(), &odd).expect("user edit");
    f.archive.observe(SEARCH, &post(9, "hello"), None, at(2));
    assert_eq!(f.read(&f.note()), odd);
    assert!(
        f.warned().contains("archive merge failed"),
        "{}",
        f.warned()
    );
}

#[test]
fn a_save_during_the_merge_is_not_lost() {
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    let saved = annotate(&f.read(&f.note()), "", "\nTyped while bird merged.\n");
    let (notes, _) = plan(SEARCH, &post(9, "hello"), None);
    let note = notes
        .iter()
        .find(|n| n.kind == note::Kind::Post)
        .expect("post note");
    let result = merge::write_note_with(&f.note(), note, "2026-10-22T02:00:00Z", || {
        std::fs::write(f.note(), &saved).expect("user save");
    });
    assert!(
        matches!(result, Err(merge::MergeError::Changed)),
        "{result:?}"
    );
    assert_eq!(f.read(&f.note()), saved);
    let names: Vec<String> = std::fs::read_dir(f.root.join("posts"))
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["1849.md"], "no temporary file is left");

    f.archive.observe(SEARCH, &post(9, "hello"), None, at(3));
    let merged = f.read(&f.note());
    assert!(merged.contains("like_count: 9"), "{merged}");
    assert!(merged.ends_with("Typed while bird merged.\n"), "{merged}");
}

#[test]
fn a_relation_note_is_written_once() {
    let f = fixture();
    let url = "https://api.x.com/2/users/12/bookmarks";
    f.archive
        .observe(url, &post(5, "hello"), Some("jack"), at(1));
    let path = f.root.join("bookmarks").join("12").join("1849.md");
    age(&path);
    let (bytes, modified) = (f.read(&path), mtime(&path));
    f.archive
        .observe(url, &post(9, "hello"), Some("jack2"), at(2));
    assert_eq!(f.read(&path), bytes);
    assert_eq!(mtime(&path), modified);
    assert!(f.read(&f.note()).contains("like_count: 9"));
}

#[cfg(unix)]
#[test]
fn a_rewritten_note_stays_private() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    f.archive.observe(SEARCH, &post(5, "hello"), None, at(1));
    f.archive.observe(SEARCH, &post(9, "hello"), None, at(2));
    let mode = std::fs::metadata(f.note())
        .expect("stat")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}
