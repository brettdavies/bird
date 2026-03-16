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

#[test]
fn bird_xurl_path_nonexistent_does_not_crash() {
    #[allow(deprecated)]
    let output = assert_cmd::Command::cargo_bin("bird")
        .unwrap()
        .args(["doctor"])
        .env("BIRD_XURL_PATH", "/tmp/nonexistent_xurl_binary_12345")
        .env("HOME", tempfile::TempDir::new().unwrap().path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "Should report nonexistent path, got stderr={}",
        stderr
    );
}

#[test]
fn bird_xurl_path_directory_rejected() {
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not a file"),
        "Should reject directory path, got stderr={}",
        stderr
    );
}

#[cfg(unix)]
#[test]
fn bird_xurl_path_not_executable_rejected() {
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is not executable"),
        "Should reject non-executable file, got stderr={}",
        stderr
    );
}
