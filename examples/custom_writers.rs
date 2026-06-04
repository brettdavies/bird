//! Fan runner output to multiple sinks via a tee-style `std::io::Write`.
//!
//! `run_with_paths` accepts any `&mut dyn Write` for stdout/stderr. The
//! `Tee` writer below forwards every byte to two backing writers — an
//! in-memory `Vec<u8>` and the process's real stdout/stderr — enabling
//! simultaneous capture and live forwarding in a single pass.
//!
//! Run with: `cargo run --example custom_writers`
//!
//! Expected exit: 0.

use bird::config::{EnvOverrides, ResolvedPaths};
use std::io::{self, Write};
use std::process::ExitCode;
use tempfile::TempDir;

/// Writer that forwards every write to two underlying sinks.
struct Tee<A: Write, B: Write> {
    a: A,
    b: B,
}

impl<A: Write, B: Write> Write for Tee<A, B> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.a.write(buf)?;
        self.b.write_all(&buf[..n])?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.a.flush()?;
        self.b.flush()
    }
}

fn main() -> ExitCode {
    let tmp = TempDir::new().expect("example: tempdir");
    let config_dir = tmp.path().join(".config").join("bird");
    std::fs::create_dir_all(&config_dir).expect("example: create config dir");
    let paths = ResolvedPaths {
        config_dir: config_dir.clone(),
        store_path: config_dir,
    };

    let mut stdout_capture: Vec<u8> = Vec::new();
    let mut stderr_capture: Vec<u8> = Vec::new();

    // The stderr handle has to be wrapped in a smaller scope so we can read
    // the capture buffer after the runner returns.
    let exit = {
        let stderr_real = io::stderr();
        let mut stdout_tee = Tee {
            a: &mut stdout_capture,
            b: io::stdout(),
        };
        let mut stderr_tee = Tee {
            a: &mut stderr_capture,
            b: stderr_real.lock(),
        };
        bird::cli::run_with_paths(
            ["bird", "--examples"],
            &mut stdout_tee,
            &mut stderr_tee,
            paths,
            EnvOverrides::default(),
        )
    };

    eprintln!(
        "\n--- captured: stdout={}B stderr={}B ---",
        stdout_capture.len(),
        stderr_capture.len()
    );
    exit
}
