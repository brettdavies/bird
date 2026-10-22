//! Fails when a test or a source file mutates the process environment.
//!
//! Tests in one binary share a process, and `run_with_paths` executes the CLI
//! in-process, so a `set_var` in one test reaches every other test's run. That
//! is a data race the compiler cannot see: `std::env::set_var` is `unsafe` in
//! edition 2024 because a concurrent read is undefined behavior. A test
//! supplies its environment through `EnvOverrides` and the injected entrypoint
//! instead, so nothing needs ordering.
//!
//! The allowlist names the tests that must keep mutating, because they prove
//! the process read itself works. Adding an entry is a deliberate, reviewable
//! act; forgetting an attribute is not.

use std::path::{Path, PathBuf};

/// A test the guard exempts, with the reason it is exempt. `file` is relative
/// to the crate root.
struct Allowed {
    file: &'static str,
    test: &'static str,
    reason: &'static str,
}

/// Every sanctioned process-environment mutation.
const ALLOWLIST: &[Allowed] = &[];

const PATTERNS: [&str; 2] = ["env::set_var(", "env::remove_var("];

fn crate_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `dir`, recursively, sorted.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The integration tests and the sources, named relative to the crate root.
/// Guard files are skipped because their pattern strings would match
/// themselves.
fn scanned_files() -> Vec<(String, PathBuf)> {
    let root = crate_root();
    let tests = rust_files(&root.join("tests")).into_iter().filter(|path| {
        !path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with("_guard.rs"))
    });
    let sources = rust_files(&root.join("src"));
    tests
        .chain(sources)
        .map(|path| {
            let name = path
                .strip_prefix(root)
                .expect("under the crate root")
                .to_string_lossy()
                .into_owned();
            (name, path)
        })
        .collect()
}

/// The name of the `fn` a byte offset falls inside.
fn enclosing_fn(source: &str, offset: usize) -> String {
    source[..offset]
        .rmatch_indices("fn ")
        .find_map(|(i, _)| {
            let name: String = source[i + 3..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (!name.is_empty()).then_some(name)
        })
        .unwrap_or_else(|| "<unknown>".to_string())
}

#[test]
fn nothing_mutates_the_process_environment() {
    let mut violations = Vec::new();
    for (file, path) in scanned_files() {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for pattern in PATTERNS {
            for (offset, _) in source.match_indices(pattern) {
                let test = enclosing_fn(&source, offset);
                let allowed = ALLOWLIST.iter().any(|a| a.file == file && a.test == test);
                if !allowed {
                    violations.push(format!("{file}::{test} calls {pattern}"));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "process-environment mutation found:\n  {}\n\n\
         Supply the value through `EnvOverrides` and `run_with_paths`, or pass the flag that \
         carries it. If the test exists to prove the process read itself, add it to ALLOWLIST \
         in this file with the reason.",
        violations.join("\n  ")
    );
}

#[test]
fn allowlist_entries_still_exist() {
    let stale: Vec<String> = ALLOWLIST
        .iter()
        .filter(|entry| {
            let path = crate_root().join(entry.file);
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            !source.contains(&format!("fn {}(", entry.test))
        })
        .map(|entry| format!("{}::{} ({})", entry.file, entry.test, entry.reason))
        .collect();

    assert!(
        stale.is_empty(),
        "ALLOWLIST names tests that no longer exist:\n  {}\n\n\
         Remove the entry. A stale exemption silently widens what the guard permits.",
        stale.join("\n  ")
    );
}
