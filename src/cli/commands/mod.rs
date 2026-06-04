//! Per-command modules extracted from the dispatcher (`fn run`).
//!
//! Each submodule owns the arm body for one or more `Command::*` variants and
//! exposes a thin `pub fn run(...)` (or per-variant `pub fn run_<verb>`) that
//! the dispatcher delegates to.

pub mod bookmarks;
pub mod cache;
pub mod login;
pub mod profile;
pub mod raw_write;
pub mod reads;
pub mod search;
pub mod thread;
pub mod usage;
pub mod watchlist;
pub mod writes;
