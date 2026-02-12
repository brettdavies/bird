//! Cache layer: SQLite-backed HTTP response cache with transparent CachedClient wrapper.
//! BirdDb is the application database — cache now, usage tracking in Plan 4.
//! Cache failures are never fatal: the Option<BirdDb> pattern degrades to no-cache mode.

use crate::requirements::AuthType;
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn migrations() -> Migrations<'static> {
    // IMPORTANT: Never modify existing migrations. Only append new ones.
    Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS cache (
                key         TEXT PRIMARY KEY,
                url         TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                body        BLOB NOT NULL,
                body_size   INTEGER NOT NULL,
                created_at  INTEGER NOT NULL,
                ttl_seconds INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cache_created ON cache(created_at);",
        ),
        // Migration 2: usage tracking table (per-API-call cost logging)
        M::up(
            "CREATE TABLE IF NOT EXISTS usage (
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
            CREATE INDEX IF NOT EXISTS idx_usage_ymd_endpoint_cache ON usage(date_ymd, endpoint, cache_hit);
            CREATE INDEX IF NOT EXISTS idx_usage_endpoint ON usage(endpoint);",
        ),
        // Migration 3: actual usage from X API (for --sync comparison)
        M::up(
            "CREATE TABLE IF NOT EXISTS usage_actual (
                date         TEXT PRIMARY KEY,
                tweet_count  INTEGER NOT NULL,
                synced_at    INTEGER NOT NULL
            );",
        ),
    ])
}

/// Application database: cache storage + future usage tracking (Plan 4).
/// Single connection per CLI invocation — no pool needed (short-lived process).
/// Blocking SQLite calls are fine: single-threaded tokio runtime.
pub struct BirdDb {
    conn: Connection,
    write_count: u32,
    max_bytes: u64,
}

impl BirdDb {
    /// Open (or create) the cache database at the given path.
    /// Pre-creates the file with 0o600 permissions, then opens with rusqlite.
    pub fn open(path: &Path, max_size_mb: u64) -> Result<Self, rusqlite::Error> {
        Self::ensure_file_permissions(path);

        let mut conn = Connection::open(path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA temp_store = MEMORY;",
        )?;

        // Reject tampered databases with triggers
        let trigger_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='trigger'",
            [],
            |r| r.get(0),
        )?;
        if trigger_count > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                Some("database contains unexpected triggers".into()),
            ));
        }

        migrations().to_latest(&mut conn).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("migration failed: {}", e)),
            )
        })?;

        Ok(Self {
            conn,
            write_count: 0,
            max_bytes: max_size_mb * 1024 * 1024,
        })
    }

    /// Pre-create file with 0o600 so WAL/SHM sidecars inherit restrictive permissions.
    fn ensure_file_permissions(path: &Path) {
        if path.exists() {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)
                .and_then(|mut f| f.write_all(b""));
        }
        #[cfg(not(unix))]
        {
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path);
        }
    }

    /// Look up a cached response by key. Returns None if not found or expired.
    pub fn get(&self, key: &str) -> Result<Option<CacheEntry>, rusqlite::Error> {
        let now = unix_now();
        let mut stmt = self.conn.prepare_cached(
            "SELECT url, status_code, body, created_at, ttl_seconds
             FROM cache WHERE key = ?1 AND (created_at + ttl_seconds) > ?2",
        )?;
        let result = stmt.query_row(params![key, now], |row| {
            Ok(CacheEntry {
                url: row.get(0)?,
                status_code: row.get(1)?,
                body: row.get(2)?,
                created_at: row.get(3)?,
                ttl_seconds: row.get(4)?,
            })
        });
        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Insert or replace a cache entry.
    pub fn put(
        &mut self,
        key: &str,
        url: &str,
        status_code: u16,
        body: &[u8],
        ttl_seconds: i64,
    ) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let body_size = body.len() as i64;
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO cache (key, url, status_code, body, body_size, created_at, ttl_seconds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        stmt.execute(params![
            key,
            url,
            status_code,
            body,
            body_size,
            now,
            ttl_seconds
        ])?;

        self.write_count += 1;
        if self.write_count.is_multiple_of(20) {
            self.prune_if_needed()?;
        }
        Ok(())
    }

    /// Remove entries past their TTL.
    pub fn delete_expired(&self) -> Result<usize, rusqlite::Error> {
        let now = unix_now();
        self.conn.execute(
            "DELETE FROM cache WHERE (created_at + ttl_seconds) <= ?1",
            params![now],
        )
    }

    /// Delete oldest entries until total body_size is under the limit.
    fn prune_if_needed(&self) -> Result<(), rusqlite::Error> {
        // First expire anything past TTL
        self.delete_expired()?;

        let total: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(body_size), 0) FROM cache", [], |r| {
                    r.get(0)
                })?;

        if total as u64 <= self.max_bytes {
            return Ok(());
        }

        // Delete oldest entries until under 90% of limit
        let target = (self.max_bytes as f64 * 0.9) as i64;
        self.conn.execute(
            "DELETE FROM cache WHERE key IN (
                SELECT key FROM cache ORDER BY created_at ASC
                LIMIT (SELECT count(*) FROM cache) -
                    (SELECT count(*) FROM cache WHERE key IN (
                        SELECT key FROM (
                            SELECT key, SUM(body_size) OVER (ORDER BY created_at DESC) AS running
                            FROM cache
                        ) WHERE running <= ?1
                    ))
            )",
            params![target],
        )?;
        Ok(())
    }

    /// Cache statistics for `bird cache stats` and `bird doctor`.
    pub fn stats(&self) -> Result<CacheStats, rusqlite::Error> {
        let entry_count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM cache", [], |r| r.get(0))?;
        let total_size: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(body_size), 0) FROM cache", [], |r| {
                    r.get(0)
                })?;
        let now = unix_now();
        let oldest: Option<i64> = self
            .conn
            .query_row("SELECT MIN(created_at) FROM cache", [], |r| r.get(0))
            .ok();
        let newest: Option<i64> = self
            .conn
            .query_row("SELECT MAX(created_at) FROM cache", [], |r| r.get(0))
            .ok();

        Ok(CacheStats {
            entry_count: entry_count as u64,
            total_size_bytes: total_size as u64,
            max_size_bytes: self.max_bytes,
            oldest_seconds_ago: oldest.map(|t| (now - t).max(0)),
            newest_seconds_ago: newest.map(|t| (now - t).max(0)),
        })
    }

    /// Delete all cache entries and reclaim space.
    pub fn clear(&self) -> Result<u64, rusqlite::Error> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM cache", [], |r| r.get(0))?;
        self.conn.execute("DELETE FROM cache", [])?;
        let _ = self.conn.execute_batch("PRAGMA incremental_vacuum;");
        Ok(count as u64)
    }

    /// Expose the DB file path for stats display.
    pub fn path(&self) -> Option<PathBuf> {
        self.conn.path().map(PathBuf::from)
    }

    // -- Usage tracking methods --

    /// Log an API call to the usage table for cost tracking.
    #[allow(dead_code)] // Public API for Plan 4 integration with CachedClient::get()
    pub fn log_usage(&mut self, entry: &UsageLogEntry<'_>) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        let date_ymd = {
            let dt = chrono::DateTime::from_timestamp(now, 0).unwrap();
            dt.format("%Y%m%d")
                .to_string()
                .parse::<i64>()
                .unwrap()
        };
        self.maybe_prune_usage(now)?;
        self.conn.execute(
            "INSERT INTO usage (timestamp, date_ymd, endpoint, method, object_type, object_count, estimated_cost, cache_hit, username)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                now,
                date_ymd,
                entry.endpoint,
                entry.method,
                entry.object_type,
                entry.object_count,
                entry.estimated_cost,
                entry.cache_hit as i32,
                entry.username
            ],
        )?;
        Ok(())
    }

    /// Prune usage rows older than 90 days. Called opportunistically every ~50 writes.
    fn maybe_prune_usage(&self, now_ts: i64) -> Result<(), rusqlite::Error> {
        let row_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM usage", [], |row| row.get(0))?;
        if row_count > 0 && row_count % 50 == 0 {
            let cutoff = now_ts - (90 * 24 * 60 * 60);
            self.conn
                .execute("DELETE FROM usage WHERE timestamp < ?1", [cutoff])?;
        }
        Ok(())
    }

    /// Query usage summary (totals) since a given YYYYMMDD date.
    pub fn query_usage_summary(&self, since_ymd: i64) -> Result<UsageSummary, rusqlite::Error> {
        self.conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END), 0.0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN cache_hit = 1 THEN estimated_cost ELSE 0 END), 0.0)
             FROM usage WHERE date_ymd >= ?1",
            [since_ymd],
            |row| {
                Ok(UsageSummary {
                    total_cost: row.get(0)?,
                    total_calls: row.get(1)?,
                    cache_hits: row.get(2)?,
                    estimated_savings: row.get(3)?,
                })
            },
        )
    }

    /// Query daily usage breakdown since a given YYYYMMDD date.
    pub fn query_daily_usage(&self, since_ymd: i64) -> Result<Vec<DailyUsage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT
                date_ymd,
                SUM(CASE WHEN cache_hit = 0 THEN estimated_cost ELSE 0 END),
                COUNT(*),
                SUM(CASE WHEN cache_hit = 1 THEN 1 ELSE 0 END)
             FROM usage
             WHERE date_ymd >= ?1
             GROUP BY date_ymd
             ORDER BY date_ymd DESC",
        )?;
        let rows = stmt.query_map([since_ymd], |row| {
            Ok(DailyUsage {
                date_ymd: row.get(0)?,
                cost: row.get(1)?,
                calls: row.get(2)?,
                cache_hits: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Query top endpoints by cost since a given YYYYMMDD date.
    pub fn query_top_endpoints(
        &self,
        since_ymd: i64,
    ) -> Result<Vec<EndpointUsage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT endpoint, SUM(estimated_cost), COUNT(*)
             FROM usage
             WHERE date_ymd >= ?1 AND cache_hit = 0
             GROUP BY endpoint
             ORDER BY SUM(estimated_cost) DESC
             LIMIT 10",
        )?;
        let rows = stmt.query_map([since_ymd], |row| {
            Ok(EndpointUsage {
                endpoint: row.get(0)?,
                cost: row.get(1)?,
                calls: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Upsert actual usage from X API (for --sync comparison).
    pub fn upsert_actual_usage(
        &self,
        date: &str,
        tweet_count: u64,
    ) -> Result<(), rusqlite::Error> {
        let now = unix_now();
        self.conn.execute(
            "INSERT OR REPLACE INTO usage_actual (date, tweet_count, synced_at)
             VALUES (?1, ?2, ?3)",
            params![date, tweet_count as i64, now],
        )?;
        Ok(())
    }

    /// Query actual usage data (from previous --sync operations).
    pub fn query_actual_usage(
        &self,
        since_ymd: i64,
    ) -> Result<Option<Vec<ActualUsageDay>>, rusqlite::Error> {
        let since_date = format!(
            "{}-{:02}-{:02}",
            since_ymd / 10000,
            (since_ymd % 10000) / 100,
            since_ymd % 100
        );
        let mut stmt = self.conn.prepare_cached(
            "SELECT date, tweet_count, synced_at FROM usage_actual
             WHERE date >= ?1
             ORDER BY date DESC",
        )?;
        let rows: Vec<ActualUsageDay> = stmt
            .query_map([&since_date], |row| {
                Ok(ActualUsageDay {
                    date: row.get(0)?,
                    tweet_count: row.get::<_, i64>(1)? as u64,
                    synced_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(rows))
        }
    }
}

// -- Usage data structures --

/// Entry for logging an API call to the usage table.
#[allow(dead_code)] // Public API for Plan 4 integration
pub struct UsageLogEntry<'a> {
    pub endpoint: &'a str,
    pub method: &'a str,
    pub object_type: Option<&'a str>,
    pub object_count: i64,
    pub estimated_cost: f64,
    pub cache_hit: bool,
    pub username: Option<&'a str>,
}

#[derive(Debug, serde::Serialize)]
pub struct UsageSummary {
    pub total_cost: f64,
    pub total_calls: i64,
    pub cache_hits: i64,
    pub estimated_savings: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct DailyUsage {
    pub date_ymd: i64,
    pub cost: f64,
    pub calls: i64,
    pub cache_hits: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct EndpointUsage {
    pub endpoint: String,
    pub cost: f64,
    pub calls: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct ActualUsageDay {
    pub date: String,
    pub tweet_count: u64,
    pub synced_at: Option<i64>,
}

impl Drop for BirdDb {
    fn drop(&mut self) {
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    }
}

/// A cached response entry.
#[derive(Debug)]
pub struct CacheEntry {
    #[allow(dead_code)] // used for debugging/logging
    pub url: String,
    pub status_code: i64,
    pub body: Vec<u8>,
    #[allow(dead_code)] // available for future cache inspection
    pub created_at: i64,
    #[allow(dead_code)] // available for future cache inspection
    pub ttl_seconds: i64,
}

/// Cache statistics.
#[derive(Debug, serde::Serialize)]
pub struct CacheStats {
    pub entry_count: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
    pub oldest_seconds_ago: Option<i64>,
    pub newest_seconds_ago: Option<i64>,
}

impl CacheStats {
    pub fn size_mb(&self) -> f64 {
        self.total_size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn max_size_mb(&self) -> f64 {
        self.max_size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn healthy(&self) -> bool {
        self.total_size_bytes < self.max_size_bytes
    }
}

/// Cache context for key computation (type-safe, not strings).
pub struct CacheContext<'a> {
    pub auth_type: &'a AuthType,
    pub username: Option<&'a str>,
}

/// Cache control options from CLI flags.
#[derive(Default)]
pub struct CacheOpts {
    pub no_cache: bool,
    pub refresh: bool,
    pub cache_ttl: Option<u64>,
}

// CacheOpts uses Default derive — all fields default to false/None

/// Response from CachedClient (covers both cache hits and fresh responses).
pub struct ApiResponse {
    pub status: reqwest::StatusCode,
    pub body: String,
    #[allow(dead_code)] // available for rate limit header inspection in Plans 2-4
    pub headers: reqwest::header::HeaderMap,
    pub cache_hit: bool,
}

impl fmt::Debug for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiResponse")
            .field("status", &self.status)
            .field("cache_hit", &self.cache_hit)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Transparent caching wrapper around reqwest::Client.
/// If BirdDb is unavailable (corrupted, disk error), degrades to direct HTTP.
pub struct CachedClient {
    http: reqwest::Client,
    db: Option<BirdDb>,
    cache_opts: CacheOpts,
}

impl CachedClient {
    /// Create a new CachedClient. If cache DB cannot be opened, degrades to no-cache.
    pub fn new(
        http: reqwest::Client,
        cache_path: &Path,
        cache_opts: CacheOpts,
        max_size_mb: u64,
    ) -> Self {
        if cache_opts.no_cache {
            return Self {
                http,
                db: None,
                cache_opts,
            };
        }
        let db = match BirdDb::open(cache_path, max_size_mb) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("[cache] warning: failed to open cache database: {}", e);
                eprintln!("[cache] Run `bird cache clear` to reset the cache database.");
                None
            }
        };
        Self {
            http,
            db,
            cache_opts,
        }
    }

    /// GET request with transparent caching.
    pub async fn get(
        &mut self,
        url: &str,
        ctx: &CacheContext<'_>,
        headers: reqwest::header::HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Never cache auth endpoints or paginated requests
        if should_skip_cache(url) || self.cache_opts.no_cache {
            return self.http_get(url, headers).await;
        }

        let key = compute_cache_key("GET", url, ctx);
        let ttl = self.effective_ttl(url);

        // Try cache read (unless --refresh)
        if !self.cache_opts.refresh {
            if let Some(ref db) = self.db {
                match db.get(&key) {
                    Ok(Some(entry)) => {
                        return Ok(ApiResponse {
                            status: reqwest::StatusCode::from_u16(entry.status_code as u16)
                                .unwrap_or(reqwest::StatusCode::OK),
                            body: String::from_utf8_lossy(&entry.body).into_owned(),
                            headers: reqwest::header::HeaderMap::new(),
                            cache_hit: true,
                        });
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("[cache] warning: read failed: {}", e);
                    }
                }
            }
        }

        // Cache miss — make HTTP request
        let response = self.http_get(url, headers).await?;

        // Write to cache (only 2xx responses)
        if response.status.is_success() {
            if let Some(ref mut db) = self.db {
                if let Err(e) = db.put(
                    &key,
                    url,
                    response.status.as_u16(),
                    response.body.as_bytes(),
                    ttl,
                ) {
                    eprintln!("[cache] warning: write failed: {}", e);
                }
            }
        }

        Ok(response)
    }

    /// POST/PUT/DELETE — pass-through, no caching.
    pub async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: Option<String>,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut req = self.http.request(method, url).headers(headers);
        if let Some(b) = body {
            req = req.body(b);
        }
        let res = req.send().await?;
        let status = res.status();
        let resp_headers = res.headers().clone();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            headers: resp_headers,
            cache_hit: false,
        })
    }

    /// Inner HTTP client ref (for auth operations that bypass cache).
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Get cache stats (None if cache unavailable).
    pub fn cache_stats(&self) -> Option<Result<CacheStats, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.stats())
    }

    /// Clear cache (None if cache unavailable).
    pub fn cache_clear(&self) -> Option<Result<u64, rusqlite::Error>> {
        self.db.as_ref().map(|db| db.clear())
    }

    /// Get the cache DB path.
    pub fn cache_path(&self) -> Option<PathBuf> {
        self.db.as_ref().and_then(|db| db.path())
    }

    /// Access the underlying BirdDb (for usage queries).
    pub fn db(&self) -> Option<&BirdDb> {
        self.db.as_ref()
    }

    /// Mutable access to the underlying BirdDb (for usage logging/writes).
    #[allow(dead_code)] // Public API for future usage logging integration
    pub fn db_mut(&mut self) -> Option<&mut BirdDb> {
        self.db.as_mut()
    }

    /// Direct HTTP GET (bypasses cache). Used for endpoints where fresh data is required.
    pub async fn http_get(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> Result<ApiResponse, Box<dyn std::error::Error + Send + Sync>> {
        let res = self.http.get(url).headers(headers).send().await?;
        let status = res.status();
        let resp_headers = res.headers().clone();
        let text = res.text().await?;
        Ok(ApiResponse {
            status,
            body: text,
            headers: resp_headers,
            cache_hit: false,
        })
    }

    fn effective_ttl(&self, url: &str) -> i64 {
        if let Some(ttl) = self.cache_opts.cache_ttl {
            // Cap at 24 hours to prevent stale-forever entries; safe i64 conversion
            return ttl.min(86400) as i64;
        }
        default_ttl_for_endpoint(url)
    }
}

/// Compute SHA-256 cache key from method + normalized URL + auth_type + username.
fn compute_cache_key(method: &str, url: &str, ctx: &CacheContext<'_>) -> String {
    let normalized = normalize_url(url);
    let auth_str = match ctx.auth_type {
        AuthType::OAuth2User => "oauth2_user",
        AuthType::OAuth1 => "oauth1",
        AuthType::Bearer => "bearer",
        AuthType::None => "none",
    };
    let username = ctx.username.unwrap_or("__app__");
    let input = format!("{}\0{}\0{}\0{}", method, normalized, auth_str, username);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(hash)
}

/// Normalize URL: sort query parameters, sort known ID lists.
fn normalize_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };

    debug_assert!(
        parsed.host_str() == Some("api.x.com"),
        "unexpected host: {:?}",
        parsed.host_str()
    );

    let mut pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    // Sort comma-separated values for known ID parameters
    for (key, value) in &mut pairs {
        if matches!(key.as_str(), "ids" | "usernames") {
            let mut parts: Vec<&str> = value.split(',').collect();
            parts.sort();
            *value = parts.join(",");
        }
    }

    // Sort by parameter key
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let path = parsed.path();
    if pairs.is_empty() {
        format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            path
        )
    } else {
        let query: String = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        format!(
            "{}://{}{}?{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            path,
            query
        )
    }
}

/// Whether to skip caching for this URL.
fn should_skip_cache(url: &str) -> bool {
    url.contains("/oauth2/token")
        || url.contains("pagination_token=")
        || url.contains("next_token=")
}

/// Per-endpoint TTL defaults (seconds). Most-specific pattern wins.
fn default_ttl_for_endpoint(url: &str) -> i64 {
    let path = url::Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_default();

    if path.starts_with("/2/users/") && path.contains("/bookmarks") {
        return 900; // 15 min
    }
    if path.starts_with("/2/tweets/search/") {
        return 900; // 15 min
    }
    if path.starts_with("/2/users/by/") || path.starts_with("/2/users/") {
        return 3600; // 1 hour
    }
    if path.starts_with("/2/tweets/") {
        return 900; // 15 min
    }
    900 // default 15 min
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// We need the `hex` encoding for SHA-256 output — use a minimal inline implementation
// to avoid adding another dependency.
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> BirdDb {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
        .unwrap();
        migrations().to_latest(&mut conn).unwrap();
        BirdDb {
            conn,
            write_count: 0,
            max_bytes: 100 * 1024 * 1024, // 100MB
        }
    }

    #[test]
    fn migrations_are_valid() {
        migrations().validate().unwrap();
    }

    #[test]
    fn put_and_get() {
        let mut db = in_memory_db();
        db.put(
            "key1",
            "https://api.x.com/2/tweets/123",
            200,
            b"hello",
            3600,
        )
        .unwrap();
        let entry = db.get("key1").unwrap().expect("should find entry");
        assert_eq!(entry.status_code, 200);
        assert_eq!(entry.body, b"hello");
        assert_eq!(entry.ttl_seconds, 3600);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let db = in_memory_db();
        assert!(db.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn expired_entries_not_returned() {
        let mut db = in_memory_db();
        // Insert with TTL of 0 (already expired)
        db.put("expired", "https://api.x.com/test", 200, b"old", 0)
            .unwrap();
        assert!(db.get("expired").unwrap().is_none());
    }

    #[test]
    fn delete_expired() {
        let mut db = in_memory_db();
        db.put("expired", "https://api.x.com/test", 200, b"old", 0)
            .unwrap();
        db.put("fresh", "https://api.x.com/test2", 200, b"new", 3600)
            .unwrap();
        let deleted = db.delete_expired().unwrap();
        assert_eq!(deleted, 1);
        assert!(db.get("fresh").unwrap().is_some());
    }

    #[test]
    fn clear_removes_all() {
        let mut db = in_memory_db();
        db.put("a", "https://api.x.com/1", 200, b"data1", 3600)
            .unwrap();
        db.put("b", "https://api.x.com/2", 200, b"data2", 3600)
            .unwrap();
        let count = db.clear().unwrap();
        assert_eq!(count, 2);
        assert!(db.get("a").unwrap().is_none());
        assert!(db.get("b").unwrap().is_none());
    }

    #[test]
    fn stats_reports_correctly() {
        let mut db = in_memory_db();
        db.put("a", "https://api.x.com/1", 200, b"hello", 3600)
            .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.total_size_bytes, 5);
        assert!(stats.healthy());
    }

    #[test]
    fn cache_key_includes_all_components() {
        let ctx1 = CacheContext {
            auth_type: &AuthType::OAuth2User,
            username: Some("alice"),
        };
        let ctx2 = CacheContext {
            auth_type: &AuthType::Bearer,
            username: Some("alice"),
        };
        let ctx3 = CacheContext {
            auth_type: &AuthType::OAuth2User,
            username: Some("bob"),
        };

        let key1 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx1);
        let key2 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx2);
        let key3 = compute_cache_key("GET", "https://api.x.com/2/users/me", &ctx3);

        // All should be different
        assert_ne!(
            key1, key2,
            "different auth_type should produce different keys"
        );
        assert_ne!(
            key1, key3,
            "different username should produce different keys"
        );
    }

    #[test]
    fn normalize_url_sorts_params() {
        let url = "https://api.x.com/2/tweets?ids=456,123&tweet.fields=text,author_id";
        let normalized = normalize_url(url);
        assert_eq!(
            normalized,
            "https://api.x.com/2/tweets?ids=123,456&tweet.fields=text,author_id"
        );
    }

    #[test]
    fn normalize_url_sorts_query_keys() {
        let url = "https://api.x.com/2/tweets?tweet.fields=text&ids=123";
        let normalized = normalize_url(url);
        assert_eq!(
            normalized,
            "https://api.x.com/2/tweets?ids=123&tweet.fields=text"
        );
    }

    #[test]
    fn normalize_url_no_query() {
        let url = "https://api.x.com/2/users/me";
        assert_eq!(normalize_url(url), url);
    }

    #[test]
    fn should_skip_oauth_and_pagination() {
        assert!(should_skip_cache("https://api.x.com/2/oauth2/token"));
        assert!(should_skip_cache(
            "https://api.x.com/2/users/123/bookmarks?pagination_token=abc"
        ));
        assert!(should_skip_cache(
            "https://api.x.com/2/tweets/search/recent?query=test&next_token=abc123"
        ));
        assert!(!should_skip_cache("https://api.x.com/2/users/me"));
    }

    #[test]
    fn ttl_defaults() {
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/me"),
            3600
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/by/username/jack"),
            3600
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/tweets/search/recent?query=test"),
            900
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/tweets/123"),
            900
        );
        assert_eq!(
            default_ttl_for_endpoint("https://api.x.com/2/users/123/bookmarks"),
            900
        );
        assert_eq!(default_ttl_for_endpoint("https://api.x.com/2/unknown"), 900);
    }

    #[test]
    fn hex_encode() {
        assert_eq!(hex::encode([0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(hex::encode([]), "");
    }

    #[test]
    fn api_response_debug_redacts_body() {
        let response = ApiResponse {
            status: reqwest::StatusCode::OK,
            body: "sensitive data here".to_string(),
            headers: reqwest::header::HeaderMap::new(),
            cache_hit: true,
        };
        let debug = format!("{:?}", response);
        assert!(!debug.contains("sensitive data here"));
        assert!(debug.contains("body_len"));
    }

    #[test]
    fn pruning_respects_size_limit() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .unwrap();
        migrations().to_latest(&mut conn).unwrap();
        let mut db = BirdDb {
            conn,
            write_count: 0,
            max_bytes: 100, // 100 bytes limit
        };

        // Insert entries totaling > 100 bytes
        db.put("a", "https://api.x.com/1", 200, &[0u8; 40], 3600)
            .unwrap();
        db.put("b", "https://api.x.com/2", 200, &[0u8; 40], 3600)
            .unwrap();
        db.put("c", "https://api.x.com/3", 200, &[0u8; 40], 3600)
            .unwrap();

        // Manually trigger prune
        db.prune_if_needed().unwrap();

        let stats = db.stats().unwrap();
        assert!(
            stats.total_size_bytes <= 100,
            "should be under limit after prune"
        );
    }
}
