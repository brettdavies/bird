use serde_json::{Value, json};

use super::merge_tests::{age, at, fixture, frontmatter, mtime};
use super::note::Kind;
use super::*;

const POST: &str = "https://api.x.com/2/tweets/1849";
const STAMP: &str = "2026-10-22T00:00:00Z";

/// Post 1849 by jack at 14:03 UTC on 2026-10-22, with `fields` over those.
fn jacks_post(fields: Value) -> Value {
    let mut post =
        json!({"id": "1849", "author_id": "12", "created_at": "2026-10-22T14:03:00.000Z"});
    if let (Some(post), Value::Object(fields)) = (post.as_object_mut(), fields) {
        post.extend(fields);
    }
    json!({"data": post, "includes": {"users": [{"id": "12", "username": "jack"}]}})
}

fn post_note(body: &Value) -> Note {
    let (notes, _) = plan(POST, body, None);
    notes
        .into_iter()
        .find(|n| n.kind == Kind::Post)
        .expect("a post note")
}

/// The `aliases` of the note a first sighting of `body` writes.
fn aliases(body: &Value) -> Value {
    frontmatter(&post_note(body).render(STAMP))["aliases"].clone()
}

/// A 300-character post: three sentences and a short last word.
fn long_text() -> String {
    let sentence = "Rust's ownership model turns concurrency bugs into compile errors instead of production incidents. ";
    let text = format!("{}Go!", sentence.repeat(3));
    assert_eq!(text.chars().count(), 300);
    text
}

const LONG_ALIAS: &str =
    "@jack 2026-10-22: Rust's ownership model turns concurrency bugs into compile…";

#[test]
fn a_post_is_named_by_author_date_and_text() {
    let body = jacks_post(json!({"text": "hello rust"}));
    assert_eq!(aliases(&body), json!(["@jack 2026-10-22: hello rust"]));
    let rendered = post_note(&body).render(STAMP);
    assert!(
        rendered.contains("\naliases:\n  - \"@jack 2026-10-22: hello rust\"\narchived_at: "),
        "{rendered}"
    );

    let late = jacks_post(json!({"text": "hi", "created_at": "2026-10-22T23:30:00-05:00"}));
    assert_eq!(aliases(&late), json!(["@jack 2026-10-23: hi"]));
}

#[test]
fn link_syntax_links_and_line_breaks_leave_the_text() {
    let text = "Big news\n\n#rust [[x]]  a|b ^id\tQ&amp;A https://t.co/AbC123\nend ";
    assert_eq!(
        aliases(&jacks_post(json!({"text": text}))),
        json!(["@jack 2026-10-22: Big news rust x ab id Q&A end"])
    );
}

#[test]
fn a_cut_never_splits_a_grapheme() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let emoji = format!("{}{family}tail", "a".repeat(57));
    assert_eq!(
        aliases(&jacks_post(json!({"text": emoji}))),
        json!([format!("@jack 2026-10-22: {}…", "a".repeat(57))])
    );

    let accent = format!("{}e\u{301}x", "b".repeat(59));
    assert_eq!(
        aliases(&jacks_post(json!({"text": accent}))),
        json!([format!("@jack 2026-10-22: {}…", "b".repeat(59))])
    );

    let fits = format!("{}\u{1F1FA}\u{1F1F8}", "c".repeat(58));
    assert_eq!(
        aliases(&jacks_post(json!({"text": fits}))),
        json!([format!("@jack 2026-10-22: {fits}")])
    );
}

#[test]
fn a_long_post_is_cut_at_a_word_with_an_ellipsis() {
    let body = jacks_post(json!({
        "text": "a shorter text https://t.co/Long1",
        "note_tweet": {"text": long_text()}
    }));
    assert_eq!(aliases(&body), json!([LONG_ALIAS]));
}

#[test]
fn an_unknown_author_leaves_the_handle_out() {
    let mut body = jacks_post(json!({"text": "hello rust"}));
    body["includes"] = json!({});
    assert_eq!(aliases(&body), json!(["2026-10-22: hello rust"]));
}

#[test]
fn a_post_without_a_time_leaves_the_date_out() {
    let mut body = jacks_post(json!({"text": "hello rust"}));
    body["data"]
        .as_object_mut()
        .expect("post")
        .remove("created_at");
    assert_eq!(aliases(&body), json!(["@jack: hello rust"]));

    let bare = json!({"data": {"id": "1849", "text": "hello rust"}});
    assert_eq!(aliases(&bare), json!(["hello rust"]));

    let nothing = json!({"data": {"id": "1849"}});
    assert_eq!(aliases(&nothing), Value::Null);
}

#[test]
fn a_media_only_post_ends_at_the_date() {
    let body = jacks_post(json!({
        "text": "https://t.co/Pic123",
        "attachments": {"media_keys": ["3_1"]}
    }));
    assert_eq!(aliases(&body), json!(["@jack 2026-10-22"]));
}

#[test]
fn bookmark_and_like_notes_get_no_alias() {
    let body = jacks_post(json!({"text": "hello rust"}));
    for url in [
        "https://api.x.com/2/users/12/bookmarks",
        "https://api.x.com/2/users/12/liked_tweets",
    ] {
        let (notes, _) = plan(url, &body, Some("jack"));
        for note in &notes {
            let fm = frontmatter(&note.render(STAMP));
            match note.kind {
                Kind::Post => assert_eq!(fm["aliases"], json!(["@jack 2026-10-22: hello rust"])),
                Kind::Bookmark | Kind::Like => assert_eq!(fm.get("aliases"), None, "{url}"),
                Kind::User => {}
            }
        }
    }
}

fn liked(likes: u64) -> Value {
    jacks_post(json!({"text": "hello rust", "public_metrics": {"like_count": likes}}))
}

const OURS: &str = "aliases:\n  - \"@jack 2026-10-22: hello rust\"\n";

#[test]
fn a_users_alias_is_kept_byte_for_byte() {
    let f = fixture();
    f.archive.observe(POST, &liked(5), None, at(1));
    let first = f.read(&f.note());
    assert!(first.contains(OURS), "{first}");
    let theirs = "aliases: ['My #rust | [[pick]]', \"@jack 2026-10-22: hello rust\"]\n";
    std::fs::write(f.note(), first.replace(OURS, theirs)).expect("user edit");

    f.archive.observe(POST, &liked(9), None, at(2));
    let after = f.read(&f.note());
    assert!(after.contains("like_count: 9\n"), "{after}");
    assert!(after.contains(theirs), "{after}");
}

#[test]
fn birds_alias_joins_a_users_list_first() {
    let f = fixture();
    f.archive.observe(POST, &liked(5), None, at(1));
    let first = f.read(&f.note());
    assert!(first.contains(OURS), "{first}");
    let without = first.replace(OURS, "").replace(
        "bird_format: 1\n",
        "bird_format: 1\naliases:\n  - My pick\n  - 2026\n",
    );
    std::fs::write(f.note(), without).expect("user edit");

    f.archive.observe(POST, &liked(5), None, at(2));
    assert_eq!(
        frontmatter(&f.read(&f.note()))["aliases"],
        json!(["@jack 2026-10-22: hello rust", "My pick", "2026"])
    );
}

#[test]
fn an_unchanged_alias_writes_nothing() {
    let f = fixture();
    f.archive.observe(POST, &liked(5), None, at(1));
    let first = f.read(&f.note());
    assert!(first.contains(OURS), "{first}");
    let reordered = first.replace(
        OURS,
        "aliases:\n  - My pick\n  - \"@jack 2026-10-22: hello rust\"\n",
    );
    for text in [&first, &reordered] {
        std::fs::write(f.note(), text).expect("user edit");
        age(&f.note());
        let modified = mtime(&f.note());
        f.archive.observe(POST, &liked(5), None, at(2));
        assert_eq!(&f.read(&f.note()), text);
        assert_eq!(mtime(&f.note()), modified);
    }
}

#[test]
fn a_fuller_observation_keeps_one_alias() {
    let f = fixture();
    let full = long_text();
    let truncated = format!(
        "{}… https://t.co/Long1",
        full.chars().take(270).collect::<String>()
    );
    let short = jacks_post(json!({"text": truncated}));
    let fuller = jacks_post(json!({"text": truncated, "note_tweet": {"text": full}}));
    for (hour, body) in [(1, &short), (2, &fuller), (3, &short)] {
        f.archive.observe(POST, body, None, at(hour));
        let text = f.read(&f.note());
        assert_eq!(frontmatter(&text)["aliases"], json!([LONG_ALIAS]), "{text}");
        assert_eq!(text.contains(&full), hour == 2, "{text}");
    }
}
