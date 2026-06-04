//! bird — X API CLI binary entrypoint.
//!
//! Restores default SIGPIPE handling (Rust masks it by default), initializes
//! tracing (library consumers manage their own), then delegates to the
//! library's `run_argv`. The library returns `ExitCode`; Rust converts it to
//! a process exit on return.

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let default_directive = "bird=info"
        .parse::<tracing_subscriber::filter::Directive>()
        .expect("invariant: 'bird=info' is a valid tracing directive");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(default_directive),
        )
        .with_writer(std::io::stderr)
        .init();

    bird::cli::run_argv()
}
