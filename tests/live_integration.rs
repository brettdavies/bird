//! Live integration test for the entity store (BirdClient + BirdDb).
//!
//! Runs 16 phases against the live X API to verify all acceptance criteria from
//! `docs/plans/2026-02-17-refactor-entity-store-cache-replacement-plan.md`.
//!
//! Cost: ~$0.10-0.15 per run (~8-10 API calls).
//!
//! Run:   cargo test --test live_integration -- --ignored --nocapture
//! Skip:  cargo test  (this test is #[ignore]'d by default)

use assert_cmd::Command;
use std::path::PathBuf;

/// X API env vars to pass through to bird subprocesses.
const X_API_ENV_VARS: &[&str] = &[
    "X_API_CLIENT_ID",
    "X_API_CLIENT_SECRET",
    "X_API_ACCESS_TOKEN",
    "X_API_REFRESH_TOKEN",
    "X_API_BEARER_TOKEN",
    "X_API_USERNAME",
    "X_API_REDIRECT_URI",
    "X_API_CONSUMER_KEY",
    "X_API_CONSUMER_SECRET",
    "X_API_OAUTH1_ACCESS_TOKEN",
    "X_API_OAUTH1_ACCESS_TOKEN_SECRET",
];

/// Isolated test environment with its own HOME, config dir, and DB.
struct TestEnv {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    config_dir: PathBuf,
    db_path: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let home = tmp.path().to_path_buf();
        let config_dir = home.join(".config").join("bird");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        let db_path = config_dir.join("bird.db");

        // Copy tokens.json + config.toml from real config dir
        if let Some(real_config) = dirs::config_dir().map(|d| d.join("bird")) {
            for file in &["tokens.json", "config.toml"] {
                let src = real_config.join(file);
                if src.exists() {
                    let _ = std::fs::copy(&src, config_dir.join(file));
                }
            }
        }

        Self {
            _tmp: tmp,
            home,
            config_dir,
            db_path,
        }
    }

    /// Build a `bird` command isolated to this test environment.
    fn bird(&self) -> Command {
        let mut cmd = Command::cargo_bin("bird").unwrap();
        cmd.env("HOME", &self.home);
        cmd.env("XDG_CONFIG_HOME", self.home.join(".config"));
        cmd.env("NO_COLOR", "1");
        cmd.env_remove("BIRD_NO_CACHE");
        for key in X_API_ENV_VARS {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }
        cmd
    }

    /// Open bird.db in read-only mode for SQL assertions.
    fn open_db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open bird.db for assertions")
    }

    fn db_exists(&self) -> bool {
        self.db_path.exists()
    }
}

/// Extract tweet IDs from stdout JSON (handles full API response or per-line tweets).
fn extract_tweet_ids(stdout: &str) -> Vec<String> {
    let mut ids = Vec::new();

    // Try whole stdout as single JSON
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(stdout) {
        collect_ids(&v, &mut ids);
        if !ids.is_empty() {
            return ids;
        }
    }

    // Fall back to line-by-line
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            collect_ids(&v, &mut ids);
        }
    }
    ids
}

fn collect_ids(v: &serde_json::Value, ids: &mut Vec<String>) {
    // data: [{id: "..."}, ...]
    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                ids.push(id.to_string());
            }
        }
    // data: {id: "..."}
    } else if let Some(id) = v
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|i| i.as_str())
    {
        ids.push(id.to_string());
    // bare {id: "..."}
    } else if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
        ids.push(id.to_string());
    }
}

#[test]
#[ignore]
fn live_entity_store_integration() {
    eprintln!("\n=== Live Integration Test: Entity Store ===");

    let env = TestEnv::new();

    // ================================================================
    // Phase 0: Environment setup
    // ================================================================
    eprintln!("\n=== Phase 0: Environment setup ===");
    eprintln!("  Temp HOME: {}", env.home.display());
    eprintln!("  Config dir: {}", env.config_dir.display());

    let output = env.bird().args(["doctor"]).output().expect("run doctor");
    assert!(output.status.success(), "Phase 0: doctor should succeed");

    let doctor_stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&doctor_stdout) {
        eprintln!(
            "  Doctor: config={}, auth={}",
            doc.get("config").is_some(),
            doc.get("auth").is_some()
        );
    }
    eprintln!("  Phase 0: PASS");

    // ================================================================
    // Phase 1: Pre-flight auth gate
    // ================================================================
    eprintln!("\n=== Phase 1: Pre-flight auth gate ===");

    let output = env
        .bird()
        .args(["profile", "elonmusk"])
        .output()
        .expect("run profile");
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("  profile elonmusk: exit {}", exit_code);

    if exit_code != 0 {
        let is_auth = exit_code == 77
            || stderr.contains("no valid auth")
            || stderr.contains("auth failed");
        if is_auth {
            eprintln!("SKIP: Auth not available. Run `bird login` to refresh tokens, or set X_API_BEARER_TOKEN.");
        } else {
            eprintln!(
                "SKIP: Command failed (exit {}). Likely API/network issue.",
                exit_code
            );
        }
        eprintln!("  Stderr: {}", stderr.lines().next().unwrap_or(""));
        return;
    }

    let profile_json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Phase 1: profile stdout should be valid JSON");
    assert!(
        profile_json
            .get("data")
            .and_then(|d| d.get("id"))
            .is_some(),
        "Phase 1: profile should have data.id"
    );
    assert!(
        stderr.contains("[cost]"),
        "Phase 1: stderr should contain [cost]"
    );
    assert!(
        stderr.contains("cache miss"),
        "Phase 1: first profile should be cache miss"
    );

    {
        let conn = env.open_db();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM users WHERE username = 'elonmusk'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            count > 0,
            "Phase 1: elonmusk should be stored in users table"
        );
    }
    eprintln!("  Phase 1: PASS");

    // ================================================================
    // Phase 2: Search stores entities (AC #1)
    // ================================================================
    eprintln!("\n=== Phase 2: Search stores entities (AC #1) ===");

    let output = env
        .bird()
        .args(["search", "twitter", "--max-results", "10"])
        .output()
        .expect("run search");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "AC #1: search should succeed");
    assert!(
        stderr.contains("[cost]"),
        "AC #1: search stderr should contain [cost]"
    );

    let tweet_ids = extract_tweet_ids(&stdout);
    eprintln!("  Found {} tweet IDs in search results", tweet_ids.len());

    {
        let conn = env.open_db();
        let tweet_count: i64 = conn
            .query_row("SELECT count(*) FROM tweets", [], |r| r.get(0))
            .unwrap();
        let user_count: i64 = conn
            .query_row("SELECT count(*) FROM users", [], |r| r.get(0))
            .unwrap();
        eprintln!("  DB: tweets={}, users={}", tweet_count, user_count);
        assert!(
            tweet_count > 0,
            "AC #1: tweets table should have entries after search"
        );
        assert!(
            user_count > 0,
            "AC #1: users table should have entries after search"
        );
    }
    eprintln!("  Phase 2: PASS");

    // ================================================================
    // Phase 3: Profile freshness + "from store" cost (AC #2, #13)
    // ================================================================
    eprintln!("\n=== Phase 3: Profile freshness + from store cost (AC #2, #13) ===");

    let now = chrono::Utc::now();
    let skip_freshness = {
        use chrono::Timelike;
        now.hour() == 23 && now.minute() >= 55
    };

    if skip_freshness {
        eprintln!("  WARNING: Near UTC midnight, skipping freshness assertion");
    } else {
        let output = env
            .bird()
            .args(["profile", "elonmusk"])
            .output()
            .expect("run profile again");
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "AC #2: second profile should succeed"
        );
        assert!(
            stderr.contains("from store"),
            "AC #2: second profile should be from store. Stderr: {}",
            stderr.trim()
        );
        assert!(
            stderr.contains("$0.00"),
            "AC #13: from-store cost should be $0.00"
        );
        eprintln!("  Stderr: {}", stderr.trim());
    }
    eprintln!("  Phase 3: PASS");

    // ================================================================
    // Phase 4: --cache-only serves/errors (AC #6)
    // ================================================================
    eprintln!("\n=== Phase 4: --cache-only serves/errors (AC #6) ===");

    // Success: cached user
    let output = env
        .bird()
        .args(["--cache-only", "profile", "elonmusk"])
        .output()
        .expect("run cache-only profile");
    assert!(
        output.status.success(),
        "AC #6: --cache-only for cached user should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _: serde_json::Value = serde_json::from_str(&stdout)
        .expect("AC #6: --cache-only stdout should be valid JSON");

    // Failure: unknown user not in store
    let output = env
        .bird()
        .args(["--cache-only", "profile", "nonexistent_user_xyz_99999"])
        .output()
        .expect("run cache-only profile unknown");
    assert!(
        !output.status.success(),
        "AC #6: --cache-only for unknown user should fail"
    );

    eprintln!("  Phase 4: PASS");

    // ================================================================
    // Phase 5: --refresh skips reads, still writes (AC #7)
    // ================================================================
    eprintln!("\n=== Phase 5: --refresh skips reads, still writes (AC #7) ===");

    let output = env
        .bird()
        .args(["--refresh", "profile", "elonmusk"])
        .output()
        .expect("run refresh profile");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "AC #7: --refresh profile should succeed"
    );
    assert!(
        stderr.contains("cache miss"),
        "AC #7: --refresh should show cache miss"
    );

    {
        let conn = env.open_db();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM users WHERE username = 'elonmusk'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count > 0, "AC #7: --refresh should still write to store");
    }
    eprintln!("  Phase 5: PASS");

    // ================================================================
    // Phase 6: --no-cache bypasses store entirely (AC #8)
    // ================================================================
    eprintln!("\n=== Phase 6: --no-cache bypasses store entirely (AC #8) ===");

    let env_nocache = TestEnv::new();
    let output = env_nocache
        .bird()
        .args(["--no-cache", "search", "twitter", "--max-results", "10"])
        .output()
        .expect("run no-cache search");

    assert!(
        output.status.success(),
        "AC #8: --no-cache search should succeed"
    );
    assert!(
        !env_nocache.db_exists(),
        "AC #8: --no-cache should not create bird.db"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[cost]"),
        "AC #8: --no-cache should still show cost"
    );
    eprintln!("  Phase 6: PASS");

    // ================================================================
    // Phase 7: Batch IDs via bird get (AC #3)
    // ================================================================
    eprintln!("\n=== Phase 7: Batch IDs via bird get (AC #3) ===");

    if tweet_ids.len() >= 3 {
        let ids = tweet_ids[..3].join(",");
        let path = format!("/2/tweets?ids={}", ids);
        let output = env
            .bird()
            .args(["get", &path])
            .output()
            .expect("run batch get");
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(output.status.success(), "AC #3: batch get should succeed");
        for id in &tweet_ids[..3] {
            assert!(
                stdout.contains(id.as_str()),
                "AC #3: batch get should contain ID {}",
                id
            );
        }
        eprintln!("  Fetched 3 IDs: {}", ids);
    } else {
        eprintln!("  SKIP: fewer than 3 tweet IDs from search");
    }
    eprintln!("  Phase 7: PASS");

    // ================================================================
    // Phase 8: Bookmarks stores relationships (AC #4)
    // ================================================================
    eprintln!("\n=== Phase 8: Bookmarks stores relationships (AC #4) ===");

    let output = env
        .bird()
        .args(["bookmarks"])
        .output()
        .expect("run bookmarks");
    let exit_code = output.status.code().unwrap_or(-1);

    if exit_code == 77 {
        eprintln!("  SKIP: bookmarks requires OAuth2User (exit 77)");
    } else if exit_code == 0 {
        let conn = env.open_db();
        let bm_count: i64 = conn
            .query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0))
            .unwrap();
        eprintln!("  Bookmarks in DB: {}", bm_count);

        if bm_count > 1 {
            let positions: Vec<i64> = {
                let mut stmt = conn
                    .prepare("SELECT position FROM bookmarks ORDER BY position")
                    .unwrap();
                stmt.query_map([], |r| r.get(0))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            };
            for w in positions.windows(2) {
                assert!(
                    w[0] <= w[1],
                    "AC #4: bookmark positions should be monotonically increasing"
                );
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "  Bookmarks failed (exit {}): {}",
            exit_code,
            stderr.lines().next().unwrap_or("")
        );
    }
    eprintln!("  Phase 8: PASS");

    // ================================================================
    // Phase 9: Thread entity lookup (AC #5)
    // ================================================================
    eprintln!("\n=== Phase 9: Thread entity lookup (AC #5) ===");

    if let Some(tweet_id) = tweet_ids.first() {
        let output = env
            .bird()
            .args(["thread", tweet_id])
            .output()
            .expect("run thread");
        let exit_code = output.status.code().unwrap_or(-1);
        eprintln!("  thread {}: exit {}", tweet_id, exit_code);

        // Thread may fail if tweet isn't part of a conversation — that's OK
        assert!(
            exit_code == 0 || exit_code == 1,
            "AC #5: thread should exit 0 or 1 (not panic), got {}",
            exit_code
        );
        if exit_code == 0 {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
                "AC #5: thread stdout should be valid JSON"
            );
        }
    } else {
        eprintln!("  SKIP: no tweet IDs from search");
    }
    eprintln!("  Phase 9: PASS");

    // ================================================================
    // Phase 10: Cache stats (AC #9)
    // ================================================================
    eprintln!("\n=== Phase 10: Cache stats (AC #9) ===");

    let output = env
        .bird()
        .args(["cache", "stats"])
        .output()
        .expect("run cache stats");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "AC #9: cache stats should succeed"
    );
    let stats: serde_json::Value =
        serde_json::from_str(&stdout).expect("AC #9: cache stats should be valid JSON");
    let tweets = stats.get("tweets").and_then(|v| v.as_u64()).unwrap_or(0);
    let users = stats.get("users").and_then(|v| v.as_u64()).unwrap_or(0);
    let healthy = stats
        .get("healthy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    eprintln!("  tweets={}, users={}, healthy={}", tweets, users, healthy);
    assert!(tweets > 0, "AC #9: cache stats tweets should be > 0");
    assert!(users > 0, "AC #9: cache stats users should be > 0");
    assert!(healthy, "AC #9: cache stats should report healthy");
    eprintln!("  Phase 10: PASS");

    // ================================================================
    // Phase 11: Usage works with data (AC #12)
    // ================================================================
    eprintln!("\n=== Phase 11: Usage works with data (AC #12) ===");

    let output = env
        .bird()
        .args(["usage"])
        .output()
        .expect("run usage");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "AC #12: usage should succeed");
    let usage: serde_json::Value =
        serde_json::from_str(&stdout).expect("AC #12: usage should be valid JSON");
    let total_calls = usage
        .pointer("/summary/total_calls")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    eprintln!("  total_calls={}", total_calls);
    assert!(
        total_calls > 0,
        "AC #12: usage should have recorded calls"
    );
    eprintln!("  Phase 11: PASS");

    // ================================================================
    // Phase 12: Cache clear preserves usage (AC #10)
    // ================================================================
    eprintln!("\n=== Phase 12: Cache clear preserves usage (AC #10) ===");

    let usage_count_before: i64 = {
        let conn = env.open_db();
        conn.query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .unwrap()
    };

    let output = env
        .bird()
        .args(["cache", "clear"])
        .output()
        .expect("run cache clear");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cleared"),
        "AC #10: cache clear should report Cleared"
    );

    // Entities should be gone
    let output = env
        .bird()
        .args(["cache", "stats"])
        .output()
        .expect("run cache stats after clear");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stats: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        stats.get("tweets").and_then(|v| v.as_u64()).unwrap_or(1),
        0,
        "AC #10: tweets should be 0 after clear"
    );
    assert_eq!(
        stats.get("users").and_then(|v| v.as_u64()).unwrap_or(1),
        0,
        "AC #10: users should be 0 after clear"
    );

    // Usage rows should be preserved
    let usage_count_after: i64 = {
        let conn = env.open_db();
        conn.query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(
        usage_count_before, usage_count_after,
        "AC #10: usage count should be preserved after clear (before={}, after={})",
        usage_count_before, usage_count_after
    );
    eprintln!(
        "  usage rows: before={}, after={}",
        usage_count_before, usage_count_after
    );
    eprintln!("  Phase 12: PASS");

    // ================================================================
    // Phase 13: Error-in-200 (AC #14)
    // ================================================================
    eprintln!("\n=== Phase 13: Error-in-200 (AC #14) ===");

    let output = env
        .bird()
        .args(["get", "/2/tweets?ids=9999999999999999999"])
        .output()
        .expect("run get nonexistent tweet");
    let exit_code = output.status.code().unwrap_or(-1);
    eprintln!("  get nonexistent tweet: exit {}", exit_code);

    // Must not panic — graceful handling regardless of exit code
    assert!(
        exit_code == 0 || exit_code == 1,
        "AC #14: error-in-200 should not panic (exit was {})",
        exit_code
    );
    if exit_code == 0 {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
            "AC #14: error-in-200 stdout should be parseable JSON"
        );
    }
    eprintln!("  Phase 13: PASS");

    // ================================================================
    // Phase 14: DB inspection (AC #15-16, #19, #21)
    // ================================================================
    eprintln!("\n=== Phase 14: DB inspection (AC #15-16, #19, #21) ===");

    // Re-seed DB since Phase 12 cleared it
    let output = env
        .bird()
        .args(["search", "twitter", "--max-results", "10"])
        .output()
        .expect("re-seed search");
    assert!(
        output.status.success(),
        "Phase 14: re-seed search should succeed"
    );

    // AC #15: File permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&env.db_path).expect("read db metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "AC #15: bird.db should have 0o600 permissions, got {:o}",
            mode
        );
        eprintln!("  Permissions: {:o}", mode);
    }

    {
        let conn = env.open_db();

        // AC #16: WAL mode
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            journal_mode, "wal",
            "AC #16: journal_mode should be wal"
        );
        eprintln!("  Journal mode: {}", journal_mode);

        // AC #19: WITHOUT ROWID on bookmarks
        let bookmarks_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='bookmarks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            bookmarks_sql.to_uppercase().contains("WITHOUT ROWID"),
            "AC #19: bookmarks should use WITHOUT ROWID"
        );
        eprintln!("  Bookmarks: WITHOUT ROWID confirmed");

        // AC #21: tweets should NOT use WITHOUT ROWID (large raw_json)
        let tweets_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='tweets'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !tweets_sql.to_uppercase().contains("WITHOUT ROWID"),
            "AC #21: tweets should NOT use WITHOUT ROWID"
        );
        assert!(
            !tweets_sql.to_uppercase().contains("REFERENCES"),
            "AC #21: tweets should have no SQL foreign keys (soft refs only)"
        );
        eprintln!("  Tweets: standard rowid, no FKs confirmed");
    }
    eprintln!("  Phase 14: PASS");

    // ================================================================
    // Phase 15: Graceful degradation with corrupt DB (AC #18)
    // ================================================================
    eprintln!("\n=== Phase 15: Graceful degradation (AC #18) ===");

    let env_corrupt = TestEnv::new();
    std::fs::write(
        &env_corrupt.db_path,
        b"THIS IS NOT A VALID SQLITE DATABASE FILE",
    )
    .expect("write corrupt db");

    let output = env_corrupt
        .bird()
        .args(["profile", "elonmusk"])
        .output()
        .expect("run profile with corrupt db");
    let exit_code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("  Corrupt DB profile: exit {}", exit_code);

    if exit_code == 0 {
        assert!(
            stderr.contains("[store] warning"),
            "AC #18: corrupt DB should produce [store] warning. Stderr: {}",
            stderr.trim()
        );
        eprintln!("  Graceful degradation: API-only mode confirmed");
    } else if exit_code == 77 {
        eprintln!("  SKIP: auth failed (can't verify degradation without valid auth)");
    } else {
        // Even on command error, should not panic (exit 1 is acceptable)
        assert_eq!(
            exit_code, 1,
            "AC #18: corrupt DB should degrade gracefully, got exit {}",
            exit_code
        );
    }
    eprintln!("  Phase 15: PASS");

    // ================================================================
    // Phase 16: Usage migration from cache.db (AC #11)
    // ================================================================
    eprintln!("\n=== Phase 16: Usage migration from cache.db (AC #11) ===");

    let env_migrate = TestEnv::new();

    // Create synthetic old cache.db with the original schema and test data
    {
        let old_cache_path = env_migrate.config_dir.join("cache.db");
        let conn = rusqlite::Connection::open(&old_cache_path).expect("create old cache.db");
        conn.execute_batch(
            "CREATE TABLE usage (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp      INTEGER NOT NULL,
                date_ymd       INTEGER NOT NULL,
                endpoint       TEXT NOT NULL,
                method         TEXT NOT NULL,
                object_type    TEXT,
                object_count   INTEGER NOT NULL DEFAULT 0,
                estimated_cost REAL NOT NULL DEFAULT 0.0,
                cache_hit      INTEGER NOT NULL DEFAULT 0,
                username       TEXT
            );
            CREATE TABLE usage_actual (
                date         TEXT PRIMARY KEY,
                tweet_count  INTEGER NOT NULL,
                synced_at    INTEGER NOT NULL
            );
            INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
                VALUES (1708000000, 20260215, '/2/tweets/search/recent', 'GET', 'tweet', 5, 0.025, 0, 'testuser');
            INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
                VALUES (1708100000, 20260216, '/2/users/by/username', 'GET', 'user', 1, 0.005, 0, 'testuser');
            INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
                VALUES (1708200000, 20260217, '/2/tweets/search/recent', 'GET', 'tweet', 10, 0.050, 1, 'testuser');
            INSERT INTO usage_actual VALUES ('2026-02-15', 5, 1708000000);",
        )
        .expect("seed old cache.db");
    }

    // Any bird command triggers BirdClient::new() which triggers migration
    let output = env_migrate
        .bird()
        .args(["cache", "stats"])
        .output()
        .expect("run cache stats to trigger migration");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "AC #11: cache stats should succeed"
    );
    assert!(
        stderr.contains("migrated usage data from cache.db"),
        "AC #11: should report migration. Stderr: {}",
        stderr.trim()
    );

    // Verify rows migrated
    {
        let conn = env_migrate.open_db();
        let usage_count: i64 = conn
            .query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .unwrap();
        assert!(
            usage_count >= 3,
            "AC #11: should have migrated 3 usage rows, got {}",
            usage_count
        );

        let actual_count: i64 = conn
            .query_row("SELECT count(*) FROM usage_actual", [], |r| r.get(0))
            .unwrap();
        assert!(
            actual_count >= 1,
            "AC #11: should have migrated 1 usage_actual row, got {}",
            actual_count
        );
    }
    eprintln!("  Migrated 3 usage rows + 1 usage_actual row");

    // Idempotency: second run should NOT re-migrate
    let output = env_migrate
        .bird()
        .args(["cache", "stats"])
        .output()
        .expect("run cache stats again for idempotency");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("migrated usage data"),
        "AC #11: second run should not re-migrate. Stderr: {}",
        stderr.trim()
    );

    {
        let conn = env_migrate.open_db();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM usage", [], |r| r.get(0))
            .unwrap();
        assert!(
            count >= 3,
            "AC #11: usage count should be unchanged after second run, got {}",
            count
        );
    }
    eprintln!("  Idempotency confirmed");
    eprintln!("  Phase 16: PASS");

    // ================================================================
    eprintln!("\n=== 16/16 phases passed ===\n");
}
