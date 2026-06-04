//! TOML-backed watchlist storage. Load/add/remove with `toml_edit` to preserve
//! formatting and comments; atomic write with 0o600 permissions.

use crate::config::FileConfig;
use std::path::Path;
use toml_edit::{Array, DocumentMut, Item};

/// Load watchlist from config.toml. Returns empty vec if file missing.
pub(super) fn load_watchlist(
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
pub(super) fn add_to_watchlist(
    config_path: &Path,
    username: &str,
    stderr: &mut dyn std::io::Write,
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
                if !quiet {
                    writeln!(stderr, "@{} is already in the watchlist.", username).ok();
                }
                return Ok(());
            }
        }
    }

    // Append to watchlist array (create if missing)
    if doc.get("watchlist").is_none() {
        doc.insert("watchlist", Item::Value(Array::new().into()));
    }
    let arr = doc["watchlist"]
        .as_array_mut()
        .ok_or("config 'watchlist' key is not an array")?;
    arr.push(username);

    safe_write_config(config_path, &doc.to_string())?;
    Ok(())
}

/// Remove a username from the watchlist in config.toml (idempotent, formatting-preserving).
pub(super) fn remove_from_watchlist(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_config_dir() -> TempDir {
        TempDir::new().expect("test")
    }

    #[test]
    fn load_watchlist_missing_file() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let result = load_watchlist(&path).expect("test");
        assert!(result.is_empty());
    }

    #[test]
    fn load_watchlist_no_key() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "username = \"alice\"\n").expect("test");
        let result = load_watchlist(&path).expect("test");
        assert!(result.is_empty());
    }

    #[test]
    fn load_watchlist_with_entries() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\", \"bob\"]\n").expect("test");
        let result = load_watchlist(&path).expect("test");
        assert_eq!(result, vec!["alice", "bob"]);
    }

    #[test]
    fn add_to_new_config() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        add_to_watchlist(&path, "alice", &mut std::io::sink(), false).expect("test");
        let entries = load_watchlist(&path).expect("test");
        assert_eq!(entries, vec!["alice"]);
    }

    #[test]
    fn add_to_existing_watchlist() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").expect("test");
        add_to_watchlist(&path, "bob", &mut std::io::sink(), false).expect("test");
        let entries = load_watchlist(&path).expect("test");
        assert_eq!(entries, vec!["alice", "bob"]);
    }

    #[test]
    fn add_duplicate_is_idempotent() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").expect("test");
        add_to_watchlist(&path, "alice", &mut std::io::sink(), false).expect("test");
        let entries = load_watchlist(&path).expect("test");
        assert_eq!(entries, vec!["alice"]);
    }

    #[test]
    fn add_duplicate_case_insensitive() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"Alice\"]\n").expect("test");
        add_to_watchlist(&path, "alice", &mut std::io::sink(), false).expect("test");
        let entries = load_watchlist(&path).expect("test");
        assert_eq!(entries, vec!["Alice"]);
    }

    #[test]
    fn remove_existing_entry() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\", \"bob\"]\n").expect("test");
        let removed = remove_from_watchlist(&path, "alice").expect("test");
        assert!(removed);
        let entries = load_watchlist(&path).expect("test");
        assert_eq!(entries, vec!["bob"]);
    }

    #[test]
    fn remove_nonexistent_entry() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"alice\"]\n").expect("test");
        let removed = remove_from_watchlist(&path, "bob").expect("test");
        assert!(!removed);
    }

    #[test]
    fn remove_case_insensitive() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        fs::write(&path, "watchlist = [\"Alice\"]\n").expect("test");
        let removed = remove_from_watchlist(&path, "alice").expect("test");
        assert!(removed);
        let entries = load_watchlist(&path).expect("test");
        assert!(entries.is_empty());
    }

    #[test]
    fn remove_missing_config_file() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let removed = remove_from_watchlist(&path, "alice").expect("test");
        assert!(!removed);
    }

    #[test]
    fn add_preserves_comments() {
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        let original = "# My bird config\nusername = \"bob\"\n# monitoring\n";
        fs::write(&path, original).expect("test");
        add_to_watchlist(&path, "alice", &mut std::io::sink(), false).expect("test");
        let content = fs::read_to_string(&path).expect("test");
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
        fs::write(&path, original).expect("test");
        remove_from_watchlist(&path, "alice").expect("test");
        let content = fs::read_to_string(&path).expect("test");
        assert!(content.contains("# My config"));
        assert!(content.contains("username = \"bob\""));
        assert!(!content.contains("alice"));
        assert!(content.contains("bob"));
    }

    #[cfg(unix)]
    #[test]
    fn new_config_has_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = setup_config_dir();
        let path = dir.path().join("config.toml");
        add_to_watchlist(&path, "alice", &mut std::io::sink(), false).expect("test");
        let metadata = fs::metadata(&path).expect("test");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
