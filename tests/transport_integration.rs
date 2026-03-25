//! Integration tests for the xurl subprocess transport layer.
//!
//! Tests the subprocess security properties and mock xurl behavior.
//! Unit tests for classify_error, MockTransport, and XurlError live in
//! src/transport.rs (in-crate tests that can access private items).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// Create a temporary executable shell script and run it.
/// Uses a fresh temp file each time, and retries execution on ETXTBSY
/// (kernel race when writing+executing scripts in rapid succession).
fn create_mock_xurl(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let path = dir.join(format!("mock_xurl_{}_{}", pid, id));
    fs::write(&path, content).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Execute a command, retrying once on ETXTBSY.
fn exec_mock(cmd: &mut Command) -> std::process::Output {
    match cmd.output() {
        Ok(out) => out,
        Err(e) if e.raw_os_error() == Some(26) => {
            // ETXTBSY: kernel still has the file locked from write; brief retry
            std::thread::sleep(std::time::Duration::from_millis(50));
            cmd.output().unwrap()
        }
        Err(e) => panic!("failed to execute mock xurl: {}", e),
    }
}

#[test]
fn adversarial_args_preserved_no_shell_injection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let args_file = tmp.path().join("received_args");
    let script = create_mock_xurl(
        tmp.path(),
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\" >> '{}'; done\nprintf '{{}}'\nexit 0\n",
            args_file.display()
        ),
    );

    let output = exec_mock(
        Command::new(&script)
            .args([
                "/2/tweets/search/recent",
                "--query",
                "$(rm -rf /); echo pwned",
                "--max-results",
                "10",
            ])
            .env("NO_COLOR", "1"),
    );

    assert!(output.status.success());

    let args = fs::read_to_string(&args_file).unwrap();
    assert!(
        args.contains("$(rm -rf /); echo pwned"),
        "Shell metacharacters should be preserved verbatim, got: {}",
        args
    );
}

#[test]
fn mock_xurl_success_returns_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json = r#"{"data":{"id":"123","username":"testuser"}}"#;
    let script = create_mock_xurl(
        tmp.path(),
        &format!("#!/bin/sh\nprintf '%s' '{}'\nexit 0\n", json),
    );

    let output = exec_mock(
        Command::new(&script)
            .args(["/2/users/me"])
            .env("NO_COLOR", "1"),
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["data"]["username"], "testuser");
}

#[test]
fn mock_xurl_error_returns_json_with_status() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json = r#"{"title":"Unauthorized","status":401,"detail":"Invalid token"}"#;
    let script = create_mock_xurl(
        tmp.path(),
        &format!("#!/bin/sh\nprintf '%s' '{}'\nexit 1\n", json),
    );

    let output = exec_mock(
        Command::new(&script)
            .args(["/2/users/me"])
            .env("NO_COLOR", "1"),
    );

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], 401);
}

#[test]
fn mock_xurl_version_subcommand() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = create_mock_xurl(
        tmp.path(),
        "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  printf 'xurl 1.0.3\\n'\n  exit 0\nfi\nprintf '{\"data\":{}}'\nexit 0\n",
    );

    let output = exec_mock(Command::new(&script).arg("version"));

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1.0.3"));
}

#[test]
fn mock_xurl_ansi_in_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = create_mock_xurl(
        tmp.path(),
        "#!/bin/sh\nprintf '\\033[31mError loading config\\033[0m\\n{\"data\":{\"id\":\"123\"}}'\nexit 0\n",
    );

    let output = exec_mock(Command::new(&script).args(["/2/users/me"]));

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('\x1b'), "Mock should emit ANSI escapes");
    assert!(stdout.contains(r#""id":"123""#));
}

#[test]
fn no_color_env_suppresses_ansi() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = create_mock_xurl(
        tmp.path(),
        "#!/bin/sh\nif [ -z \"$NO_COLOR\" ]; then\n  printf '\\033[31mcolored\\033[0m\\n'\nfi\nprintf '{\"data\":{}}'\nexit 0\n",
    );

    let output = exec_mock(
        Command::new(&script)
            .args(["/2/users/me"])
            .env("NO_COLOR", "1"),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\x1b'),
        "NO_COLOR=1 should suppress ANSI, got: {}",
        stdout
    );
}

/// Doctor now bypasses the xurl fail-fast check and reports xurl status via stdout JSON.
/// These tests verify that doctor succeeds (exit 0) and reports xurl as unavailable.
#[test]
fn bird_xurl_path_nonexistent_doctor_reports_unavailable() {
    #[allow(deprecated)]
    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["doctor"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_binary_12345")
        .env("HOME", tempfile::TempDir::new().unwrap().path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "doctor should exit 0 even without xurl"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["xurl"]["available"], false);
}

#[test]
fn bird_xurl_path_directory_doctor_reports_unavailable() {
    let tmp = tempfile::TempDir::new().unwrap();
    #[allow(deprecated)]
    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["doctor"])
        .env("BIRD_XURL_PATH", tmp.path())
        .env("HOME", tempfile::TempDir::new().unwrap().path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "doctor should exit 0 even with invalid xurl path"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["xurl"]["available"], false);
}

#[cfg(unix)]
#[test]
fn bird_xurl_path_not_executable_doctor_reports_unavailable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("fake_xurl");
    fs::write(&path, "not a script").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    #[allow(deprecated)]
    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["doctor"])
        .env("BIRD_XURL_PATH", &path)
        .env("HOME", tempfile::TempDir::new().unwrap().path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "doctor should exit 0 even with non-executable xurl"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["xurl"]["available"], false);
}

/// API commands should still fail-fast when xurl is missing (exit 78).
#[test]
fn bird_me_without_xurl_still_fails_fast() {
    #[allow(deprecated)]
    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["me"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_binary_12345")
        .env("HOME", tempfile::TempDir::new().unwrap().path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(78),
        "API commands should exit 78 when xurl is missing"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "Should report xurl not found on stderr, got: {}",
        stderr
    );
}

// ── Pipe deadlock regression tests ───────────────────────────────────────
//
// These tests exercise xurl_call() end-to-end through the bird binary with
// mock xurl scripts that produce output exceeding the OS pipe buffer (64 KB
// on Linux). They guard against the deadlock fixed in 580f6e5: if bird
// waits for child exit before draining stdout/stderr, the child blocks
// writing to a full pipe buffer while bird blocks waiting — classic deadlock.
//
// All tests use .timeout() so a deadlock manifests as a timeout failure
// rather than hanging CI forever.

/// Regression: xurl writing >64KB to stdout must not deadlock bird.
/// Before the fix, bird called waitpid() before reading stdout. If xurl
/// wrote more than the pipe buffer, both processes blocked forever.
#[cfg(unix)]
#[test]
fn large_stdout_no_deadlock() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Mock xurl outputs ~100KB of valid JSON (exceeds 64KB pipe buffer).
    // dd writes 100,000 null bytes, tr converts to 'A', wrapped in JSON.
    let script = create_mock_xurl(
        tmp.path(),
        concat!(
            "#!/bin/sh\n",
            "printf '{\"data\":{\"id\":\"123\",\"text\":\"'\n",
            "dd if=/dev/zero bs=100000 count=1 2>/dev/null | tr '\\0' 'A'\n",
            "printf '\"}}'\n",
        ),
    );

    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["get", "/2/users/me"])
        .env("BIRD_XURL_PATH", &script)
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .timeout(std::time::Duration::from_secs(30))
        .output()
        .expect("bird timed out — probable pipe deadlock on large stdout");

    assert!(
        output.status.success(),
        "bird should succeed with large xurl stdout, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.len() > 64 * 1024,
        "stdout should be >64KB (got {} bytes) to prove pipe was drained",
        stdout.len()
    );
}

/// Regression: xurl writing >64KB to stderr must not deadlock bird.
/// stderr uses the same pipe buffer as stdout — both must be drained
/// concurrently in background threads.
#[cfg(unix)]
#[test]
fn large_stderr_no_deadlock() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Mock xurl writes ~100KB to stderr and an error JSON to stdout.
    let script = create_mock_xurl(
        tmp.path(),
        concat!(
            "#!/bin/sh\n",
            "dd if=/dev/zero bs=100000 count=1 2>/dev/null | tr '\\0' 'E' >&2\n",
            "printf '{\"title\":\"Error\",\"status\":500,\"detail\":\"server error\"}'\n",
            "exit 1\n",
        ),
    );

    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["get", "/2/users/me"])
        .env("BIRD_XURL_PATH", &script)
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .timeout(std::time::Duration::from_secs(30))
        .output()
        .expect("bird timed out — probable pipe deadlock on large stderr");

    // bird should exit with an error (xurl returned status 500), but NOT hang.
    assert!(
        !output.status.success(),
        "bird should fail when xurl returns error"
    );
}

/// Regression: xurl writing >64KB to BOTH stdout and stderr simultaneously
/// must not deadlock. A regression that serializes the drain threads (e.g.,
/// joining stdout before spawning stderr) would deadlock when both pipes fill.
#[cfg(unix)]
#[test]
fn large_stdout_and_stderr_simultaneous_no_deadlock() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Mock xurl writes ~100KB to stderr in a background subshell while
    // writing ~100KB to stdout in the foreground, then waits for both.
    let script = create_mock_xurl(
        tmp.path(),
        concat!(
            "#!/bin/sh\n",
            "dd if=/dev/zero bs=100000 count=1 2>/dev/null | tr '\\0' 'E' >&2 &\n",
            "printf '{\"data\":{\"id\":\"123\",\"text\":\"'\n",
            "dd if=/dev/zero bs=100000 count=1 2>/dev/null | tr '\\0' 'A'\n",
            "printf '\"}}'\n",
            "wait\n",
            "exit 0\n",
        ),
    );

    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["get", "/2/users/me"])
        .env("BIRD_XURL_PATH", &script)
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .timeout(std::time::Duration::from_secs(30))
        .output()
        .expect("bird timed out — probable deadlock on simultaneous stdout+stderr");

    assert!(
        output.status.success(),
        "bird should succeed with large concurrent stdout+stderr, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.len() > 64 * 1024,
        "stdout should be >64KB (got {} bytes)",
        stdout.len()
    );
}

/// xurl killed by a signal (SIGSEGV, SIGABRT) should produce a clean error,
/// not hang or panic. When a process dies by signal, status.code() is None
/// and only status.signal() has a value — bird must handle this gracefully.
#[cfg(unix)]
#[test]
fn child_signal_death_reported_cleanly() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Mock xurl writes partial JSON then kills itself with SIGKILL.
    // SIGKILL (not SEGV) because SEGV might be caught by the shell.
    let script = create_mock_xurl(
        tmp.path(),
        concat!(
            "#!/bin/sh\n",
            "printf '{\"data\":{\"partial\"'\n",
            "kill -9 $$\n",
        ),
    );

    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["get", "/2/users/me"])
        .env("BIRD_XURL_PATH", &script)
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("NO_COLOR", "1")
        .timeout(std::time::Duration::from_secs(30))
        .output()
        .expect("bird timed out — probable hang on signal-killed child");

    // bird should report an error (non-zero exit), not succeed with partial data
    assert!(
        !output.status.success(),
        "bird should fail when xurl is killed by signal"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty(), "bird should report the error on stderr");
}
