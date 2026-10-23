//! bird's file store: a Markdown archive, per-account JSON response files,
//! and a JSONL spend ledger.

pub mod archive;
pub mod atomic;
pub mod client;
pub mod endpoint;
pub mod file_store;
pub mod ledger;
pub mod paths;
pub mod report;
pub mod responses;
pub mod spend_view;
pub mod status;

pub use client::{BirdClient, CacheOpts, RequestContext, StoreSetup};
pub use file_store::{FileStore, StoreAccount};
pub use paths::{ArchiveDirError, ArchiveSource, StorePaths};
pub use status::{Area, Failure, StoreStatus};
