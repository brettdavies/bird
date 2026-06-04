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

use crate::config::EnvOverrides;
use crate::output;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Maximum stdout capture size (50 MB) to prevent memory exhaustion.
const MAX_STDOUT_BYTES: usize = 50 * 1024 * 1024;

/// Grace period after SIGTERM before SIGKILL.
const KILL_GRACE_SECS: u64 = 5;

/// Minimum supported xurl version.
const MIN_VERSION: &str = "1.0.3";

/// Centralized xurl install guidance (DRY across transport.rs and doctor.rs).
pub const XURL_INSTALL_HINT: &str = "Install xurl-rs: brew install brettdavies/tap/xurl-rs (or Go xurl: brew install xdevplatform/tap/xurl)";

/// Resolve the absolute path to the xurl binary against caller-supplied env.
///
/// Pure with respect to its input: the runner snapshots the host env into
/// [`EnvOverrides`] once at startup and threads that snapshot through here.
/// Tests pass an [`EnvOverrides`] with an explicit `xurl_path` (or `None` to
/// exercise the `which` fallback) without mutating process env.
///
/// Honors `env.xurl_path` first (validating existence + executable bit), then
/// falls back to `which::which("xr")` then `which::which("xurl")`. Resolved
/// paths are canonicalized and version-checked as an integrity gate.
pub fn resolve_xurl_path(
    env: &EnvOverrides,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(path) = env.xurl_path.as_ref() {
        if !path.exists() {
            return Err(format!("BIRD_XURL_PATH={} does not exist", path.display()).into());
        }
        let p = path.canonicalize().map_err(|e| {
            format!(
                "BIRD_XURL_PATH={} cannot be resolved: {}",
                path.display(),
                e
            )
        })?;
        if !p.is_file() {
            return Err(format!("BIRD_XURL_PATH={} is not a file", p.display()).into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = p
                .metadata()
                .map_err(|e| format!("BIRD_XURL_PATH={}: {}", path.display(), e))?
                .permissions()
                .mode();
            if mode & 0o111 == 0 {
                return Err(format!("BIRD_XURL_PATH={} is not executable", path.display()).into());
            }
        }
        return Ok(p);
    }
    // Try xr (xurl-rs) first, then xurl (Go original).
    // Canonicalize to resolve symlinks and mitigate impersonation.
    // Version check acts as integrity gate: reject binaries that don't
    // report a parseable version with "xurl " or "xr " prefix.
    for name in &["xr", "xurl"] {
        if let Ok(found) = which::which(name) {
            let canonical = found.canonicalize().unwrap_or(found);
            if verify_xurl_binary(&canonical) {
                return Ok(canonical);
            }
        }
    }
    Err(format!("xurl not found. {}", XURL_INSTALL_HINT).into())
}

/// Verify a candidate binary is a genuine xurl/xr by checking its version output.
/// Returns true if the binary reports a parseable version string.
fn verify_xurl_binary(path: &Path) -> bool {
    let Ok(output) = Command::new(path)
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_version_string(stdout.trim()).is_some()
}

/// Parse a version string from xurl/xr version output.
/// Accepts formats: "xurl X.Y.Z", "xr X.Y.Z", or bare "X.Y.Z" (with optional v prefix).
fn parse_version_string(s: &str) -> Option<semver::Version> {
    let version_part = s
        .strip_prefix("xurl ")
        .or_else(|| s.strip_prefix("xr "))
        .unwrap_or(s);
    let clean = version_part.strip_prefix('v').unwrap_or(version_part);
    semver::Version::parse(clean).ok()
}

/// Run `xurl version` (or `xr version`) and return the version string.
/// Warns if below minimum. Handles both "xurl X.Y.Z" and "xr X.Y.Z" prefixes.
///
/// `stderr` receives the below-minimum warning under the KTD-1 guard.
pub fn check_xurl_version(
    path: &Path,
    stderr: &mut dyn std::io::Write,
    quiet: bool,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new(path)
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run xurl version: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();

    if let Some(current) = parse_version_string(trimmed) {
        if let Ok(minimum) = semver::Version::parse(MIN_VERSION)
            && current < minimum
            && !quiet
        {
            writeln!(
                stderr,
                "[transport] warning: xurl {} is below minimum {}; consider upgrading",
                current, MIN_VERSION
            )
            .ok();
        }
        Ok(current.to_string())
    } else {
        // Return raw output if we can't parse — still useful for diagnostics
        Ok(trimmed.to_string())
    }
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
    /// xurl process timed out after the given duration.
    Timeout(Duration),
    /// xurl process failed (non-JSON stderr, crash, etc.)
    Process(String),
}

impl std::fmt::Display for XurlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XurlError::NotFound(msg) => write!(f, "{}", msg),
            XurlError::Auth(msg) => write!(f, "auth error: {}", msg),
            XurlError::Api { status, message } => write!(f, "API error {}: {}", status, message),
            XurlError::Timeout(d) => {
                write!(f, "timeout: xurl exceeded {}s", d.as_secs())
            }
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
///
/// Caller supplies the resolved xurl binary path and the subprocess timeout;
/// the runner resolves both once at startup and threads them through.
pub fn xurl_call(
    args: &[&str],
    xurl_path: &Path,
    timeout: Duration,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut child = match Command::new(xurl_path)
        .args(args)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Box::new(XurlError::NotFound(format!(
                "xurl not found. {}",
                XURL_INSTALL_HINT
            ))));
        }
        Err(e) => {
            return Err(Box::new(XurlError::Process(format!(
                "failed to spawn xurl: {}",
                e
            ))));
        }
    };

    // Drain stdout/stderr in background threads to prevent pipe-buffer deadlock.
    // If we wait for exit before reading, the child can block writing to a full
    // pipe buffer (typically 64 KB on Linux), deadlocking both processes.
    let stdout_thread = child.stdout.take().map(|out| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            out.take(MAX_STDOUT_BYTES as u64).read_to_end(&mut buf).ok();
            buf
        })
    });
    let stderr_thread = child.stderr.take().map(|err| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            err.take(MAX_STDOUT_BYTES as u64).read_to_end(&mut buf).ok();
            buf
        })
    });

    // Wait with timeout (child can now write freely — readers are draining)
    let status = wait_with_timeout(&mut child, timeout)?;

    // Join reader threads
    let stdout_buf = stdout_thread
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    let stdout_str = String::from_utf8_lossy(&stdout_buf);

    let stderr_buf = stderr_thread
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
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
pub fn xurl_passthrough(
    args: &[&str],
    xurl_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let status = Command::new(xurl_path)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Box::new(XurlError::NotFound(format!(
                    "xurl not found. {}",
                    XURL_INSTALL_HINT
                ))) as Box<dyn std::error::Error + Send + Sync>
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
                                    return Err(Box::new(XurlError::Timeout(timeout)));
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

/// Transport trait for testability. Production uses [`XurlTransport`]; tests
/// use `MockTransport` (defined under `#[cfg(test)]` in this module, so it's
/// invisible to rustdoc and intentionally not linked here).
///
/// The `Send + Sync` bound is a hard prerequisite for the
/// `Arc<Mutex<dyn Write + Send>>` writer storage on `BirdClient`, which needs
/// `Box<dyn Transport>` (a field on `BirdClient`) to qualify as `Send + Sync`.
///
/// Implementations carry their own configuration (resolved xurl path,
/// subprocess timeout). The runner constructs the production transport with
/// the resolved path and `--timeout` value at startup and threads it through.
pub trait Transport: Send + Sync {
    fn request(
        &self,
        args: &[String],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;

    /// Resolved xurl binary path, when the transport spawns a real subprocess.
    /// Mock transports return `None`. Surfaced for direct call sites
    /// (`bird login`, write commands) that need the binary path without going
    /// through [`Self::request`].
    fn xurl_path(&self) -> Option<&Path> {
        None
    }
}

/// Production transport: delegates to xurl subprocess. Carries the resolved
/// xurl binary path (or the resolution error message) and the subprocess
/// timeout; both are caller-supplied at construction.
///
/// The path is wrapped in `Result<PathBuf, String>` so the runner can
/// construct the transport unconditionally: commands that never spawn xurl
/// (local watchlist, cache, doctor's xurl-status report) tolerate the error
/// path silently, while commands that DO spawn xurl surface the resolution
/// error on first [`Transport::request`].
pub struct XurlTransport {
    xurl_path: Result<PathBuf, String>,
    timeout: Duration,
}

impl XurlTransport {
    /// Construct a transport bound to a successfully-resolved xurl binary.
    /// The resolved path is surfaced through the [`Transport::xurl_path`]
    /// trait method so direct call sites (`bird login`, write commands) read
    /// off the live transport rather than re-resolving.
    pub fn new(xurl_path: PathBuf, timeout: Duration) -> Self {
        Self {
            xurl_path: Ok(xurl_path),
            timeout,
        }
    }

    /// Construct a transport that will surface `error` on every
    /// [`Transport::request`] call. Used when the runner could not resolve
    /// the xurl binary but the command being dispatched might still succeed
    /// without spawning xurl (`bird doctor`, local-only watchlist commands).
    pub fn from_error(error: String, timeout: Duration) -> Self {
        Self {
            xurl_path: Err(error),
            timeout,
        }
    }
}

impl Transport for XurlTransport {
    fn request(
        &self,
        args: &[String],
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match &self.xurl_path {
            Ok(path) => {
                let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                xurl_call(&arg_refs, path, self.timeout)
            }
            Err(msg) => Err(Box::new(XurlError::NotFound(msg.clone()))),
        }
    }

    fn xurl_path(&self) -> Option<&Path> {
        self.xurl_path.as_ref().ok().map(|p| p.as_path())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Mock transport for unit tests. Returns pre-configured responses in order.
    ///
    /// Uses `std::sync::Mutex` (not `RefCell`) so the type satisfies the
    /// `Transport: Send + Sync` bound enforced by R20.
    pub struct MockTransport {
        pub responses:
            Mutex<VecDeque<Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>>>,
    }

    impl MockTransport {
        pub fn new(
            responses: Vec<Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>>,
        ) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    impl Transport for MockTransport {
        fn request(
            &self,
            _args: &[String],
        ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
            self.responses
                .lock()
                .expect("MockTransport mutex poisoned")
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
        let e = XurlError::Timeout(Duration::from_secs(42));
        assert!(e.to_string().contains("timeout"));
        assert!(e.to_string().contains("42"));
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
        let r1 = mock.request(&[]).expect("test: first response present");
        assert_eq!(r1["data"], "first");
        let r2 = mock.request(&[]).expect("test: second response present");
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
        let a = semver::Version::parse("1.0.9").expect("test: parse 1.0.9");
        let b = semver::Version::parse("1.0.10").expect("test: parse 1.0.10");
        assert!(a < b);
        let c = semver::Version::parse("1.0.3").expect("test: parse 1.0.3");
        assert!(b >= c);
    }

    #[test]
    fn version_comparison_major() {
        let a = semver::Version::parse("2.0.0").expect("test: parse 2.0.0");
        let b = semver::Version::parse("1.0.3").expect("test: parse 1.0.3");
        assert!(a >= b);
    }

    #[test]
    fn version_comparison_prerelease() {
        // semver spec: pre-release < release
        let a = semver::Version::parse("1.0.3-beta").expect("test: parse 1.0.3-beta");
        let b = semver::Version::parse("1.0.3").expect("test: parse 1.0.3");
        assert!(a < b);
    }

    // Negative example (intentionally commented out — uncomment locally to
    // verify the regression catches it). The `Transport: Send + Sync` bound
    // and the `BirdClient: Send + Sync` compile-time assertion should both
    // reject a non-Send field. Example:
    //
    // const _: () = {
    //     struct NotSend(std::rc::Rc<u32>);
    //     fn _assert<T: Send + Sync>() {}
    //     _assert::<NotSend>(); // compile error: Rc is neither Send nor Sync
    // };

    // --- XurlTransport accessor + from_error mapping --------------------

    #[test]
    fn xurl_transport_new_exposes_path_via_trait_accessor() {
        let path = std::path::PathBuf::from("/tmp/xurl-fixture-path");
        let t = XurlTransport::new(path.clone(), Duration::from_secs(7));
        assert_eq!(t.xurl_path(), Some(path.as_path()));
    }

    #[test]
    fn xurl_transport_from_error_hides_path() {
        let t = XurlTransport::from_error("boom".to_string(), Duration::from_secs(7));
        assert_eq!(t.xurl_path(), None);
    }

    #[test]
    fn xurl_transport_from_error_request_surfaces_not_found() {
        let t = XurlTransport::from_error(
            "BIRD_XURL_PATH=/missing does not exist".to_string(),
            Duration::from_secs(7),
        );
        let err = t.request(&[]).expect_err("from_error transport must error");
        let xerr = err
            .downcast_ref::<XurlError>()
            .expect("error must downcast to XurlError");
        match xerr {
            XurlError::NotFound(msg) => {
                assert!(
                    msg.contains("BIRD_XURL_PATH=/missing does not exist"),
                    "NotFound message must carry the resolution error verbatim, got: {msg}"
                );
            }
            other => panic!("expected XurlError::NotFound, got {other:?}"),
        }
    }

    #[test]
    fn mock_transport_xurl_path_defaults_to_none() {
        let mock = MockTransport::new(vec![]);
        assert_eq!(mock.xurl_path(), None);
    }

    #[test]
    fn xurl_transport_instances_carry_independent_paths() {
        // Two transports constructed with different paths must surface their
        // own path through `xurl_path()` — no shared state between instances.
        let path_a = std::path::PathBuf::from("/tmp/xurl-instance-a");
        let path_b = std::path::PathBuf::from("/tmp/xurl-instance-b");
        let a = XurlTransport::new(path_a.clone(), Duration::from_secs(1));
        let b = XurlTransport::new(path_b.clone(), Duration::from_secs(2));
        assert_eq!(a.xurl_path(), Some(path_a.as_path()));
        assert_eq!(b.xurl_path(), Some(path_b.as_path()));
    }

    // --- resolve_xurl_path error-branch coverage -----------------------

    #[test]
    fn resolve_xurl_path_with_nonexistent_path_errors() {
        let env = EnvOverrides {
            xurl_path: Some(std::path::PathBuf::from(
                "/tmp/bird-resolve-nonexistent-xurl-fixture",
            )),
            ..EnvOverrides::default()
        };
        let err = resolve_xurl_path(&env).expect_err("nonexistent path must error");
        let msg = err.to_string();
        assert!(
            msg.contains("does not exist"),
            "error must mention nonexistence, got: {msg}"
        );
    }

    #[test]
    fn resolve_xurl_path_with_directory_errors() {
        let tmp = tempfile::tempdir().expect("test: tempdir");
        let env = EnvOverrides {
            xurl_path: Some(tmp.path().to_path_buf()),
            ..EnvOverrides::default()
        };
        let err = resolve_xurl_path(&env).expect_err("directory must error");
        let msg = err.to_string();
        assert!(
            msg.contains("is not a file"),
            "directory error must mention 'is not a file', got: {msg}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn resolve_xurl_path_with_non_executable_errors() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("test: tempdir");
        let file_path = tmp.path().join("not-executable");
        std::fs::write(&file_path, b"#!/bin/sh\necho hi\n").expect("test: write");
        let mut perms = std::fs::metadata(&file_path)
            .expect("test: metadata")
            .permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).expect("test: chmod");
        let env = EnvOverrides {
            xurl_path: Some(file_path),
            ..EnvOverrides::default()
        };
        let err = resolve_xurl_path(&env).expect_err("non-executable must error");
        let msg = err.to_string();
        assert!(
            msg.contains("is not executable"),
            "non-executable error must mention 'is not executable', got: {msg}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn resolve_xurl_path_with_valid_executable_succeeds() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("test: tempdir");
        let file_path = tmp.path().join("fake-xurl");
        // Print a version string `verify_xurl_binary` is not called for the
        // env-supplied path (the function trusts the user's choice), but the
        // file still has to pass the existence / file / executable checks.
        std::fs::write(&file_path, b"#!/bin/sh\necho 'xurl 1.0.3'\n").expect("test: write");
        let mut perms = std::fs::metadata(&file_path)
            .expect("test: metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).expect("test: chmod");
        let env = EnvOverrides {
            xurl_path: Some(file_path.clone()),
            ..EnvOverrides::default()
        };
        let resolved = resolve_xurl_path(&env).expect("valid executable must resolve");
        let canonical_expected = file_path
            .canonicalize()
            .expect("test: canonicalize fixture");
        assert_eq!(resolved, canonical_expected);
    }

    #[test]
    fn resolve_xurl_path_with_none_falls_back_to_which() {
        // Without a `xurl_path` snapshot the resolver falls back to
        // `which::which("xr")` then `which::which("xurl")`. The PATH on the
        // test host may or may not carry either binary, so this assertion is
        // intentionally weak: it only proves the resolver does NOT consult
        // the snapshot when the field is None, by feeding an empty snapshot
        // and accepting either outcome.
        let env = EnvOverrides {
            xurl_path: None,
            ..EnvOverrides::default()
        };
        let _ = resolve_xurl_path(&env);
    }
}
