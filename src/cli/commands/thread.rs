//! `bird thread` — reconstruct a conversation thread.

use crate::cli::dispatch::default_auth_type;
use crate::db;
use crate::error::BirdError;
use crate::output::OutputConfig;
use crate::thread;

pub fn run(
    client: &mut db::BirdClient,
    out: &OutputConfig,
    stdout: &mut dyn std::io::Write,
    tweet_id: String,
    pretty: bool,
    max_pages: u32,
) -> Result<(), BirdError> {
    let auth_type = default_auth_type("thread");
    thread::run_thread(
        client,
        out,
        stdout,
        thread::ThreadOpts {
            tweet_id: &tweet_id,
            pretty,
            max_pages,
        },
        &auth_type,
    )
    .map_err(|e| BirdError::from_source("thread", e))?;
    Ok(())
}
