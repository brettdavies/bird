//! Removes the 0.2.0 SQLite stores, `bird.db` and the older `cache.db`, each
//! with its `-wal` and `-shm` files, from bird's config directory.
//!
//! Nothing reads them, so they go on every run until they are gone, whatever
//! the cache flags say; a `--no-cache` habit would otherwise keep them
//! forever.

use std::io;
use std::path::{Path, PathBuf};

/// The 0.2.0 store files, newest first.
const STORES: [&str; 2] = ["bird.db", "cache.db"];

/// The SQLite sidecars beside each store.
const SIDECARS: [&str; 2] = ["-wal", "-shm"];

/// What one removal pass did.
#[derive(Debug, Default)]
pub struct Removal {
    /// Each store that had at least one file removed, by its main path.
    pub removed: Vec<PathBuf>,
    /// Each file that exists and could not be removed.
    pub failed: Vec<(PathBuf, io::Error)>,
}

/// Removes every 0.2.0 store file found in `config_dir`.
pub fn remove(config_dir: &Path) -> Removal {
    let mut removal = Removal::default();
    for store in STORES {
        let main = config_dir.join(store);
        let mut files = vec![main.clone()];
        files.extend(SIDECARS.iter().map(|suffix| {
            let mut name = main.clone().into_os_string();
            name.push(suffix);
            PathBuf::from(name)
        }));
        let mut any_removed = false;
        for file in files {
            match std::fs::remove_file(&file) {
                Ok(()) => any_removed = true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => removal.failed.push((file, e)),
            }
        }
        if any_removed {
            removal.removed.push(main);
        }
    }
    removal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(dir: &Path, names: &[&str]) {
        for name in names {
            std::fs::write(dir.join(name), b"sqlite").expect("seed");
        }
    }

    #[test]
    fn the_first_run_removes_bird_db_and_its_sidecars() {
        let dir = tempfile::tempdir().expect("tempdir");
        seed(
            dir.path(),
            &["bird.db", "bird.db-wal", "bird.db-shm", "config.toml"],
        );
        let removal = remove(dir.path());
        assert_eq!(removal.removed, vec![dir.path().join("bird.db")]);
        assert!(removal.failed.is_empty());
        for gone in ["bird.db", "bird.db-wal", "bird.db-shm"] {
            assert!(!dir.path().join(gone).exists(), "{gone} remains");
        }
        assert!(dir.path().join("config.toml").exists());
    }

    #[test]
    fn the_second_run_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        seed(dir.path(), &["bird.db"]);
        remove(dir.path());
        let again = remove(dir.path());
        assert!(again.removed.is_empty());
        assert!(again.failed.is_empty());
    }

    #[test]
    fn an_older_cache_db_is_removed_too() {
        let dir = tempfile::tempdir().expect("tempdir");
        seed(dir.path(), &["cache.db", "cache.db-wal"]);
        let removal = remove(dir.path());
        assert_eq!(removal.removed, vec![dir.path().join("cache.db")]);
        assert!(!dir.path().join("cache.db").exists());
        assert!(!dir.path().join("cache.db-wal").exists());
    }

    #[test]
    fn a_name_that_only_starts_like_a_store_stays() {
        let dir = tempfile::tempdir().expect("tempdir");
        seed(dir.path(), &["bird.db.bak", "my-cache.db"]);
        let removal = remove(dir.path());
        assert!(removal.removed.is_empty());
        assert!(dir.path().join("bird.db.bak").exists());
        assert!(dir.path().join("my-cache.db").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_removal_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        seed(dir.path(), &["bird.db"]);
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500))
            .expect("chmod");
        let removal = remove(dir.path());
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod back");
        assert!(removal.removed.is_empty());
        assert_eq!(removal.failed.len(), 1);
        assert_eq!(removal.failed[0].0, dir.path().join("bird.db"));
        assert_eq!(removal.failed[0].1.kind(), io::ErrorKind::PermissionDenied);
    }
}
