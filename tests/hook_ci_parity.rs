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
    /// An opt-in job of the shared `rust-ci.yml`, which runs only when
    /// `ci.yml` sets this input to `true`.
    RustCiOptIn {
        job: &'static str,
        input: &'static str,
    },
    /// No CI job runs this check; the reason says what the hook covers alone.
    HookOnly(&'static str),
}

const GATES: &[(&str, Mirror)] = &[
    ("fmt", Mirror::RustCi("Fmt, clippy, test")),
    ("clippy", Mirror::RustCi("Fmt, clippy, test")),
    ("test", Mirror::RustCi("Fmt, clippy, test")),
    (
        "msrv",
        Mirror::RustCiOptIn {
            job: "MSRV",
            input: "msrv",
        },
    ),
    (
        "doc",
        Mirror::RustCiOptIn {
            job: "Doc build",
            input: "docs",
        },
    ),
    ("deny", Mirror::RustCi("Security audit")),
    (
        "output discipline",
        Mirror::RustCiOptIn {
            job: "Output discipline",
            input: "lint_stdio",
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
        Mirror::RustCiOptIn {
            job: "Workflow concurrency",
            input: "workflow_concurrency",
        },
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
            Mirror::RustCiOptIn { job, input } => {
                assert!(
                    ci.contains(RUST_CI),
                    "{label}: ci.yml calls no rust-ci.yml for {job}"
                );
                assert!(
                    ci.lines()
                        .any(|line| line.trim() == format!("{input}: true")),
                    "{label}: ci.yml does not set `{input}: true`, so {job} never runs"
                );
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
            Mirror::RustCi(_) | Mirror::RustCiOptIn { .. } => uses.starts_with(RUST_CI),
            Mirror::LintBasics(_) => uses.starts_with(LINT_BASICS),
            _ => false,
        });
        assert!(
            mirrored,
            "ci.yml calls `{uses}`, which no pre-push step mirrors"
        );
    }
}
