---
title: "Shell completions subcommand with main() dependency-gated restructuring"
category: architecture-patterns
date: 2026-03-16
tags:
  - shell-completions
  - clap-complete
  - sigpipe
  - main-restructuring
  - dependency-gating
  - cli-ux
components:
  - src/main.rs
  - Cargo.toml
  - tests/cli_smoke.rs
  - tests/transport_integration.rs
severity: medium
resolution_time: "1 hour"
pr: 9
branch: feat/shell-completions
---

# Shell completions subcommand with main() dependency-gated restructuring

## Problem

Bird has 28+ subcommands with various flags and positional arguments but had no shell completion support. Users could not tab-complete commands, flags, or arguments, which hurt discoverability -- especially for new users encountering the CLI for the first time. Package managers like Homebrew need pre-generated completion scripts to install completions automatically alongside the binary.

A secondary problem compounded the first: `main()` had an unconditional xurl fail-fast check that blocked ALL commands when xurl was not installed -- including purely local operations like `doctor` (which diagnoses missing xurl) and the new `completions` (which has zero external dependencies). The `main()` control flow needed restructuring so that the xurl gate only applied to commands that actually need xurl for API calls.

## Solution

Added a `bird completions <shell>` subcommand using `clap_complete` for generating shell completion scripts (bash, zsh, fish, powershell, elvish). Restructured `main()` into a three-tier dependency gate so commands only pay for the initialization they need. Added global SIGPIPE handling so piped commands like `bird completions bash | head` exit cleanly.

## Key Implementation Details

### Completions subcommand

Added `clap_complete = "4"` as a dependency. The `Completions` variant uses `clap_complete::Shell` directly as its argument type, which implements `ValueEnum` -- clap handles validation, error messages, and case-insensitive matching automatically:

```rust
/// Generate shell completions
Completions {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: clap_complete::Shell,
},
```

Generation is a single call that writes the script to stdout:

```rust
if let Command::Completions { shell } = &cli.command {
    clap_complete::generate(*shell, &mut Cli::command(), "bird", &mut std::io::stdout());
    return ExitCode::SUCCESS;
}
```

The `clap_complete::Shell` enum is `#[non_exhaustive]`, meaning new shells may be added in future crate versions without a breaking change.

### main() three-tier dependency gating

The core architectural change: `main()` was restructured so the vertical ordering reflects the dependency hierarchy. Commands exit at the earliest tier that satisfies their needs:

```text
Tier 1: Meta-commands (completions)     -- needs: parsed args only
Tier 2: Diagnostic commands (doctor)    -- needs: config + BirdClient
Tier 3: API commands (everything else)  -- needs: config + BirdClient + xurl
```

The `Doctor` and `Completions` arms in `run()` use `unreachable!()` since they are handled before `run()` is ever called:

```rust
Command::Doctor { .. } => {
    unreachable!("doctor is handled before the xurl gate in main()")
}
Command::Completions { .. } => {
    unreachable!("completions is handled before config init in main()")
}
```

This is safe for doctor because `BirdClient::new()` does not invoke xurl -- `XurlTransport` is a unit struct that resolves xurl lazily only when `xurl_call()` is invoked. Doctor's own `build_xurl_status()` calls `transport::resolve_xurl_path()` internally and handles the error gracefully (returns `available: false`).

### SIGPIPE fix

Rust masks SIGPIPE by default, which causes panics when writing to closed pipes (e.g., `bird completions bash | head`). The fix restores POSIX-standard behavior at the top of `main()`, before any I/O:

```rust
#[cfg(unix)]
unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
}
```

This was chosen over the BufWriter+flush approach because `clap_complete::generate()` writes via internal `writeln!` macros. When the completion script exceeds BufWriter's 8KB buffer, an internal flush occurs during `generate()` -- if the pipe is closed, `generate()` panics before any explicit error handler is reached. The global SIGPIPE fix protects ALL piped commands. This matches ripgrep and bat's approach. The `libc` crate was already a dependency.

### Test coverage

11 new tests in `tests/cli_smoke.rs`:

| Test | What it verifies |
|------|-----------------|
| `completions_bash_exits_zero` | bash generation succeeds with non-empty output |
| `completions_zsh_contains_function_name` | zsh output contains `_bird` function |
| `completions_fish_exits_zero` | fish generation succeeds |
| `completions_powershell_exits_zero` | PowerShell generation succeeds |
| `completions_elvish_exits_zero` | Elvish generation succeeds |
| `completions_invalid_shell_exits_two` | invalid shell name produces clap error (exit 2) |
| `completions_no_argument_exits_two` | missing argument produces clap error (exit 2) |
| `completions_bash_contains_subcommand_names` | output contains `me`, `bookmarks`, `completions` |
| `completions_bash_output_is_substantial` | output >1KB for 28+ subcommands |
| `completions_works_without_xurl` | succeeds with `BIRD_XURL_PATH=/tmp/nonexistent` |
| `completions_does_not_create_config` | no `~/.config/bird` created |

4 updated tests in `tests/transport_integration.rs`: existing `bird_xurl_path_*` tests updated to assert doctor succeeds (exit 0) with `xurl.available: false` in stdout JSON, plus 1 new test verifying API commands still exit 78 when xurl is missing.

## Gotchas

1. **`clap_complete::Shell` is `#[non_exhaustive]`** -- you cannot exhaustively match on it. If you need to enumerate shells, use the `Shell::value_variants()` method from `ValueEnum`.

2. **The `unreachable!()` arms in `run()` are load-bearing documentation** -- they explain why `Doctor` and `Completions` are absent from the command dispatch. Removing them would cause a compiler error (non-exhaustive match). Do not replace with `_` wildcards.

3. **SIGPIPE fix is `unsafe`** -- this is trivially auditable (`SIG_DFL` restores default signal disposition) but will appear in `cargo audit` unsafe reports. The unstable `-Zon-broken-pipe=kill` compiler flag is the long-term Rust solution but remains nightly-only as of March 2026.

4. **`CompleteEnv` (dynamic/runtime completions) was deliberately rejected** -- it requires the `unstable-dynamic` feature flag, invokes the binary on every Tab press (adding latency), and is not suitable for package manager distribution where Homebrew expects static files.

5. **Adding new "meta-commands" in the future** -- any command that needs zero external dependencies should follow the Tier 1 pattern (early return before config/DB init). Any command that needs config/DB but not xurl should follow the Tier 2 pattern (after config init, before xurl gate).

## Prevention

- **When adding new subcommands**: decide which tier the command belongs to. If it needs no config/DB/xurl, add a Tier 1 early return. If it needs config/DB but not xurl, add a Tier 2 early return. Otherwise it falls through to Tier 3 (the default `run()` dispatch).
- **When adding new shells**: `clap_complete::Shell` is `#[non_exhaustive]`, so new variants appear automatically when updating the crate. Add corresponding smoke tests in `tests/cli_smoke.rs`.
- **SIGPIPE**: no maintenance needed -- the global fix at the top of `main()` protects all current and future piped output.

## Related Solutions

- [xurl subprocess transport layer](xurl-subprocess-transport-layer.md) -- documents the xurl fail-fast check design, `XurlTransport` unit struct, and `OnceLock` caching that makes the Tier 2 doctor bypass safe
- [Security audit](../security-issues/rust-cli-security-code-quality-audit.md) -- documents exit codes as public API (78=config, 77=auth, 1=command) and the `run()` extraction pattern
- [CI formatting drift](../build-errors/ci-formatting-drift-rust-edition-2024.md) -- documents the toolchain pinning and edition 2024 setup that this branch builds on
