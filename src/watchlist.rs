//! Watchlist command: manage and check a curated list of X users.
//! Config-driven (config.toml), uses toml_edit for formatting-preserving writes.

use crate::config::{FileConfig, ResolvedConfig};
use crate::db::{BirdClient, RequestContext};
use crate::diag;
use crate::fields;
use crate::requirements::AuthType;
use crate::schema;
use std::path::Path;
use toml_edit::{Array, DocumentMut, Item};

/// Load watchlist from config.toml. Returns empty vec if file missing.
fn load_watchlist(
    config_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let file_config: FileConfig = toml::from_str(&content)?;
    Ok(file_config.watchlist.unwrap_or_default())
}

/// Add a username to the watchlist in config.toml (idempotent, formatting-preserving).
fn add_to_watchlist(
    config_path: &Path,
    username: &str,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut doc = content.parse::<DocumentMut>()?;

    // Check for duplicates (case-insensitive)
    if let Some(existing) = doc.get("watchlist")
        && let Some(arr) = existing.as_array()
    {
        for val in arr.iter() {
            if val
                .as_str()
                .map(|u| u.eq_ignore_ascii_case(username))
                .unwrap_or(false)
            {
                diag!(quiet, "@{} is already in the watchlist.", username);
                return Ok(());
            }
        }
    }

    // Append to watchlist array (create if missing)
    if doc.get("watchlist").is_none() {
        doc.insert("watchlist", Item::Value(Array::new().into()));
    }
    doc["watchlist"].as_array_mut().unwrap().push(username);

    safe_write_config(config_path, &doc.to_string())?;
    Ok(())
}

/// Remove a username from the watchlist in config.toml (idempotent, formatting-preserving).
fn remove_from_watchlist(
    config_path: &Path,
    username: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let mut doc = content.parse::<DocumentMut>()?;

    let removed = if let Some(arr) = doc
        .get_mut("watchlist")
        .and_then(|item| item.as_array_mut())
    {
        let initial_len = arr.len();
        arr.retain(|v| {
            !v.as_str()
                .map(|u| u.eq_ignore_ascii_case(username))
                .unwrap_or(false)
        });
        initial_len != arr.len()
    } else {
        false
    };

    if removed {
        safe_write_config(config_path, &doc.to_string())?;
    }
    Ok(removed)
}

/// Atomically write config using tempfile + rename. Sets 0o600 permissions.
fn safe_write_config(
    config_path: &Path,
    content: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Write;
    use tempfile::Builder;

    let dir = config_path
        .parent()
        .ok_or("config path has no parent directory")?;
    std::fs::create_dir_all(dir)?;

    let mut builder = Builder::new();
    builder.prefix(".bird-config-").suffix(".tmp");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o600));
    }

    let mut tmp = builder.tempfile_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.as_file().sync_all()?;
    tmp.persist(config_path).map_err(|e| e.error)?;
    Ok(())
}

/// `bird watchlist list` — display the current watchlist as JSON.
pub fn run_watchlist_list(
    config: &ResolvedConfig,
    pretty: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_path = config.config_dir.join("config.toml");
    let entries = load_watchlist(&config_path)?;

    if entries.is_empty() {
        diag!(
            quiet,
            "Watchlist is empty. Add users with: bird watchlist add <username>"
        );
    }

    if pretty {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!("{}", serde_json::to_string(&entries)?);
    }
    Ok(())
}

/// `bird watchlist add <username>` — add a user to the watchlist (idempotent).
pub fn run_watchlist_add(
    config: &ResolvedConfig,
    username: &str,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    add_to_watchlist(&config_path, clean, quiet)?;
    diag!(quiet, "Added @{} to watchlist.", clean);
    Ok(())
}

/// `bird watchlist remove <username>` — remove a user from the watchlist (idempotent).
pub fn run_watchlist_remove(
    config: &ResolvedConfig,
    username: &str,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let clean = schema::validate_username(username)?;
    let config_path = config.config_dir.join("config.toml");
    let removed = remove_from_watchlist(&config_path, clean)?;
    if removed {
        diag!(quiet, "Removed @{} from watchlist.", clean);
    } else {
        diag!(quiet, "@{} was not in the watchlist.", clean);
    }
    Ok(())
}

/// `bird watchlist check` — check recent activity for all watched users.
/// Streams NDJSON (one JSON object per line) per user as they complete.
pub fn run_watchlist_check(
    client: &mut BirdClient,
    config: &ResolvedConfig,
    pretty: bool,
    use_color: bool,
    quiet: bool,
    auth_type: &AuthType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config_path = config.config_dir.join("config.toml");
    let entries = load_watchlist(&config_path)?;

    if entries.is_empty() {
        diag!(
            quiet,
            "Watchlist is empty. Add users with: bird watchlist add <username>"
        );
        return Ok(());
    }

    let ctx = RequestContext {
        auth_type,
        username: None,
    };

    use std::io::Write;
    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());

    let total = entries.len();
    for (i, username) in entries.iter().enumerate() {
        diag!(
            quiet,
            "[watchlist] checking @{} ({}/{})...",
            username,
            i + 1,
            total
        );

        let query = format!("from:{} -is:retweet", username);
        let search_url = build_check_url(&query);

        let activity = match execute_check(client, &ctx, &search_url, use_color, quiet) {
            Ok((tweet_count, latest_tweet, cache_hit)) => AccountActivity {
                username: username.clone(),
                recent_tweets: tweet_count,
                latest_tweet,
                cache_hit,
            },
            Err(e) => {
                diag!(quiet, "[watchlist] error checking @{}: {}", username, e);
                AccountActivity {
                    username: username.clone(),
                    recent_tweets: 0,
                    latest_tweet: None,
                    cache_hit: false,
                }
            }
        };

        if pretty {
            serde_json::to_writer_pretty(&mut writer, &activity)?;
        } else {
            serde_json::to_writer(&mut writer, &activity)?;
        }
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

fn build_check_url(query: &str) -> String {
    let mut url = url::Url::parse("https://api.x.com/2/tweets/search/recent").unwrap();
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("query", query);
        pairs.append_pair("max_results", "10");
        for (key, value) in fields::tweet_query_params() {
            pairs.append_pair(key, value);
        }
    }
    url.to_string()
}

fn execute_check(
    client: &mut BirdClient,
    ctx: &RequestContext<'_>,
    url: &str,
    use_color: bool,
    quiet: bool,
) -> Result<(u64, Option<LatestTweet>, bool), Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url, ctx)?;

    if !response.is_success() {
        return Err(format!(
            "GET search {}: {}",
            response.status,
            crate::output::sanitize_for_stderr(&response.body, 200)
        )
        .into());
    }

    let json = response.json.ok_or("invalid JSON from search")?;

    // Cost display per account
    let estimate = crate::cost::estimate_cost(&json, url, response.cache_hit);
    crate::cost::display_cost(&estimate, use_color, quiet);

    let tweet_count = json
        .get("meta")
        .and_then(|m| m.get("result_count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    let latest_tweet = extract_latest_tweet(&json);

    Ok((tweet_count, latest_tweet, response.cache_hit))
}

fn extract_latest_tweet(body: &serde_json::Value) -> Option<LatestTweet> {
    let data = body.get("data")?.as_array()?;
    let tweet = data.first()?;
    Some(LatestTweet {
        id: tweet.get("id")?.as_str()?.to_string(),
        text: tweet.get("text")?.as_str()?.to_string(),
        created_at: tweet
            .get("created_at")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string(),
        likes: tweet
            .get("public_metrics")
            .and_then(|m| m.get("like_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        retweets: tweet
            .get("public_metrics")
            .and_then(|m| m.get("retweet_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

#[derive(serde::Serialize)]
struct AccountActivity {
    username: String,
    recent_tweets: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_tweet: Option<LatestTweet>,
    cache_hit: bool,
}

#[derive(serde::Serialize)]
struct LatestTweet {
    id: String,
    text: String,
    created_at: String,
    likes: u64,
    retweets: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_config_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // -- load_watchlist tests --

    #[test]
    fn load_watchlist_missing_file() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let result = load_watchlist(&path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_watchlist_no_key() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "username = \"alice\"\n").unwrap();
        let result = load_watchlist(&path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_watchlist_with_entries() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\", \"bob\"]\n").unwrap();
        let result = load_watchlist(&path).unwrap();
        assert_eq!(result, vec!["alice", "bob"]);
    }

    // -- add_to_watchlist tests --

    #[test]
    fn add_to_new_config() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        add_to_watchlist(&path, "alice", false).unwrap();
        let entries = load_watchlist(&path).unwrap();
        assert_eq!(entries, vec!["alice"]);
    }

    #[test]
    fn add_to_existing_watchlist() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").unwrap();
        add_to_watchlist(&path, "bob", false).unwrap();
        let entries = load_watchlist(&path).unwrap();
        assert_eq!(entries, vec!["alice", "bob"]);
    }

    #[test]
    fn add_duplicate_is_idempotent() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").unwrap();
        add_to_watchlist(&path, "alice", false).unwrap();
        let entries = load_watchlist(&path).unwrap();
        assert_eq!(entries, vec!["alice"]);
    }

    #[test]
    fn add_duplicate_case_insensitive() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"Alice\"]\n").unwrap();
        add_to_watchlist(&path, "alice", false).unwrap();
        let entries = load_watchlist(&path).unwrap();
        assert_eq!(entries, vec!["Alice"]); // keeps original casing
    }

    // -- remove_from_watchlist tests --

    #[test]
    fn remove_existing_entry() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\", \"bob\"]\n").unwrap();
        let removed = remove_from_watchlist(&path, "alice").unwrap();
        assert!(removed);
        let entries = load_watchlist(&path).unwrap();
        assert_eq!(entries, vec!["bob"]);
    }

    #[test]
    fn remove_nonexistent_entry() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").unwrap();
        let removed = remove_from_watchlist(&path, "bob").unwrap();
        assert!(!removed);
    }

    #[test]
    fn remove_case_insensitive() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"Alice\"]\n").unwrap();
        let removed = remove_from_watchlist(&path, "alice").unwrap();
        assert!(removed);
        let entries = load_watchlist(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn remove_missing_config_file() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let removed = remove_from_watchlist(&path, "alice").unwrap();
        assert!(!removed);
    }

    // -- TOML preservation tests --

    #[test]
    fn add_preserves_comments() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let original = "# My bird config\nusername = \"bob\"\n# monitoring\n";
        fs::write(&path, original).unwrap();
        add_to_watchlist(&path, "alice", false).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# My bird config"));
        assert!(content.contains("# monitoring"));
        assert!(content.contains("username = \"bob\""));
        assert!(content.contains("alice"));
    }

    #[test]
    fn remove_preserves_comments() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let original = "# My config\nusername = \"bob\"\nwatchlist = [\"alice\", \"bob\"]\n";
        fs::write(&path, original).unwrap();
        remove_from_watchlist(&path, "alice").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# My config"));
        assert!(content.contains("username = \"bob\""));
        assert!(!content.contains("alice"));
        assert!(content.contains("bob"));
    }

    // -- File permissions tests --

    #[cfg(unix)]
    #[test]
    fn new_config_has_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        add_to_watchlist(&path, "alice", false).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    // -- extract_latest_tweet tests --

    #[test]
    fn extract_latest_tweet_empty_data() {
        let body = serde_json::json!({"data": [], "meta": {"result_count": 0}});
        assert!(extract_latest_tweet(&body).is_none());
    }

    #[test]
    fn extract_latest_tweet_with_data() {
        let body = serde_json::json!({
            "data": [{
                "id": "123",
                "text": "hello world",
                "created_at": "2026-02-11T10:00:00.000Z",
                "public_metrics": {
                    "like_count": 42,
                    "retweet_count": 5
                }
            }]
        });
        let tweet = extract_latest_tweet(&body).unwrap();
        assert_eq!(tweet.id, "123");
        assert_eq!(tweet.text, "hello world");
        assert_eq!(tweet.likes, 42);
        assert_eq!(tweet.retweets, 5);
    }

    #[test]
    fn extract_latest_tweet_no_data_key() {
        let body = serde_json::json!({"meta": {"result_count": 0}});
        assert!(extract_latest_tweet(&body).is_none());
    }
}
