---
title: "feat: Add shell completion generation"
type: feat
status: completed
date: 2026-03-16
deepened: 2026-03-16
origin: docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md
schedule: docs/plans/2026-03-16-000-meta-implementation-schedule.md
schedule_phase: 1
---

# feat: Add shell completion generation

## Enhancement Summary

**Deepened on:** 2026-03-16 (round 2)
**Research agents used:** framework-docs-researcher (clap_complete), best-practices-researcher (release workflows), architecture-strategist, security-sentinel, code-simplicity-reviewer, pattern-recognition-specialist, spec-flow-analyzer, learnings-researcher

### Key Improvements (Round 2)

1. **SIGPIPE: global `libc::signal(SIGPIPE, SIG_DFL)` is the correct approach** -- BufWriter+flush is insufficient because `generate()` can panic mid-write when its internal buffer overflows; `libc` is already a dependency (Cargo.toml line 46)
2. **Doctor early-return: restructure `main()` to gate xurl check by command need** -- not a growing exception list; concrete `main()` flow provided
3. **Existing tests will break**: 3 tests in `tests/transport_integration.rs` assert on xurl fail-fast stderr for `bird doctor`; must be updated
4. **Archive structure resolved**: use top-level directory convention (`bird-<target>/...`), matching bat and ripgrep; Homebrew extracts automatically
5. **Supply chain: add SLSA attestation and pin GitHub Actions to commit SHAs** for release integrity
6. **Windows archive should include `completions/bird.ps1`** (platform-appropriate), Unix archives include bash/zsh/fish only
7. **`CompleteEnv` (dynamic completions) rejected** -- requires unstable feature flag, adds latency to Tab press, not suitable for package managers; AOT `generate()` is correct

### Key Improvements (Round 1)

1. Added SIGPIPE handling guidance with concrete stable-Rust solution
2. Added ValueHint annotations recommendation for richer shell completions
3. Added concrete CI workflow patterns from bat project and community best practices for archive bundling
4. Added edge cases: write error handling on stdout, archive naming with version, PowerShell/Elvish exclusion from archives
5. **`bird doctor` should also get early-return treatment** -- it is currently blocked by the xurl fail-fast check, but its purpose is to *diagnose* missing xurl

## Overview

Add a `bird completions <shell>` subcommand that prints a shell completion script to stdout, using `clap_complete`. Pre-generate scripts as GitHub Release assets for packaging. This is PR 3 of 4 from the distribution/DX brainstorm (see brainstorm: `docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md`).

## Problem Statement / Motivation

Bird has 28 subcommands with various flags but no shell completion support. Users cannot tab-complete commands, flags, or subcommand-specific arguments. This hurts discoverability, especially for new users. Package managers (Homebrew, in PR 2) need pre-generated scripts to install completions automatically.

## Proposed Solution

1. Add `clap_complete = "4"` dependency.
2. Add a `Completions` variant to the `Command` enum accepting `clap_complete::Shell` (which implements `ValueEnum`).
3. Generate and print the completion script to stdout.
4. **Bypass the xurl fail-fast check** for the `completions` subcommand -- it has zero dependency on xurl, config, or the database. A user who runs `cargo install bird` without xurl should still be able to generate completions.
5. Update `release.yml` to generate completion scripts and include them as release assets.

## Technical Considerations

### Early return before xurl check

The current `main()` at `src/main.rs:775` follows a strict sequential pipeline:

```text
parse CLI -> use_color -> xurl fail-fast -> validate username -> load config -> create BirdClient -> run(command)
```

The xurl fail-fast check at line 796 is unconditional -- it blocks ALL commands when xurl is missing, including `doctor` and the new `completions`. The plan restructures `main()` to gate the xurl check by command need.

#### Recommended `main()` restructuring (Round 2 refinement)

Instead of adding a growing list of early-return exceptions, restructure `main()` so the vertical ordering reflects the dependency hierarchy:

```rust
fn main() -> ExitCode {
    // Restore default SIGPIPE handling (see SIGPIPE section below)
    #[cfg(unix)]
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }

    tracing_subscriber::fmt()...init();

    let cmd = Cli::command().color(output::color_choice_for_clap());
    let matches = cmd.get_matches();
    let cli = match Cli::from_arg_matches(&matches) { ... };
    let use_color = use_color_from_cli(cli.plain, cli.no_color);

    // --- Meta-commands: need nothing beyond parsed args ---
    if let Command::Completions { shell } = &cli.command {
        clap_complete::generate(*shell, &mut Cli::command(), "bird", &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }

    // --- Username validation + config + DB init (no xurl needed) ---
    let overrides = validate_and_build_overrides(&cli, use_color);
    let config = match ResolvedConfig::load(overrides) { ... };
    let mut client = db::BirdClient::new(...);

    // --- Diagnostic commands: need config/DB but not xurl ---
    if let Command::Doctor { command, pretty } = &cli.command {
        match doctor::run_doctor(&client, pretty, command.as_deref(), use_color, ...) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => { e.print(use_color); return ExitCode::from(e.exit_code()); }
        }
    }

    // --- xurl gate: only for API commands ---
    if let Err(e) = transport::resolve_xurl_path() {
        let err = BirdError::Config(e);
        err.print(use_color);
        return ExitCode::from(err.exit_code());
    }

    // --- All remaining commands ---
    match run(cli.command, config, &mut client, use_color, cli.cache_only) { ... }
}
```

This is architecturally superior because:

1. **Completions** escapes before ANY initialization (correct -- it needs nothing).
2. **Doctor** escapes after config/DB init but before the xurl gate (correct -- it needs `BirdClient` but not xurl). `XurlTransport` is a unit struct; `BirdClient::new()` succeeds even without xurl because xurl is only resolved lazily when `xurl_call()` is invoked.
3. **All remaining commands** get the xurl check, which is correct since they all depend on xurl for API calls.
4. The vertical ordering of `main()` becomes a visual map of the dependency chain.
5. No duplicate initialization code. Doctor uses the real `BirdClient`, not a "lightweight" variant.

#### Why this is safe for doctor

`doctor.rs:50-66` has its own `build_xurl_status()` that calls `transport::resolve_xurl_path()` internally and handles the error gracefully (returns `available: false`). Doctor never depended on the xurl path from `main()` -- the fail-fast check was simply blocking it unnecessarily.

### Shell argument validation

`clap_complete::Shell` is an enum with variants `Bash`, `Zsh`, `Fish`, `PowerShell`, `Elvish`. Since it implements `ValueEnum`, clap handles validation and error messages automatically. The argument is case-insensitive by default. The enum is `#[non_exhaustive]`, so it cannot be exhaustively matched (new shells may be added in future versions).

### SIGPIPE handling (Round 2: resolved)

**Use the global `libc::signal(SIGPIPE, SIG_DFL)` approach. Do NOT rely solely on BufWriter + BrokenPipe.**

The BufWriter + explicit `flush()` approach from Round 1 has a critical gap: `clap_complete::generate()` writes via `writeln!` macros internally. `BufWriter`'s internal buffer is 8KB by default. When the completion script exceeds 8KB (very likely with 28+ subcommands), an internal flush occurs during `generate()`. If the downstream pipe is closed (e.g., `bird completions bash | head`), this internal flush fails, and `generate()` panics -- BEFORE the explicit `flush()` error handler is reached.

The global fix is one line and protects ALL piped commands (not just completions):

```rust
fn main() -> ExitCode {
    // Restore default SIGPIPE handling so piped commands exit cleanly.
    // Without this, Rust masks SIGPIPE and all writes to closed pipes panic.
    // The `libc` crate is already a dependency (Cargo.toml line 46).
    #[cfg(unix)]
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }
    // ...
}
```

This restores POSIX-standard behavior: the process terminates with signal 13 (exit code 141) when writing to a closed pipe. This is how every C program, Python script, and non-Rust tool behaves. ripgrep and bat use the same approach.

The unstable `-Zon-broken-pipe=kill` compiler flag ([rust-lang/rust#97889](https://github.com/rust-lang/rust/issues/97889)) is the long-term Rust solution but is nightly-only.

### Subcommand vs. flag (Round 2: confirmed correct)

xurl-rs uses a hidden `--generate-completion` flag. Bird's plan uses a visible `completions` subcommand. This divergence is intentional and correct:

- **Structural fit**: xurl-rs has `command: Option<Commands>` (subcommand is optional). Bird has `command: Command` (subcommand is required). A flag on the root struct would be structurally anomalous for Bird.
- **Discoverability**: `bird completions` appears in `--help` and is tab-completable. With 28+ subcommands, discoverability matters.
- **Semantic correctness**: Completions generation is an action, not a modifier. Subcommands model actions; flags model modifiers.
- **Ecosystem alignment**: `rustup completions`, `gh completion`, `poetry completions` all use the subcommand pattern.

### Release asset generation

Completion scripts are platform-independent (they describe the CLI structure, not the binary). Generate them once from any platform in the release workflow.

**Strategy (recommended): Generate from one native build.** Add a dedicated `completions` job that runs after the native Linux x86_64 build, generates all shell scripts, and uploads as an artifact. The downstream `release` job downloads both per-target binary artifacts and the completions artifact to bundle archives.

```text
Job dependency graph:
  build (5 targets, parallel)
      |
      v
  completions (needs: build, runs linux x86_64 binary)
      |
      v
  release (needs: build + completions, creates archives + GH release)
```

### Release asset naming convention

| Shell | Asset filename | Rationale |
|-------|---------------|-----------|
| Bash | `bird.bash` | Standard Homebrew convention |
| Zsh | `_bird` | Zsh requires `_` prefix for completion functions |
| Fish | `bird.fish` | Fish requires `.fish` extension |
| PowerShell | `bird.ps1` | Standard PowerShell script extension |
| Elvish | `bird.elv` | Standard Elvish extension |

### Release artifact format change

Current release uploads raw binaries. For PR 2 (Homebrew), we need `.tar.gz` archives containing the binary plus completions. **This PR should switch to `.tar.gz` (Linux/macOS) and `.zip` (Windows) archives** to avoid rework in PR 2 (Homebrew).

#### Archive structure (Round 2: resolved contradiction)

Use a top-level directory matching the archive name, consistent with bat and ripgrep conventions. Homebrew's `tar xf` automatically enters the first directory.

**Unix archives** (`.tar.gz`):

```text
bird-aarch64-apple-darwin/
  bird
  completions/bird.bash
  completions/_bird
  completions/bird.fish
  LICENSE-MIT
  LICENSE-APACHE
  README.md
```

**Windows archive** (`.zip`):

```text
bird-x86_64-pc-windows-msvc/
  bird.exe
  completions/bird.ps1
  LICENSE-MIT
  LICENSE-APACHE
  README.md
```

Note: Windows archive includes PowerShell completions (platform-appropriate). Unix archives include bash/zsh/fish only. PowerShell and Elvish are also uploaded as standalone release assets for cross-platform users.

#### Archive creation commands

```bash
# Unix (Linux/macOS)
ARCHIVE="bird-${TARGET}"
mkdir -p "${ARCHIVE}/completions"
cp "target/${TARGET}/release/bird" "${ARCHIVE}/"
cp completions/bird.bash completions/_bird completions/bird.fish "${ARCHIVE}/completions/"
cp LICENSE-MIT LICENSE-APACHE README.md "${ARCHIVE}/"
tar czf "${ARCHIVE}.tar.gz" "${ARCHIVE}"
# Verify: no absolute paths, no traversal
tar tzf "${ARCHIVE}.tar.gz"

# Windows
ARCHIVE="bird-x86_64-pc-windows-msvc"
mkdir -p "${ARCHIVE}/completions"
cp "target/x86_64-pc-windows-msvc/release/bird.exe" "${ARCHIVE}/"
cp completions/bird.ps1 "${ARCHIVE}/completions/"
cp LICENSE-MIT LICENSE-APACHE README.md "${ARCHIVE}/"
7z -y a "${ARCHIVE}.zip" "${ARCHIVE}"/*
```

**SHA256 checksums**: Generate and upload a `checksums.sha256` file alongside archives. PR 2 (Homebrew) needs SHA256 hashes, and providing them as a release asset simplifies the tap update workflow.

```bash
cd release
for f in bird-*.tar.gz bird-*.zip; do
  [ -f "$f" ] && sha256sum "$f" > "$f.sha256"
done
sha256sum bird-*.tar.gz bird-*.zip > checksums.sha256
```

### Interaction with `requirements.rs` and `bird doctor`

The `completions` subcommand has no auth requirements and no API interaction. It should NOT be added to `requirements.rs`. This is consistent with how `cache` and `doctor` are handled -- meta-commands are not registered in `requirements.rs`. Doctor already only lists commands returned by `requirements_for_command()`, so `completions` will be correctly omitted from doctor output.

### `CompleteEnv` (dynamic completions) -- rejected

`clap_complete` offers a newer `CompleteEnv` API where the binary handles completions at runtime when invoked with `COMPLETE=$SHELL`. This is NOT appropriate for bird:

1. Requires the `unstable-dynamic` feature flag (API is explicitly unstable)
2. The binary is invoked on every Tab press, adding latency
3. Version mismatch risk between the registration snippet and binary
4. Not suitable for package manager distribution (Homebrew expects static completion files)

The AOT `generate()` approach is the correct choice.

### Research Insight: ValueHint annotations for richer completions

`clap_complete` generates richer completions when arguments have `ValueHint` annotations (e.g., `ValueHint::FilePath`, `ValueHint::Url`). Defer to a follow-up PR. See [clap ValueHint docs](https://docs.rs/clap/latest/clap/enum.ValueHint.html).

### Research Insight: Fish completion limitations

Fish completions generated by `clap_complete` only support named arguments (`-o` or `--opt`), not positional arguments. This is a known `clap_complete` limitation, not a bug in our implementation. Document in README completion instructions.

## Acceptance Criteria

### Completions subcommand

- [x] `bird completions bash` prints a bash completion script to stdout and exits 0
- [x] `bird completions zsh` prints a zsh completion script to stdout and exits 0
- [x] `bird completions fish` prints a fish completion script to stdout and exits 0
- [x] `bird completions powershell` prints a PowerShell script to stdout and exits 0
- [x] `bird completions elvish` prints an Elvish script to stdout and exits 0
- [x] `bird completions invalid` exits non-zero with a helpful clap error message
- [x] `bird completions` (no argument) exits 2 with clap usage error
- [x] `bird completions bash` works **without xurl installed** (bypasses fail-fast check)
- [x] `bird completions bash` does NOT create `~/.config/bird/bird.db` or load config
- [x] Generated scripts contain subcommand names (`me`, `bookmarks`, `search`, `thread`, `completions`, etc.)
- [x] Generated scripts contain shell names (`bash`, `zsh`, etc.) as completable values for the `completions` subcommand

### SIGPIPE and pipeline safety

- [x] `bird completions bash | head -1` exits cleanly (no panic)
- [x] Global `libc::signal(SIGPIPE, SIG_DFL)` added with `#[cfg(unix)]` guard

### Doctor early-return

- [x] `bird doctor` works **without xurl installed** (bypasses fail-fast check, reports xurl as missing)
- [x] `bird doctor --pretty` without xurl shows xurl status and install guidance
- [x] `bird me` without xurl still exits 78 (fail-fast preserved for API commands)
- [x] 3 existing `bird_xurl_path_*` tests in `tests/transport_integration.rs` updated and passing

### Release workflow (deferred to Plan 003B)

- [ ] Release workflow produces `.tar.gz` / `.zip` archives with completions included
- [ ] Release workflow uploads standalone completion scripts as individual assets
- [ ] SHA256 checksums generated and uploaded alongside archives
- [x] All existing tests pass; `cargo clippy` clean; `cargo fmt` clean

## Implementation Plan

### Phase 1: SIGPIPE fix (`src/main.rs`)

Add at the very top of `main()`, before any I/O:

```rust
#[cfg(unix)]
unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }
```

This is one line, uses the existing `libc` dependency, and protects all piped commands globally.

### Phase 2: Completions subcommand (`src/main.rs`)

1. Add `clap_complete = "4"` to `Cargo.toml` dependencies.
2. Add `Completions` variant to `Command` enum:

   ```rust
   /// Generate shell completions
   Completions {
       /// Shell to generate completions for
       #[arg(value_enum)]
       shell: clap_complete::Shell,
   },
   ```

3. In `main()`, after `Cli::from_arg_matches()` and `use_color_from_cli()`, add early return:

   ```rust
   if let Command::Completions { shell } = &cli.command {
       clap_complete::generate(*shell, &mut Cli::command(), "bird", &mut std::io::stdout());
       return ExitCode::SUCCESS;
   }
   ```

   With the global SIGPIPE fix in Phase 1, no per-site BrokenPipe handling is needed. This matches the xurl-rs pattern exactly.

4. This goes BEFORE the xurl check at line 796, config load at line 830, and client init at line 845.

### Phase 3: Doctor early-return (`src/main.rs`)

Restructure `main()` so that doctor runs after config/DB init but before the xurl fail-fast check:

1. Move the completions early-return to just after `use_color_from_cli()` (before everything else).
2. Keep username validation, config load, and BirdClient init in their current order.
3. After BirdClient init, add the doctor early-return:

   ```rust
   if let Command::Doctor { command, pretty } = &cli.command {
       let scope = command.as_deref();
       let use_emoji = use_color && *pretty;
       match doctor::run_doctor(&client, *pretty, scope, use_color, use_emoji) {
           Ok(()) => return ExitCode::SUCCESS,
           Err(e) => {
               let err = BirdError::Command { name: "doctor", source: e };
               err.print(use_color);
               return ExitCode::from(err.exit_code());
           }
       }
   }
   ```

4. Move the xurl fail-fast check AFTER the doctor early-return. It now only gates API commands.

**Why this is safe**: `BirdClient::new()` does not invoke xurl. `XurlTransport` is a unit struct that resolves xurl lazily. Doctor's own `build_xurl_status()` at `doctor.rs:50-66` handles missing xurl gracefully (returns `available: false`).

#### Existing test impact

Three tests in `tests/transport_integration.rs` currently test `bird doctor` with invalid `BIRD_XURL_PATH` values and assert on stderr error messages from the xurl fail-fast check. After this restructuring, doctor bypasses the fail-fast and handles xurl status internally. These tests must be updated:

- **Before**: Assert stderr contains "does not exist" / "is not a file" / "is not executable"
- **After**: Assert stdout JSON contains `"xurl": {"available": false, ...}` and exit 0

Also add a new test: `bird me` with invalid `BIRD_XURL_PATH` still exits 78 (the fail-fast check is preserved for API commands).

### Phase 4: Release workflow (`.github/workflows/release.yml`)

1. Switch build artifacts from raw binaries to `.tar.gz` (Linux/macOS) and `.zip` (Windows) archives.
2. Add a `completions` job that runs after the native Linux x86_64 build, generates all 5 shell scripts, and uploads them as an artifact.
3. In the `release` job, download both binary artifacts and completions artifact, bundle archives with completions included.
4. Generate SHA256 checksums.
5. Pin all GitHub Actions to commit SHAs for supply chain security.

#### Concrete completions job

```yaml
completions:
  needs: build
  runs-on: ubuntu-22.04
  steps:
    - name: Download Linux x86_64 binary
      uses: actions/download-artifact@v4
      with:
        name: bird-x86_64-unknown-linux-gnu
        path: bin

    - name: Generate completions
      run: |
        chmod +x bin/bird
        mkdir -p completions
        bin/bird completions bash > completions/bird.bash
        bin/bird completions zsh > completions/_bird
        bin/bird completions fish > completions/bird.fish
        bin/bird completions powershell > completions/bird.ps1
        bin/bird completions elvish > completions/bird.elv

    - name: Upload completions
      uses: actions/upload-artifact@v4
      with:
        name: completions
        path: completions/
```

#### Supply chain security (Round 2 addition)

- **Pin GitHub Actions to commit SHAs**: Replace `@v4` tags with specific commit SHAs (e.g., `actions/checkout@<sha> # v4.x.x`). Dependabot can auto-update SHAs.
- **SLSA attestation**: Add `actions/attest-build-provenance@v2` to the release job. This generates a signed SLSA provenance attestation tied to the GitHub Actions workflow run, signed by Sigstore. Users verify with `gh attestation verify`. Zero-cost, no secret management.

### Phase 5: Tests (`tests/cli_smoke.rs` and `tests/transport_integration.rs`)

**New completions tests:**

1. Smoke test: `bird completions bash` exits 0, stdout is non-empty.
2. Smoke test: `bird completions zsh` exits 0, stdout contains `_bird`.
3. Smoke test: `bird completions invalid-shell` exits 2 (clap usage error).
4. Smoke test: `bird completions` (no argument) exits 2 (clap missing required argument).
5. Content test: generated bash script contains key subcommand names (`me`, `bookmarks`, `completions`).
6. Pipeline test: `bird completions bash | head -1` exits cleanly (no panic, validates SIGPIPE fix).
7. Size test: `bird completions bash` output is >1KB (validates coverage of 28+ subcommands).

**Updated existing tests:**

1. Update 3 `bird_xurl_path_*` tests in `tests/transport_integration.rs` to assert on doctor's stdout JSON (`xurl.available: false`) instead of stderr fail-fast messages.
2. Add test: `bird me` with invalid `BIRD_XURL_PATH` still exits 78 (fail-fast preserved for API commands).

### Scope note: `main.rs` at 913 lines

`main.rs` currently has 913 lines, exceeding the 200-line refactor trigger from CLAUDE.md. This PR will add ~20 lines net (completions variant + early-return). A `main.rs` refactoring (extracting the `run()` function and/or write-command dispatch) should be tracked as a separate task.

## Success Metrics

- `eval "$(bird completions zsh)"` enables tab completion for all 28+ subcommands and their flags in zsh
- `bird completions bash | head -1` works without panicking
- `bird doctor` works without xurl installed, reporting `xurl.available: false`
- GitHub Release includes archives with completions bundled and standalone completion files
- No regression in existing CLI behavior or test suite

## Dependencies & Risks

- **`clap_complete` version compatibility**: Bird's `clap = "4.4"` resolves to 4.5.57 in the lock file. `clap_complete = "4"` resolves to 4.6.0, which requires clap >= 4.5.20. Compatible. Pin to `"4"` (not `"4.5"`) to match clap's minor range and xurl-rs convention.
- **SIGPIPE on Unix (resolved)**: Addressed by global `libc::signal(SIGPIPE, SIG_DFL)` at top of `main()`. `libc` is already a dependency (Cargo.toml line 46). The `unsafe` block is trivially auditable -- `SIG_DFL` restores default signal disposition, matching every C program and most non-Rust languages.
- **Release workflow changes**: Switching to archives is a backwards-incompatible change for anyone scripting against release assets by name. Since bird is pre-1.0 with no known downstream consumers, this is acceptable.
- **Existing test breakage**: 3 tests in `tests/transport_integration.rs` that test doctor with invalid xurl paths will need updating. See Phase 5.
- **Archive structure must align with Homebrew formula**: The top-level directory convention (`bird-<target>/bird`) requires the Homebrew formula's `def install` block to either `cd` into the directory or rely on Homebrew's automatic directory stripping. Verify alignment with the formula in PR 2 before merging.

## Sources & References

- **Origin brainstorm:** [docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md](docs/brainstorms/2026-03-16-distribution-dx-completions-brainstorm.md) -- key decisions: runtime subcommand + build-time assets, `clap_complete` crate, 5 shells supported
- clap_complete docs: https://docs.rs/clap_complete/latest/clap_complete/
- `clap_complete::Shell` enum: implements `ValueEnum` (clap arg) and `Generator` (script generation); `#[non_exhaustive]`
- `clap_complete::generate()` signature: `fn generate<G: Generator, S: Into<String>>(gen: G, cmd: &mut Command, bin_name: S, buf: &mut dyn Write)`
- Cross-compilation caveat: clap issue #5562 -- runtime generation fails for cross-compiled targets
- Existing pattern: `src/main.rs:784-791` -- manual `CommandFactory::command()` + `get_matches()` + `from_arg_matches()` (supports color customization, also enables early-return for completions)
- xurl-rs reference: `/home/brett/dev/xurl-rs/Cargo.toml:19` -- already uses `clap_complete = "4"`; xurl-rs completion implementation at `/home/brett/dev/xurl-rs/src/main.rs:22-26`
- xurl-rs completion tests: `/home/brett/dev/xurl-rs/tests/completion_tests.rs` -- 5 shell tests + invalid shell test

### Research References (Round 1)

- SIGPIPE in Rust: [rust-lang/rust#46016](https://github.com/rust-lang/rust/issues/46016) -- spurious broken pipe errors in pipelines
- SIGPIPE unstable flag: [rust-lang/rust#97889](https://github.com/rust-lang/rust/issues/97889) -- tracking issue for `unix_sigpipe`
- ValueHint enum: [docs.rs/clap ValueHint](https://docs.rs/clap/latest/clap/enum.ValueHint.html) -- shell completion hints for argument types
- bat project release workflow: bundles completions in `autocomplete/` subdirectory inside archives
- Cross-platform Rust CI/CD: [ahmedjama.com](https://ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/) -- tar.gz/zip archive pattern with target triples
- `softprops/action-gh-release`: standard GitHub Action for uploading release assets
- CLI shell completions guide: [kbknapp.dev/shell-completions](https://kbknapp.dev/shell-completions/) -- best practices for both build-time and runtime generation
- Rust CLI patterns: [dasroot.net](https://dasroot.net/posts/2026/02/rust-cli-patterns-clap-cargo-configuration/) -- modern clap patterns for 2026

### Research References (Round 2)

- `clap_complete` module structure: `aot` (stable), `env` (unstable-dynamic), `generator`/`shells` (deprecated redirects to `aot`); root re-exports from `aot`
- `CompleteEnv` evaluation: requires `unstable-dynamic` feature, invokes binary on every Tab press, version mismatch risk -- rejected for bird
- ripgrep release workflow: archive staging with checksums, cross-compilation via pre-built `cross` binary
- bat CICD workflow: archive creation with `tar czf`, completions in `autocomplete/` directory, aarch64 cross-compilation via `gcc-aarch64-linux-gnu`
- `actions/attest-build-provenance`: SLSA provenance attestation via Sigstore for supply chain integrity
- Homebrew and Rust CLI packaging: [ivaniscoding.github.io](https://ivaniscoding.github.io/posts/rustpackaging2/)
- `docs/solutions/architecture-patterns/xurl-subprocess-transport-layer.md` -- xurl fail-fast check design, `OnceLock` caching, `XurlTransport` unit struct
- `docs/solutions/security-issues/rust-cli-security-code-quality-audit.md` -- exit codes as public API, `run()` extraction pattern
- `docs/solutions/build-errors/ci-formatting-drift-rust-edition-2024.md` -- toolchain pinning, edition 2024 let-chains
