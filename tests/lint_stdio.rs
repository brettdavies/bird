//! `scripts/lint-stdio.sh` holds bird's output rule: no print macro under
//! `src/` outside `src/main.rs` and `src/error/fatal.rs`, the retired
//! `out_println!`, `out_print!`, and `diag!` included, and comments count.
//! Each case runs the script on a planted tree twice: with the ripgrep on
//! `PATH`, and with a ripgrep that lacks PCRE2, as CI's apt ripgrep 13 does.
//! A script that cannot search on that ripgrep must fail, never report clean.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

const SCRIPT: &str = "scripts/lint-stdio.sh";

/// The ripgrep on `PATH`. None of GitHub's runner images ship ripgrep, so in
/// a job that installs none the test has nothing to run the script with; the
/// pre-push hook, whose output-discipline step needs ripgrep too, runs it.
fn real_rg() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("rg"))
        .find(|candidate| candidate.is_file())
}

/// A directory holding an `rg` that refuses `--pcre2` the way a ripgrep built
/// without PCRE2 does and hands every other call to the real one.
fn no_pcre2_rg(root: &Path, real: &Path) -> PathBuf {
    let dir = root.join("no-pcre2-bin");
    fs::create_dir_all(&dir).expect("test: shim dir");
    let shim = dir.join("rg");
    let body = format!(
        "#!/usr/bin/env bash\n\
         for arg in \"$@\"; do\n\
           if [ \"$arg\" = --pcre2 ]; then\n\
             echo 'PCRE2 is not available in this build of ripgrep' >&2\n\
             exit 2\n\
           fi\n\
         done\n\
         exec '{}' \"$@\"\n",
        real.display()
    );
    fs::write(&shim, body).expect("test: write shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("test: chmod shim");
    dir
}

/// A scratch repository holding the script and the given `src/` files.
fn tree(files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().expect("test: tempdir");
    let scripts = tmp.path().join("scripts");
    fs::create_dir_all(&scripts).expect("test: scripts dir");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join(SCRIPT);
    fs::copy(&script, scripts.join("lint-stdio.sh")).expect("test: copy script");
    for (path, body) in files {
        let file = tmp.path().join(path);
        fs::create_dir_all(file.parent().expect("test: parent")).expect("test: src dir");
        fs::write(file, body).expect("test: write source");
    }
    tmp
}

/// Runs the script in `root` with `bin` ahead of `PATH`; returns its exit
/// code and stderr.
fn run(root: &Path, bin: Option<&Path>) -> (i32, String) {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("scripts/lint-stdio.sh"));
    if let Some(bin) = bin {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let joined = std::env::join_paths(
            std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .expect("test: join PATH");
        cmd.env("PATH", joined);
    }
    let out = cmd.output().expect("test: run lint-stdio.sh");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Asserts `expected` under both ripgreps.
fn check(files: &[(&str, &str)], expected: i32) {
    let Some(real) = real_rg() else {
        eprintln!("lint_stdio: skipped, did not run {SCRIPT}: ripgrep is not on PATH");
        return;
    };
    let tmp = tree(files);
    let shim = no_pcre2_rg(tmp.path(), &real);
    for (label, bin) in [
        ("ripgrep on PATH", None),
        ("ripgrep without PCRE2", Some(&*shim)),
    ] {
        let (code, stderr) = run(tmp.path(), bin);
        assert_eq!(
            code, expected,
            "{SCRIPT} with {label} exited {code}, expected {expected}, on {files:?}\nstderr:\n{stderr}"
        );
    }
}

const ALLOWED: &[(&str, &str)] = &[
    ("src/main.rs", "fn main() { println!(\"usage\"); }\n"),
    (
        "src/error/fatal.rs",
        "pub fn die() { eprintln!(\"fatal\"); }\n",
    ),
    ("src/lib.rs", "pub fn quiet() {}\n"),
];

#[test]
fn allowed_files_may_print() {
    check(ALLOWED, 0);
}

#[test]
fn std_macro_outside_allowed_files_fails() {
    for mac in ["println", "eprintln", "print", "eprint"] {
        let body = format!("pub fn leak() {{ {mac}!(\"x\"); }}\n");
        check(&[ALLOWED[0], ALLOWED[1], ("src/lib.rs", &body)], 1);
    }
}

#[test]
fn retired_macro_fails() {
    for mac in ["out_println", "out_print", "diag"] {
        let body = format!("pub fn leak() {{ {mac}!(\"x\"); }}\n");
        check(&[ALLOWED[0], ALLOWED[1], ("src/store/mod.rs", &body)], 1);
    }
}

#[test]
fn commented_macro_fails() {
    check(
        &[
            ALLOWED[0],
            ALLOWED[1],
            ("src/lib.rs", "// diag!(\"left behind\");\n"),
        ],
        1,
    );
}
