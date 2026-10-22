//! Shared fixtures for library-style integration tests.
//!
//! Tests use [`TestEnv`] to construct a `TempDir`-backed [`ResolvedPaths`] and
//! call [`bird::cli::run_with_paths`] in-process instead of forking the binary.
//!
//! Default assertion scope: exit code + filesystem side effects. The
//! [`Vec<u8>`] writers passed into `run_with_paths` capture every stdout and
//! stderr byte the runner emits.

#![allow(dead_code)]

use bird::config::{EnvOverrides, ResolvedPaths};
use std::process::ExitCode;
use tempfile::TempDir;

/// Per-test fixture owning a `TempDir` and the derived [`ResolvedPaths`] +
/// [`EnvOverrides`] snapshot. Drop the `TestEnv` to clean up the directory.
pub struct TestEnv {
    pub paths: ResolvedPaths,
    pub env: EnvOverrides,
    _tmp: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("test: tempdir");
        let config_dir = tmp.path().join(".config").join("bird");
        std::fs::create_dir_all(&config_dir).expect("test: create config dir");
        let paths = ResolvedPaths {
            config_dir: config_dir.clone(),
            token_store: Some(config_dir.join("auth.yml")),
        };
        Self {
            paths,
            env: EnvOverrides::default(),
            _tmp: tmp,
        }
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Invoke [`bird::cli::run_with_paths`] in-process with captured writers.
///
/// Returns `(exit_code_byte, stdout_utf8, stderr_utf8)`. Per Plan 1 U9 scope,
/// `stdout` / `stderr` are typically empty because the output macros bypass the
/// injected writers; rely on the exit code byte and on filesystem side effects
/// until Plan 2 U11 lands.
pub fn run_in_process(args: &[&str], env: &TestEnv) -> (u8, String, String) {
    run_in_process_with_stdin(args, env, &mut std::io::empty())
}

/// [`run_in_process`] reading `stdin` in place of an empty input.
pub fn run_in_process_with_stdin(
    args: &[&str],
    env: &TestEnv,
    stdin: &mut dyn std::io::BufRead,
) -> (u8, String, String) {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let exit = bird::cli::run_with_paths(
        args.iter().copied(),
        stdin,
        &mut stdout,
        &mut stderr,
        env.paths.clone(),
        env.env.clone(),
    );
    (
        exit_code_byte(exit),
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    )
}

/// A stdin that fails the test the moment anything reads it.
pub struct PanicOnRead;

impl std::io::Read for PanicOnRead {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        panic!("test: stdin was read");
    }
}

impl std::io::BufRead for PanicOnRead {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        panic!("test: stdin was read");
    }

    fn consume(&mut self, _amt: usize) {}
}

/// The last JSON document in `stderr`, indented or on one line.
pub fn stderr_envelope(stderr: &str) -> serde_json::Value {
    last_json_document(stderr)
        .unwrap_or_else(|| panic!("test: no JSON envelope on stderr: {stderr:?}"))
}

/// The last JSON document that starts at the beginning of a line of `text`.
pub fn last_json_document(text: &str) -> Option<serde_json::Value> {
    let starts = text
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .chain(std::iter::once(0))
        .filter(|&i| text[i..].starts_with('{'));
    let mut starts: Vec<usize> = starts.collect();
    starts.sort_unstable();
    starts.into_iter().rev().find_map(|i| {
        serde_json::Deserializer::from_str(&text[i..])
            .into_iter::<serde_json::Value>()
            .next()
            .and_then(Result::ok)
    })
}

/// Extract the inner exit-status byte from an [`ExitCode`].
///
/// `ExitCode` does not implement `PartialEq` on stable, so its `Debug` impl is
/// the documented escape hatch. The format is
/// `"ExitCode(unix_exit_status(N))"` on Unix and `"ExitCode(N)"` on Windows;
/// both render the byte inside the innermost parentheses. We walk to the
/// innermost `(` and read the digit run that follows.
fn exit_code_byte(code: ExitCode) -> u8 {
    let dbg = format!("{:?}", code);
    let start = dbg
        .rfind('(')
        .unwrap_or_else(|| panic!("test: no '(' in {:?}", dbg));
    let digits: String = dbg[start + 1..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits
        .parse::<u8>()
        .unwrap_or_else(|_| panic!("test: could not parse exit code byte from {:?}", dbg))
}

/// Assert that `stdout` parses as a JSON object matching the expected envelope
/// kind.
///
/// * `kind == "success"` requires the `data` and `meta` keys.
/// * `kind == "error"` requires `error`, `kind`, `message`, and `exit_code`.
pub fn assert_envelope_shape(stdout: &str, kind: &str) {
    let val: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("test: envelope is valid JSON");
    let obj = val.as_object().expect("test: envelope is object");
    match kind {
        "success" => {
            assert!(obj.contains_key("data"), "success envelope must have data");
            assert!(obj.contains_key("meta"), "success envelope must have meta");
        }
        "error" => {
            for key in &["error", "kind", "message", "exit_code"] {
                assert!(obj.contains_key(*key), "error envelope must have {}", key);
            }
        }
        other => panic!("unknown envelope kind: {}", other),
    }
}
