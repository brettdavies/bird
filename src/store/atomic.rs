//! Write-then-rename for every store file: a file lands whole or not at all.
//!
//! The temporary file sits in the target's own directory, so the rename never
//! crosses a filesystem, and is named `.bird-<random>.tmp`, which notes apps
//! hide. New files are 0600 and new directories 0700 on Unix, because the
//! archive holds the user's bookmarks.

use std::io::{self, Write};
use std::path::Path;

/// The prefix and suffix every temporary store file carries.
pub const TEMP_PREFIX: &str = ".bird-";
/// See [`TEMP_PREFIX`].
pub const TEMP_SUFFIX: &str = ".tmp";

/// Whether a write is flushed to disk before its rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    /// `sync_all` before the rename: a crash cannot leave an empty file in
    /// place of the user's annotations or spend history.
    Synced,
    /// No flush: the next observation recreates the file.
    Unsynced,
}

/// Writes `bytes` to `target` through a temporary file in its directory,
/// creating missing parent directories.
pub fn write(target: &Path, bytes: &[u8], durability: Durability) -> io::Result<()> {
    write_checked(target, bytes, durability, || Ok(()))
}

/// [`write()`], running `before_rename` once the temporary file is complete.
/// An error from it drops the temporary file and leaves `target` untouched.
pub fn write_checked(
    target: &Path,
    bytes: &[u8],
    durability: Durability,
    before_rename: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let dir = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", target.display()),
        )
    })?;
    create_dir(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(TEMP_SUFFIX)
        .tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    if durability == Durability::Synced {
        tmp.as_file().sync_all()?;
    }
    before_rename()?;
    tmp.persist(target).map_err(|e| e.error)?;
    Ok(())
}

/// Creates `dir` and its missing parents, each new one private on Unix.
pub fn create_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// Whether `name` is a temporary file a store write left behind.
pub fn is_temp_name(name: &str) -> bool {
    name.starts_with(TEMP_PREFIX) && name.ends_with(TEMP_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_interrupted_write_leaves_the_old_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("note.md");
        write(&target, b"old", Durability::Synced).expect("first write");
        let err = write_checked(&target, b"new and partial", Durability::Synced, || {
            Err(io::Error::other("stopped before the rename"))
        })
        .expect_err("the rename is never reached");
        assert_eq!(err.to_string(), "stopped before the rename");
        assert_eq!(std::fs::read(&target).expect("read"), b"old");
        assert_eq!(names(dir.path()), ["note.md"]);
    }

    #[test]
    fn a_write_never_leaves_a_visible_temp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("posts").join("1.md");
        write(&target, b"one", Durability::Unsynced).expect("write");
        write(&target, b"two", Durability::Synced).expect("rewrite");
        assert_eq!(names(&dir.path().join("posts")), ["1.md"]);
        assert_eq!(std::fs::read(&target).expect("read"), b"two");
    }

    #[test]
    fn a_name_is_appended_never_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("spend.jsonl");
        write(&target, b"{}", Durability::Unsynced).expect("write");
        assert_eq!(names(dir.path()), ["spend.jsonl"]);
    }

    #[test]
    fn temp_names_are_recognized() {
        assert!(is_temp_name(".bird-a1B2c3.tmp"));
        assert!(!is_temp_name("bird-a1.tmp"));
        assert!(!is_temp_name(".bird-a1.json"));
    }

    #[cfg(unix)]
    #[test]
    fn new_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("a").join("b").join("note.md");
        write(&target, b"x", Durability::Unsynced).expect("write");
        let mode = |p: &Path| std::fs::metadata(p).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode(&target), 0o600);
        assert_eq!(mode(&dir.path().join("a")), 0o700);
        assert_eq!(mode(&dir.path().join("a").join("b")), 0o700);
    }
}
