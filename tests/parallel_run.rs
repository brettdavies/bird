//! Parallel-execution regression for the path-injection design.
//!
//! Spawn 16 threads concurrently invoking [`bird::cli::run_with_paths`] via
//! the shared `tests/common/mod.rs` helpers, each thread carrying a unique
//! `xurl_path` snapshot. The runner attaches that path to the per-call
//! [`bird::transport::XurlTransport`] instance, so concurrent runs do not
//! share state via any process-global cache. Each thread's resolution
//! error envelope must reference its own unique path, proving the absence
//! of cross-thread state leak.
//!
//! If this test deadlocks, fails the per-thread path assertion, or
//! observes another thread's path in its stderr, one of the injected
//! paths is being shadowed by a global read or the per-call transport
//! carry isn't actually per-call.

mod common;

use common::{TestEnv, run_in_process};
use std::path::PathBuf;
use std::thread;

#[test]
fn parallel_run_in_process_no_env_race() {
    let mut handles = Vec::with_capacity(16);
    for i in 0..16 {
        handles.push(thread::spawn(move || {
            let unique_path = PathBuf::from(format!(
                "/tmp/bird-parallel-xurl-thread-{}-{}-fixture",
                std::process::id(),
                i
            ));
            let env = TestEnv::new().with_xurl_path(unique_path.clone());
            // `bird me` goes through the xurl gate; with a nonexistent
            // BIRD_XURL_PATH the runner emits a config-error envelope on
            // stderr referencing the exact path. If another thread's path
            // leaks through, this thread's assertion fires.
            let (exit, _stdout, stderr) = run_in_process(&["bird", "--output", "json", "me"], &env);
            (i, unique_path, exit, stderr)
        }));
    }
    for handle in handles {
        let (i, unique_path, exit, stderr) = handle.join().expect("test: thread panicked");
        assert_eq!(
            exit, 78,
            "thread {i}: missing xurl must yield config-error exit 78"
        );
        assert!(
            !stderr.is_empty(),
            "thread {i}: stderr must carry the error envelope"
        );
        let unique_str = unique_path.to_string_lossy();
        assert!(
            stderr.contains(unique_str.as_ref()),
            "thread {i}: stderr must reference this thread's unique path {unique_str:?}, \
             got stderr: {stderr:?}"
        );
    }
}
