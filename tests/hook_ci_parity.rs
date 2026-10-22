//! The pre-push hook mirrors CI. Every step it runs is listed here with the CI
//! job that runs the same check, so a gate added to one side without the other
//! fails this test.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Where CI runs the check a hook step runs.
enum Mirror {
    /// A job of the shared `rust-ci.yml`, which `ci.yml` calls.
    RustCi(&'static str),
    /// A job of the shared `lint-basics.yml`, which `ci.yml` calls.
    LintBasics(&'static str),
    /// A bird workflow whose run step carries every one of these fragments,
    /// as the hook step does.
    Workflow {
        file: &'static str,
        fragments: &'static [&'static str],
    },
    /// No CI job runs this check; the reason says what the hook covers alone.
    HookOnly(&'static str),
}

const R20: &[&str] = &[
    r"'\b(println|eprintln|print|eprint|out_println|out_print|diag)!'",
    "--glob 'src/**/*.rs'",
    "--glob '!src/main.rs'",
    "--glob '!src/error/fatal.rs'",
];

const GATES: &[(&str, Mirror)] = &[
    ("fmt", Mirror::RustCi("Fmt, clippy, test")),
    ("clippy", Mirror::RustCi("Fmt, clippy, test")),
    ("test", Mirror::RustCi("Fmt, clippy, test")),
    (
        "msrv",
        Mirror::HookOnly("rust-ci.yml compiles on the pinned toolchain only"),
    ),
    (
        "doc",
        Mirror::HookOnly("rust-ci.yml builds no rustdoc with -D warnings"),
    ),
    ("deny", Mirror::RustCi("Security audit")),
    (
        "r20",
        Mirror::Workflow {
            file: "guard-r20.yml",
            fragments: R20,
        },
    ),
    (
        "windows compat (libc grep)",
        Mirror::RustCi("Windows check"),
    ),
    (
        "windows clippy (cross-target)",
        Mirror::RustCi("Windows check"),
    ),
    ("shellcheck", Mirror::RustCi("Shellcheck")),
    (
        "shfmt",
        Mirror::HookOnly("no shared job checks shell layout"),
    ),
    ("actionlint", Mirror::LintBasics("actionlint")),
    (
        "concurrency namespacing",
        Mirror::HookOnly("no shared job runs check-workflow-concurrency.sh"),
    ),
    (
        "core-env guard",
        Mirror::HookOnly("no shared job runs core-env-guard.sh"),
    ),
];

const RUST_CI: &str = "uses: brettdavies/.github/.github/workflows/rust-ci.yml@";
const LINT_BASICS: &str = "uses: brettdavies/.github/.github/workflows/lint-basics.yml@";

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The label of every `step` and `step_bg` call in the hook, cut at the first
/// shell expansion so `msrv${msrv:+ ($msrv)}` reads as `msrv`.
fn hook_steps(hook: &str) -> Vec<String> {
    hook.lines()
        .map(str::trim_start)
        .filter_map(|line| {
            line.strip_prefix("step \"")
                .or_else(|| line.strip_prefix("step_bg \""))
        })
        .map(|rest| {
            let label = rest.split('"').next().unwrap_or_default();
            label.split('$').next().unwrap_or_default().to_owned()
        })
        .collect()
}

#[test]
fn every_hook_step_names_its_ci_job() {
    let steps = hook_steps(&read("scripts/hooks/pre-push"));
    let unique: BTreeSet<&str> = steps.iter().map(String::as_str).collect();
    assert_eq!(
        unique.len(),
        steps.len(),
        "duplicate step label in pre-push"
    );

    let listed: BTreeSet<&str> = GATES.iter().map(|(label, _)| *label).collect();
    let unlisted: Vec<_> = unique.difference(&listed).collect();
    let missing: Vec<_> = listed.difference(&unique).collect();
    assert!(
        unlisted.is_empty() && missing.is_empty(),
        "pre-push steps and GATES differ: in the hook only {unlisted:?}, in GATES only {missing:?}"
    );
}

#[test]
fn every_ci_job_a_gate_names_is_called() {
    let ci = read(".github/workflows/ci.yml");
    let hook = read("scripts/hooks/pre-push");
    for (label, gate) in GATES {
        match gate {
            Mirror::RustCi(job) => assert!(
                ci.contains(RUST_CI),
                "{label}: ci.yml calls no rust-ci.yml for {job}"
            ),
            Mirror::LintBasics(job) => {
                assert!(
                    ci.contains(LINT_BASICS),
                    "{label}: ci.yml calls no lint-basics.yml for {job}"
                )
            }
            Mirror::Workflow { file, fragments } => {
                let workflow = read(&format!(".github/workflows/{file}"));
                for fragment in *fragments {
                    assert!(
                        workflow.contains(fragment),
                        "{label}: {file} lacks {fragment}"
                    );
                    assert!(
                        hook.contains(fragment),
                        "{label}: pre-push lacks {fragment}"
                    );
                }
            }
            Mirror::HookOnly(reason) => {
                assert!(!reason.is_empty(), "{label}: HookOnly needs a reason")
            }
        }
    }
}

#[test]
fn every_reusable_ci_calls_has_a_hook_step() {
    let ci = read(".github/workflows/ci.yml");
    let called: Vec<&str> = ci
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("uses: "))
        .collect();
    for uses in called {
        let mirrored = GATES.iter().any(|(_, gate)| match gate {
            Mirror::RustCi(_) => uses.starts_with(RUST_CI),
            Mirror::LintBasics(_) => uses.starts_with(LINT_BASICS),
            _ => false,
        });
        assert!(
            mirrored,
            "ci.yml calls `{uses}`, which no pre-push step mirrors"
        );
    }
}
