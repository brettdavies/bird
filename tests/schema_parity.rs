//! Parity test: embedded schema bytes must equal on-disk source bytes.
//!
//! The `schema/*.schema.json` files are the build-time source of truth; the
//! `bird schema` subcommand emits them via `include_str!`. A divergence
//! (someone hand-edits the embedded literal, or the disk file gets out of
//! sync) would silently break agents that pinned against the published schema
//! URL. This test catches it at CI time.

use std::fs;
use std::path::PathBuf;

/// Each entry is `(name, embedded_bytes)`. The disk file is found at
/// `schema/<name>.schema.json` relative to `CARGO_MANIFEST_DIR`.
const EMBEDDED: &[(&str, &str)] = &[
    ("bookmarks", include_str!("../schema/bookmarks.schema.json")),
    ("doctor", include_str!("../schema/doctor.schema.json")),
    (
        "error-envelope",
        include_str!("../schema/error-envelope.schema.json"),
    ),
    ("profile", include_str!("../schema/profile.schema.json")),
    ("raw-get", include_str!("../schema/raw-get.schema.json")),
    ("search", include_str!("../schema/search.schema.json")),
    (
        "success-envelope",
        include_str!("../schema/success-envelope.schema.json"),
    ),
    ("thread", include_str!("../schema/thread.schema.json")),
    ("usage", include_str!("../schema/usage.schema.json")),
    ("watchlist", include_str!("../schema/watchlist.schema.json")),
];

fn schema_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema")
}

#[test]
fn embedded_bytes_match_disk_bytes() {
    let dir = schema_dir();
    for (name, embedded) in EMBEDDED {
        let path = dir.join(format!("{}.schema.json", name));
        let disk =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
        assert_eq!(
            disk,
            *embedded,
            "schema {} disk bytes differ from embedded bytes (path: {})",
            name,
            path.display()
        );
    }
}

#[test]
fn every_disk_schema_file_is_embedded() {
    let dir = schema_dir();
    let mut on_disk: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {}", dir.display(), e))
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".schema.json").map(str::to_string)
        })
        .collect();
    on_disk.sort();

    let mut embedded: Vec<String> = EMBEDDED.iter().map(|(n, _)| n.to_string()).collect();
    embedded.sort();

    assert_eq!(
        on_disk, embedded,
        "every schema/*.schema.json file on disk must be embedded by EMBEDDED \
         (and vice versa) — add or remove entries to keep them in sync"
    );
}
