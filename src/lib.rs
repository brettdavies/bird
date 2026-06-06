//! bird library surface — internal test infrastructure only.
//!
//! bird is distributed as a binary; the library surface is unstable and
//! intended for internal test infrastructure only. External consumers should
//! shell out to the `bird` binary, not import this crate.
//!
//! Concrete invocation patterns — `run_argv`, `run_with_paths` against a
//! `TempDir`-backed `ResolvedPaths`, captured writers, parsed JSON
//! envelopes — live under `examples/` and mirror the shapes used by
//! `tests/common/mod.rs`. They are reference code for someone debugging
//! the in-process test surface, not an endorsement of the library as a
//! downstream API.

#[doc(hidden)]
pub mod cli;

pub mod bookmarks;
pub mod config;
pub mod cost;
pub mod db;
pub mod doctor;
pub mod error;
pub mod fields;
pub mod login;
pub mod output;
pub mod profile;
pub mod raw;
pub mod schema;
pub mod schema_print;
pub mod search;
pub mod skill_install;
pub mod thread;
pub mod usage;
pub mod watchlist;
pub mod xurl_client;
