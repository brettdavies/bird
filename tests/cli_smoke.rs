use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn bird() -> Command {
    Command::cargo_bin("bird").unwrap()
}

#[test]
fn version_flag() {
    bird()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bird"));
}

#[test]
fn help_flag() {
    bird()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("usage:")));
}

#[test]
fn no_args_shows_usage() {
    bird().assert().failure().code(2);
}

#[test]
fn watchlist_list_empty_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success();
}

#[test]
fn watchlist_add_and_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Add alice
    bird()
        .args(["watchlist", "add", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    // List should contain alice
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn watchlist_add_remove_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    bird()
        .args(["watchlist", "add", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    bird()
        .args(["watchlist", "remove", "alice"])
        .env("HOME", tmp.path())
        .assert()
        .success();
    // List should be empty (no "alice")
    bird()
        .args(["watchlist", "list"])
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("alice").not());
}
