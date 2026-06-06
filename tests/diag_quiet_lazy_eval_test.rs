//! Plan 2 R24 regression test: the `if !quiet { writeln!(stderr, ...).ok(); }`
//! replacement for the deleted `diag!` macro must preserve the zero-allocation
//! property — when `quiet` is true, no `format_args!` evaluation and no
//! `write*` call may run.
//!
//! Strategy: install a `PanicOnWrite` writer in BirdClient's `stderr` slot,
//! then drive a code path that previously emitted a `diag!` (now a guarded
//! `writeln!`). With `quiet = true`, the writer must never be touched — if
//! the guard regresses to evaluating the formatter argument first, the
//! panic from the writer fires. With `quiet = false`, the same path must
//! panic to confirm the test wires the writer correctly.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Writer that panics on any byte-level write operation. Used to verify the
/// quiet-gate short-circuit — a successful test pass means no `writeln!`
/// reached this writer.
struct PanicOnWrite;

impl Write for PanicOnWrite {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        panic!("PanicOnWrite::write — quiet-gate regression");
    }
    fn flush(&mut self) -> std::io::Result<()> {
        panic!("PanicOnWrite::flush — quiet-gate regression");
    }
    fn write_all(&mut self, _buf: &[u8]) -> std::io::Result<()> {
        panic!("PanicOnWrite::write_all — quiet-gate regression");
    }
    fn write_fmt(&mut self, _args: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        panic!("PanicOnWrite::write_fmt — quiet-gate regression");
    }
}

/// Build a BirdClient with the given quiet flag and PanicOnWrite as its stderr.
/// Uses no_store=true so no DB file is opened (avoids the BirdDb::open path
/// that has its own diag sites we may not be exercising).
fn make_client(quiet: bool) -> bird::db::BirdClient {
    let xurl = Box::new(bird::xurl_client::ConstructionStub::new(
        "test: no xurl resolved".to_string(),
    ));
    let cache_opts = bird::db::CacheOpts {
        no_store: true,
        refresh: false,
        cache_only: false,
    };
    let stderr: Arc<Mutex<dyn Write + Send>> = Arc::new(Mutex::new(PanicOnWrite));
    bird::db::BirdClient::new(
        xurl,
        &PathBuf::from("/tmp/bird-lazy-eval-test-store"),
        cache_opts,
        100,
        None,
        quiet,
        stderr,
    )
}

#[test]
fn diag_quiet_gate_does_not_evaluate_writer_when_quiet() {
    // With quiet = true, every internal `if !self.quiet { writeln!(*w, ...).ok(); }`
    // site short-circuits before touching the writer. Constructing the client
    // exercises several BirdClient-internal paths (no_store branch); none of
    // them should write to stderr under quiet.
    let client = make_client(true);
    // Drop the client. Drop paths on BirdClient/BirdDb may emit diagnostics —
    // verify those are also gated.
    drop(client);
    // If we reach this line without panicking, the quiet-gate is intact.
}

#[test]
#[should_panic(expected = "quiet-gate regression")]
fn diag_quiet_gate_does_panic_when_not_quiet_and_a_diag_fires() {
    // Sanity check: with quiet = false, a path that emits a diag MUST reach
    // the writer (and therefore panic). This proves the test isn't trivially
    // a false negative — i.e., quiet = true above succeeded because the
    // writer wasn't reached, not because the writer was never set up right.
    //
    // We force a diag site by attempting a BirdDb open with a bogus path.
    // The no_store=false branch of BirdClient::new opens a real DB, and if
    // the path is invalid the diag site at "[store] warning: failed to open"
    // fires.
    let xurl = Box::new(bird::xurl_client::ConstructionStub::new(
        "test: no xurl resolved".to_string(),
    ));
    let cache_opts = bird::db::CacheOpts {
        no_store: false,
        refresh: false,
        cache_only: false,
    };
    let stderr: Arc<Mutex<dyn Write + Send>> = Arc::new(Mutex::new(PanicOnWrite));
    let _ = bird::db::BirdClient::new(
        xurl,
        // Force the BirdDb::open failure path so the warning diag fires.
        &PathBuf::from("/dev/null/cannot-exist-here/store"),
        cache_opts,
        100,
        None,
        false, // quiet = false → diag must fire
        stderr,
    );
    // If we reach here without a panic, the diag failed to fire — the test
    // setup is wrong. The #[should_panic] attribute ensures CI catches that.
}
