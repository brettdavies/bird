//! Plan 2 R24 regression test: the `if !quiet { writeln!(stderr, ...).ok(); }`
//! guard must preserve the zero-allocation property — when `quiet` is true,
//! no `format_args!` evaluation and no `write*` call may run.
//!
//! Strategy: install a `PanicOnWrite` writer in BirdClient's `stderr` slot
//! and force a store failure: a write clears its account's response files,
//! and here the account's directory is a regular file, so the clear fails on
//! every platform. With `quiet = true` the writer must never be touched; with
//! `quiet = false` the same failure must reach it, which proves the test
//! wires the writer correctly.

use std::io::Write;
use std::sync::{Arc, Mutex};

use bird::cli::auth_scheme::AuthType;
use bird::store::responses::{Account, account_key};
use bird::store::{BirdClient, CacheOpts, RequestContext, StoreAccount, StoreSetup};

/// Writer that panics on any byte-level write operation. A passing quiet
/// test means no `writeln!` reached this writer.
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

/// Sends one write whose response-file clear fails, through a client whose
/// diagnostics go to `PanicOnWrite`.
fn send_a_write_that_cannot_clear(dir: &tempfile::TempDir, quiet: bool) -> BirdClient {
    let mut setup = StoreSetup::under(dir.path());
    setup.account = StoreAccount {
        app: "default".into(),
        default_user: Some("alice".into()),
    };
    let account = Account {
        app: "default".into(),
        user: "alice".into(),
    };
    let blocker = setup.paths.responses_dir.join(account_key(&account));
    std::fs::create_dir_all(&setup.paths.responses_dir).expect("responses dir");
    std::fs::write(&blocker, b"not a directory").expect("blocker file");

    let xurl = Box::new(bird::xurl_client::ConstructionStub::new(
        "test: no xurl resolved".to_string(),
    ));
    let stderr: Arc<Mutex<dyn Write + Send>> = Arc::new(Mutex::new(PanicOnWrite));
    let mut client = BirdClient::new(xurl, setup, CacheOpts::default(), None, quiet, stderr);
    let ctx = RequestContext {
        auth_type: &AuthType::OAuth2User,
        username: None,
    };
    let _ = client.request("POST", "https://api.x.com/2/users/42/likes", &ctx, None);
    client
}

#[test]
fn diag_quiet_gate_does_not_evaluate_writer_when_quiet() {
    let dir = tempfile::tempdir().expect("tempdir");
    let client = send_a_write_that_cannot_clear(&dir, true);
    assert!(
        client.store().status().first_failure().is_some(),
        "the store failure must have happened for this test to mean anything"
    );
    drop(client);
}

#[test]
#[should_panic(expected = "quiet-gate regression")]
fn diag_quiet_gate_does_panic_when_not_quiet_and_a_diag_fires() {
    let dir = tempfile::tempdir().expect("tempdir");
    let _ = send_a_write_that_cannot_clear(&dir, false);
}
