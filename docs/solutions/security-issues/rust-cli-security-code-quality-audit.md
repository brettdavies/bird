---
title: "Rust CLI Security & Code Quality Audit Fixes"
problem_type: code-quality
severity: critical
date_resolved: "2026-02-10"
tags:
  - security
  - rust
  - code-quality
  - dead-code
  - xss
  - file-permissions
  - error-handling
  - performance
files_changed:
  - src/main.rs
  - src/auth.rs
  - src/login.rs
  - src/raw.rs
  - src/bookmarks.rs
  - src/config.rs
  - src/doctor.rs
  - src/output.rs
  - src/requirements.rs
  - src/schema.rs
  - Cargo.toml
related_docs:
  - docs/CLI_DESIGN.md
  - docs/DEVELOPER.md
  - docs/SECRETS.md
---

# Rust CLI Security & Code Quality Audit Fixes

## Problem

A comprehensive code review of the bird CLI (X/Twitter API client) identified 15 findings across security, performance, architecture, code patterns, simplicity, and agent-native parity. The findings ranged from critical (XSS vulnerability, world-readable token files) to low (dead code, compiler warnings).

## Root Cause

The codebase was in its initial v1 state with common patterns seen in early-stage Rust CLI projects: scattered HTTP client construction, no timeouts, dead code behind `#[allow(dead_code)]`, duplicated logic across modules, and inconsistent error handling with `eprintln!` + `std::process::exit(1)`.

## Solution

All 15 findings were fixed across 7 dependency-ordered batches. Each batch was verified with `cargo check` (0 errors, 0 warnings) and `cargo test` (6/6 pass) before committing.

### Batch 1: Dead Code Cleanup + Trivial Fixes (#012, #013, #014)

**Dead code removal:**
- Deleted `SchemaPaths`, `load_schema_paths`, `path_params_for_path` from `schema.rs`
- Deleted unused `API_BASE` constant from `config.rs`
- Deleted unused `accepts()` function and `command_name` field from `requirements.rs`
- Deleted unused `option()` and `accent()` helpers from `output.rs`
- Removed all `#[allow(dead_code)]` annotations

**Added `--version` flag:**
```rust
#[command(name = "bird", about = "X API CLI", version)]
struct Cli { ... }
```

**Minimized tokio features:**
```toml
# Before
tokio = { version = "1", features = ["full"] }

# After
tokio = { version = "1", features = ["macros", "rt", "net", "io-util", "sync", "time"] }
```

Changed runtime to single-threaded:
```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode { ... }
```

### Batch 2: Shared HTTP Client with Timeouts (#002, #003)

Created a single `reqwest::Client` in `main()` with 30s timeouts, passed through to all command handlers:

```rust
let client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(30))
    .timeout(Duration::from_secs(30))
    .build()
    .expect("failed to build HTTP client");
```

Removed 5 separate `Client::new()` calls across `raw.rs`, `bookmarks.rs`, `auth.rs`, `login.rs`, and `main.rs`. Note: `reqwest_oauth1::oauth1()` takes ownership of `Client`, so `client.clone()` is needed for OAuth1 paths (cheap since `Client` is internally `Arc`-based).

### Batch 3: Login Hardening + XSS Fix (#004, #005)

**XSS fix:** Replaced dynamic error interpolation with static HTML:
```rust
// Before (vulnerable)
format!("<html><body>Authorization failed: {}</body></html>", err)

// After (safe)
"<html><body>Authorization failed. Check the terminal for details.</body></html>"
```

**Login timeout:** Wrapped `rx.await` with 120s timeout:
```rust
let result = tokio::time::timeout(Duration::from_secs(120), rx).await;
match result {
    Ok(Ok(inner)) => inner,
    Ok(Err(_)) => Err("login callback channel closed".into()),
    Err(_) => Err("login timed out after 120 seconds (no browser callback received)".into()),
}
```

### Batch 4: Auth Security (#001, #010, #011)

**Token file permissions (Unix 0o600):**
```rust
use std::os::unix::fs::OpenOptionsExt;

let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(path)?;
file.write_all(s.as_bytes())?;
```

**Deduplicated token exchange/refresh:** Extracted a shared `post_token()` helper that handles form body construction, optional Basic auth, and response parsing.

**Redacted secrets in Debug output:** Custom `Debug` implementations for `OAuth2Account`, `TokenResponse`, and `ResolvedConfig` that print `[REDACTED]` for secret fields instead of exposing token values.

### Batch 5: Replace run_me + Path Validation (#006, #008)

**Replaced `run_me` with `run_raw`:** Deleted the 40-line bespoke `run_me()` function and replaced the `Me` command dispatch with:
```rust
Command::Me { pretty } => {
    let params = HashMap::new();
    raw::run_raw(client, &config, "GET", "/2/users/me", &params, &[], None, pretty)
        .await
        .map_err(|e| BirdError::Command { name: "me", source: e })?;
}
```

**Path parameter validation:** Added `validate_param_value()` to reject injection attempts:
```rust
fn validate_param_value(name: &str, value: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if value.is_empty() {
        return Err(format!("path parameter '{}' must not be empty", name).into());
    }
    if !value.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err(format!(
            "path parameter '{}' contains invalid characters (only alphanumeric, underscore, hyphen, dot allowed): {}",
            name, value
        ).into());
    }
    Ok(())
}
```

### Batch 6: Bookmarks Memory Optimization (#009)

Rewrote bookmarks to stream pages to stdout instead of collecting into a `Vec`:

```rust
// Manual JSON array wrapping with streaming
if pretty { println!("{{\n  \"data\": ["); } else { print!("{{\"data\":["); }

loop {
    // ... fetch page ...
    for item in data {
        if !first_item { /* comma */ }
        first_item = false;
        // Print each item immediately
        if pretty {
            let s = serde_json::to_string_pretty(item)?;
            for line in s.lines() { println!("    {}", line); }
        } else {
            print!("{}", serde_json::to_string(item)?);
        }
    }
}

if pretty { println!("\n  ]\n}}"); } else { println!("]}}"); }
```

This reduces peak memory from O(total_bookmarks) to O(page_size).

### Batch 7: Structured Errors + AuthType Unification (#007, #015)

**Unified AuthType enums:** Added `None` variant to `requirements::AuthType` with serde rename attributes, deleted the duplicate `doctor::AuthType` enum:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AuthType {
    #[serde(rename = "oauth2_user")]
    OAuth2User,
    #[serde(rename = "oauth1")]
    OAuth1,
    #[serde(rename = "bearer")]
    Bearer,
    #[serde(rename = "none")]
    None,
}
```

**Structured error handling:** Created `BirdError` enum with distinct exit codes:
```rust
enum BirdError {
    Config(Box<dyn std::error::Error + Send + Sync>),       // exit 78 (EX_CONFIG)
    Auth(requirements::AuthRequiredError),                    // exit 77 (EX_NOPERM)
    Command { name: &'static str, source: Box<dyn ...> },   // exit 1
}
```

Extracted `run()` function from `main()` that returns `Result<(), BirdError>`, centralizing all error display and exit code logic.

## Errors Encountered During Implementation

1. **`reqwest_oauth1` ownership:** `client.oauth1(secrets)` takes ownership of `Client`. Fixed with `client.clone()` (cheap: internally `Arc`-based).
2. **AuthType naming mismatch:** `doctor.rs` used `Oauth2User`/`Oauth1` while `requirements.rs` uses `OAuth2User`/`OAuth1`. Fixed with `replace_all` edits.
3. **Partial struct move:** Initially tried passing full `Cli` to `run()` after fields were moved to overrides. Fixed by having `run()` accept `Command` instead.

## Verification

- `cargo check`: 0 errors, 0 warnings
- `cargo test`: 6/6 tests passing
- 8 commits total (7 batches + 1 cleanup), all pushed to `origin/development`

## Prevention Strategies

### Code Review Checklist

- [ ] No `#[allow(dead_code)]` without a tracking issue
- [ ] HTTP clients are shared, not constructed per-request
- [ ] All HTTP clients have connect + request timeouts
- [ ] No user-controlled input interpolated into HTML responses
- [ ] Token/credential files use restrictive permissions (0o600)
- [ ] Debug impls redact secret fields
- [ ] Error types carry structured data, not just strings
- [ ] Streaming output for paginated endpoints (no unbounded memory)
- [ ] Path/URL parameters validated before substitution

### CI/CD Recommendations

- Add `cargo clippy -- -D warnings` to CI pipeline
- Add `cargo fmt --check` for formatting consistency
- Run `cargo test` on every PR
- Consider `cargo audit` for dependency vulnerability scanning

### Testing Gaps to Address

- Unit tests for `resolve_path` template substitution
- Unit tests for `resolve_oauth2_token` priority chain
- Unit tests for `make_code_verifier` / `make_code_challenge` PKCE
- Integration tests with mock HTTP servers for pagination
- Tests for structured error exit codes
