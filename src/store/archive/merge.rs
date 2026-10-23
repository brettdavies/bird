//! Rewriting a note that exists: bird's keys and region update, and every
//! other frontmatter entry and line of text stays byte for byte.
//!
//! The edit check is the file's SHA-256, taken at read and again just before
//! the rename, because synced and network filesystems keep mtimes to the
//! second.

use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::alias;
use super::frontmatter::{Stored, region_parts, split};
use super::note::{Note, region_block};
use super::yaml::{Field, entry};
use crate::store::atomic::{self, Durability};

/// What a note write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The note did not exist and was written.
    Created,
    /// The stored note was merged and replaced.
    Rewritten,
    /// The stored note already said everything; nothing was written.
    Unchanged,
    /// A relation note existed and is never rewritten.
    Kept,
}

/// Why a stored note was left as it is.
#[derive(Debug)]
pub enum MergeError {
    /// Reading or writing failed.
    Io(io::Error),
    /// The frontmatter does not parse, or its line split disagrees with the
    /// parse.
    Unparseable(String),
    /// The file changed between the read and the rename.
    Changed,
}

/// Marks the edit-check failure inside the `io::Error` the rename hook
/// returns.
#[derive(Debug)]
struct NoteChanged;

impl std::fmt::Display for NoteChanged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the note changed while bird merged it")
    }
}

impl std::error::Error for NoteChanged {}

impl MergeError {
    /// The error as the store warning names it.
    pub fn to_io(&self) -> io::Error {
        match self {
            Self::Io(e) => io::Error::new(e.kind(), e.to_string()),
            Self::Unparseable(why) => io::Error::new(io::ErrorKind::InvalidData, why.clone()),
            Self::Changed => io::Error::other(NoteChanged),
        }
    }
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

/// Writes `note` at `path` as first written or rewritten at `stamp`.
pub fn write_note(path: &Path, note: &Note, stamp: &str) -> Result<Outcome, MergeError> {
    write_note_with(path, note, stamp, || {})
}

/// [`write_note`], running `before_check` just before the edit check.
pub(crate) fn write_note_with(
    path: &Path,
    note: &Note,
    stamp: &str,
    before_check: impl FnOnce(),
) -> Result<Outcome, MergeError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            atomic::write(path, note.render(stamp).as_bytes(), Durability::Unsynced)
                .map_err(MergeError::Io)?;
            return Ok(Outcome::Created);
        }
        Err(e) => return Err(MergeError::Io(e)),
    };
    if note.kind.create_only() {
        return Ok(Outcome::Kept);
    }
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| MergeError::Unparseable("the note is not UTF-8".into()))?;
    let stored = split(&text).map_err(MergeError::Unparseable)?;
    let Some(merged) = merge(note, &stored, stamp) else {
        return Ok(Outcome::Unchanged);
    };
    let hash = sha256(&bytes);
    let check = || {
        before_check();
        match std::fs::read(path) {
            Ok(now) if sha256(&now) == hash => Ok(()),
            Ok(_) => Err(io::Error::other(NoteChanged)),
            Err(e) => Err(e),
        }
    };
    match atomic::write_checked(path, merged.as_bytes(), Durability::Synced, check) {
        Ok(()) => Ok(Outcome::Rewritten),
        Err(e) if e.get_ref().is_some_and(|inner| inner.is::<NoteChanged>()) => {
            Err(MergeError::Changed)
        }
        Err(e) => Err(MergeError::Io(e)),
    }
}

/// The merged file, or `None` when bird's keys and region already match.
fn merge(note: &Note, stored: &Stored<'_>, stamp: &str) -> Option<String> {
    let raw_of = |key: &str| {
        stored
            .entries
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .map(|e| e.raw.to_string())
    };
    let (before, region, after) = region_parts(stored.body);
    let rendered = note.region(Some(stored));
    let mut changed = region != Some(rendered.as_str());
    let mut bird = Vec::new();
    for &key in note.kind.keys() {
        let value = match key {
            "archived_at" | "updated_at" => None,
            "bird_format" => Some(Field::Int(super::note::BIRD_FORMAT)),
            "aliases" => alias::merged(note, stored),
            _ => note.value(key, Some(stored)),
        };
        if let Some(v) = &value
            && stored.parsed.get(key) != Some(&v.to_json())
        {
            changed = true;
        }
        bird.push((key, value));
    }
    if !changed {
        return None;
    }
    let mut fm = String::new();
    for (key, value) in bird {
        let rendered = match (key, value) {
            ("archived_at", _) => {
                raw_of(key).or_else(|| Some(entry(key, &Field::Str(stamp.into()))))
            }
            ("updated_at", _) => Some(entry(key, &Field::Str(stamp.into()))),
            (_, Some(v)) => Some(entry(key, &v)),
            (_, None) => raw_of(key),
        };
        fm.push_str(&rendered.unwrap_or_default());
    }
    let owned = note.kind.keys();
    for e in &stored.entries {
        if e.key.as_deref().is_none_or(|k| !owned.contains(&k)) {
            fm.push_str(e.raw);
        }
    }
    Some(format!(
        "---\n{fm}---\n{before}{}{after}",
        region_block(&rendered)
    ))
}
