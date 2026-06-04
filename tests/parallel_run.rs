//! Parallel-execution regression for the path-injection refactor (Plan 1 U9.17).
//!
//! Spawn 16 threads concurrently invoking [`bird::cli::run_with_paths`] via the
//! shared `tests/common/mod.rs` helpers. The previous `assert_cmd`-based
//! integration suite paid for isolation with per-test `HOME` /
//! `XDG_CONFIG_HOME` mutation, which made `cargo test -- --test-threads=N>1`
//! a live env-var race. Path injection eliminates that race entirely: each
//! thread holds its own [`TempDir`]-backed `ResolvedPaths`, and the
//! transport's `XURL_PATH` cache is wrapped in `OnceLock<Mutex<...>>` (U8) so
//! `reset_xurl_path_for_tests` is safe to call concurrently.
//!
//! If this test deadlocks or fails, the OnceLock wrap in U8 is incomplete or
//! one of the injected paths is still being shadowed by a global read.

mod common;

use common::{TestEnv, run_in_process};
use std::thread;

#[test]
fn parallel_run_in_process_no_env_race() {
    let mut handles = Vec::with_capacity(16);
    for _ in 0..16 {
        handles.push(thread::spawn(|| {
            let env = TestEnv::new();
            let (exit, _stdout, _stderr) = run_in_process(&["bird", "--version"], &env);
            exit
        }));
    }
    for handle in handles {
        let exit = handle.join().expect("test: thread panicked");
        assert_eq!(exit, 0, "parallel --version should exit 0");
    }
}
