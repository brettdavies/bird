//! bird's file store: a Markdown archive, per-account JSON response files,
//! and a JSONL spend ledger.

pub mod atomic;
pub mod paths;
pub mod status;

pub use paths::{ArchiveDirError, ArchiveSource, StorePaths};
pub use status::{Area, Failure, StoreStatus};
