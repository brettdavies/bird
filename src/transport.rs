//! xurl subprocess transport layer.
//!
//! All X API HTTP calls route through xurl as a subprocess. Bird owns the
//! intelligence layer (entity store, caching, cost tracking, UX); xurl owns
//! the transport layer (auth, HTTP, X API compatibility).
//!
//! # Security Invariants
//!
//! - NEVER use shell=true or compose a single string from multiple args.
//!   `Command::new(path).args(args)` calls execvp directly — no shell interpretation.
//! - NEVER pass tokens, credentials, or secrets as subprocess arguments.
//!   xurl reads auth from its own token store (~/.xurl).
//! - All user input (search queries, tweet text) passes as separate argv elements.

use crate::output;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

/// Maximum stdout capture size (50 MB) to prevent memory exhaustion.
const MAX_STDOUT_BYTES: usize = 50 * 1024 * 1024;

/// Subprocess timeout before SIGTERM.
const TIMEOUT_SECS: u64 = 60;

/// Grace period after SIGTERM before SIGKILL.
const KILL_GRACE_SECS: u64 = 5;

/// Minimum supported xurl version.
const MIN_VERSION: &str = "1.0.3";

/// Cached absolute path to the xurl binary, resolved once at startup.
static XURL_PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Resolve and cache the absolute path to the xurl binary.
/// Checks `BIRD_XURL_PATH` env var first, falls back to `which::which("xurl")`.
pub fn resolve_xurl_path() -> Result<&'static Path, Box<dyn std::error::Error + Send + Sync>> {
    let result = XURL_PATH.get_or_init(|| {
        if let Ok(path) = std::env::var("BIRD_XURL_PATH") {
            let p = PathBuf::from(&path);
            if !p.exists() {
                return Err(format!("BIRD_XURL_PATH={} does not exist", path));
            }
            let p = p
                .canonicalize()
                .map_err(|e| format!("BIRD_XURL_PATH={} cannot be resolved: {}", path, e))?;
            if !p.is_file() {
                return Err(format!("BIRD_XURL_PATH={} is not a file", p.display()));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = p
                    .metadata()
                    .map_err(|e| format!("BIRD_XURL_PATH={}: {}", path, e))?
                    .permissions()
                    .mode();
                if mode & 0o111 == 0 {
                    return Err(format!("BIRD_XURL_PATH={} is not executable", path));
                }
            }
            return Ok(p);
        }
        which::which("xurl")
            .map_err(|_| "xurl not found. Install: brew install xdevplatform/tap/xurl".to_string())
    });
    match result {
        Ok(p) => Ok(p.as_path()),
        Err(e) => Err(e.clone().into()),
    }
}

/// Run `xurl version` and return the version string. Warns if below minimum.
pub fn check_xurl_version(path: &Path) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new(path)
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run xurl version: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // xurl version output: "xurl 1.0.3\n"
    let version = stdout
        .trim()
        .strip_prefix("xurl ")
        .unwrap_or(stdout.trim())
        .to_string();

    if !version.is_empty() {
        let clean = version.strip_prefix('v').unwrap_or(&version);
        if let (Ok(current), Ok(minimum)) = (
            semver::Version::parse(clean),
            semver::Version::parse(MIN_VERSION),
        ) && current < minimum
        {
            eprintln!(
                "[transport] warning: xurl {} is below minimum {}; consider upgrading",
                version, MIN_VERSION
            );
        }
    }

    Ok(version)
}

/// Error from an xurl subprocess call.
#[derive(Debug)]
pub enum XurlError {
    /// xurl binary not found (exit 78 — EX_CONFIG)
    NotFound(String),
    /// xurl returned an auth error (HTTP 401/403)
    Auth(String),
    /// xurl returned an API error (non-auth HTTP error)
    Api { status: u16, message: String },
    /// xurl process timed out
    Timeout,
    /// xurl process failed (non-JSON stderr, crash, etc.)
    Process(String),
}

impl std::fmt::Display for XurlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XurlError::NotFound(msg) => write!(f, "{}", msg),
            XurlError::Auth(msg) => write!(f, "auth error: {}", msg),
            XurlError::Api { status, message } => write!(f, "API error {}: {}", status, message),
            XurlError::Timeout => write!(f, "xurl timed out after {}s", TIMEOUT_SECS),
            XurlError::Process(msg) => write!(f, "xurl process error: {}", msg),
        }
    }
}

impl std::error::Error for XurlError {}

/// Call xurl with the given arguments, capture stdout as JSON.
///
/// Spawns xurl with `NO_COLOR=1` to suppress ANSI escape codes in output.
/// Stdout is piped and parsed as JSON. On failure, classifies the error type
/// from the JSON body's `status` field or stderr content.
pub fn xurl_call(
    args: &[&str],
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let path = resolve_xurl_path()?;

    let mut child = match Command::new(path)
        .args(args)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Box::new(XurlError::NotFound(
                "xurl not found. Install: brew install xdevplatform/tap/xurl".into(),
            )));
        }
        Err(e) => {
            return Err(Box::new(XurlError::Process(format!(
                "failed to spawn xurl: {}",
                e
            ))));
        }
    };

    // Wait with timeout
    let status = wait_with_timeout(&mut child, Duration::from_secs(TIMEOUT_SECS))?;

    // Capture stdout (capped at MAX_STDOUT_BYTES)
    let mut stdout_buf = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        stdout
            .take(MAX_STDOUT_BYTES as u64)
            .read_to_end(&mut stdout_buf)?;
    }
    let stdout_str = String::from_utf8_lossy(&stdout_buf);

    // Capture stderr
    let mut stderr_buf = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        stderr
            .take(MAX_STDOUT_BYTES as u64)
            .read_to_end(&mut stderr_buf)?;
    }
    let stderr_str = String::from_utf8_lossy(&stderr_buf);

    // Strip ANSI lines as fallback (hardcoded escape codes in xurl error paths)
    let clean_stdout = output::strip_ansi_lines(&stdout_str);

    if status.success() {
        // Exit 0: parse stdout as JSON
        let json: serde_json::Value = serde_json::from_str(&clean_stdout).map_err(|e| {
            XurlError::Process(format!(
                "xurl returned invalid JSON: {} (stdout: {})",
                e,
                output::sanitize_for_stderr(&clean_stdout, 200)
            ))
        })?;
        Ok(json)
    } else {
        // Exit non-zero: classify error
        classify_error(&clean_stdout, &stderr_str)
    }
}

/// Run xurl with inherited stdio (for interactive flows like `bird login`).
/// No timeout: OAuth2 flows require user interaction in a browser; user can Ctrl+C.
pub fn xurl_passthrough(args: &[&str]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = resolve_xurl_path()?;

    let status = Command::new(path)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Box::new(XurlError::NotFound(
                    "xurl not found. Install: brew install xdevplatform/tap/xurl".into(),
                )) as Box<dyn std::error::Error + Send + Sync>
            } else {
                Box::new(XurlError::Process(format!("failed to run xurl: {}", e)))
                    as Box<dyn std::error::Error + Send + Sync>
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(Box::new(XurlError::Process(format!(
            "xurl exited with code {}",
            status.code().unwrap_or(-1)
        ))))
    }
}

/// Classify an xurl error from its stdout JSON and stderr.
fn classify_error(
    stdout: &str,
    stderr: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    // Try to parse stdout as JSON error response
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
        let status = json.get("status").and_then(|s| s.as_u64()).unwrap_or(0) as u16;

        let detail = json
            .get("detail")
            .and_then(|d| d.as_str())
            .or_else(|| json.get("title").and_then(|t| t.as_str()))
            .unwrap_or("unknown error")
            .to_string();

        return Err(match status {
            401 | 403 => Box::new(XurlError::Auth(detail)),
            _ if status > 0 => Box::new(XurlError::Api {
                status,
                message: detail,
            }),
            // status=0 means no HTTP status in response — treat as process error
            _ => Box::new(XurlError::Api {
                status: 0,
                message: detail,
            }),
        });
    }

    // No JSON in stdout — use stderr
    let msg = if stderr.is_empty() {
        output::sanitize_for_stderr(stdout, 200)
    } else {
        output::sanitize_for_stderr(stderr, 200)
    };

    Err(Box::new(XurlError::Process(msg)))
}

/// Wait for a child process with a timeout. Sends SIGTERM, then SIGKILL after grace period.
/// Always reaps the child to prevent zombies.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, Box<dyn std::error::Error + Send + Sync>> {
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(50);

    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None => {
                if start.elapsed() >= timeout {
                    // Timeout: SIGTERM
                    #[cfg(unix)]
                    {
                        unsafe {
                            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = child.kill();
                    }

                    // Grace period for SIGTERM
                    let grace_start = std::time::Instant::now();
                    loop {
                        match child.try_wait()? {
                            Some(status) => return Ok(status),
                            None => {
                                if grace_start.elapsed() >= Duration::from_secs(KILL_GRACE_SECS) {
                                    // SIGKILL and reap to prevent zombie
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    return Err(Box::new(XurlError::Timeout));
                                }
                                std::thread::sleep(poll_interval);
                            }
                        }
                    }
                }
                std::thread::sleep(poll_interval);
            }
        }
    }
}

/// Transport trait for testability. Production uses XurlTransport; tests use MockTransport.
pub trait Transport {
    fn request(
        &self,
        args: &[String],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Production transport: delegates to xurl subprocess.
pub struct XurlTransport;

impl Transport for XurlTransport {
    fn request(
        &self,
        args: &[String],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        xurl_call(&arg_refs)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Mock transport for unit tests. Returns pre-configured responses in order.
    pub struct MockTransport {
        pub responses:
            RefCell<VecDeque<Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>>>,
    }

    impl MockTransport {
        pub fn new(
            responses: Vec<Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>>,
        ) -> Self {
            Self {
                responses: RefCell::new(VecDeque::from(responses)),
            }
        }
    }

    impl Transport for MockTransport {
        fn request(
            &self,
            _args: &[String],
        ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err("MockTransport: no more responses".into()))
        }
    }

    #[test]
    fn xurl_error_display_not_found() {
        let e = XurlError::NotFound("xurl not found".into());
        assert_eq!(e.to_string(), "xurl not found");
    }

    #[test]
    fn xurl_error_display_auth() {
        let e = XurlError::Auth("Unauthorized".into());
        assert_eq!(e.to_string(), "auth error: Unauthorized");
    }

    #[test]
    fn xurl_error_display_api() {
        let e = XurlError::Api {
            status: 429,
            message: "Too Many Requests".into(),
        };
        assert_eq!(e.to_string(), "API error 429: Too Many Requests");
    }

    #[test]
    fn xurl_error_display_timeout() {
        let e = XurlError::Timeout;
        assert!(e.to_string().contains("timed out"));
    }

    #[test]
    fn classify_error_auth_401() {
        let stdout = r#"{"title":"Unauthorized","status":401,"detail":"Unauthorized"}"#;
        let result = classify_error(stdout, "");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("auth error"));
    }

    #[test]
    fn classify_error_auth_403() {
        let stdout = r#"{"title":"Forbidden","status":403,"detail":"Forbidden"}"#;
        let result = classify_error(stdout, "");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("auth error"));
    }

    #[test]
    fn classify_error_api_429() {
        let stdout = r#"{"title":"Too Many Requests","status":429,"detail":"Rate limit exceeded"}"#;
        let result = classify_error(stdout, "");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("API error 429"));
    }

    #[test]
    fn classify_error_no_json_uses_stderr() {
        let result = classify_error("not json", "some error on stderr");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("some error on stderr"));
    }

    #[test]
    fn classify_error_no_json_no_stderr_uses_stdout() {
        let result = classify_error("raw error output", "");
        let err = result.unwrap_err();
        assert!(err.to_string().contains("raw error output"));
    }

    #[test]
    fn mock_transport_returns_responses_in_order() {
        let mock = MockTransport::new(vec![
            Ok(serde_json::json!({"data": "first"})),
            Ok(serde_json::json!({"data": "second"})),
        ]);
        let r1 = mock.request(&[]).unwrap();
        assert_eq!(r1["data"], "first");
        let r2 = mock.request(&[]).unwrap();
        assert_eq!(r2["data"], "second");
    }

    #[test]
    fn mock_transport_exhausted_returns_error() {
        let mock = MockTransport::new(vec![]);
        let result = mock.request(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn version_comparison_multi_digit() {
        // The bug: lexicographic "1.0.9" > "1.0.10" because '9' > '1'
        assert!(
            semver::Version::parse("1.0.9").unwrap() < semver::Version::parse("1.0.10").unwrap()
        );
        assert!(
            (semver::Version::parse("1.0.10").unwrap() >= semver::Version::parse("1.0.3").unwrap())
        );
    }

    #[test]
    fn version_comparison_major() {
        assert!(
            (semver::Version::parse("2.0.0").unwrap() >= semver::Version::parse("1.0.3").unwrap())
        );
    }

    #[test]
    fn version_comparison_prerelease() {
        // semver spec: pre-release < release
        assert!(
            semver::Version::parse("1.0.3-beta").unwrap()
                < semver::Version::parse("1.0.3").unwrap()
        );
    }
}
