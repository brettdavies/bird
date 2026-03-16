---
title: "Fix CI formatting drift with rust-toolchain.toml and edition 2024"
category: build-errors
date: 2026-03-16
tags:
  - ci
  - rustfmt
  - edition-2024
  - toolchain-pinning
  - cargo-fmt
  - let-chains
---

# Fix CI Formatting Drift with Edition 2024

## Problem

CI failed on `cargo fmt --all --check` despite code passing locally. Formatting diffs appeared across `tests/live_integration.rs` and other files — line wrapping and indentation disagreements.

## Root Cause

Two issues compounded:

1. **Version drift**: Local used rustc 1.93.1 / rustfmt 1.8.0; CI used `dtolnay/rust-toolchain@stable` which resolved to rustc 1.94.0 with a newer rustfmt. Different rustfmt versions produce different output for the same code.
2. **No explicit config**: No `rustfmt.toml` existed, so formatting depended entirely on implicit defaults that change between rustfmt versions. The effective `style_edition` was `"2015"` (the backward-compat default).

## Solution

Added two config files and bumped the Cargo edition:

**`rust-toolchain.toml`** — ensures `rustup` auto-switches to stable with required components:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

**`rustfmt.toml`** — locks formatting rules explicitly:

```toml
style_edition = "2024"
edition = "2024"
```

**`Cargo.toml`** — bumped edition from `"2021"` to `"2024"`.

After adding the config, ran `cargo fmt --all` for a one-time reformat, then fixed new clippy warnings triggered by edition 2024:

- **`collapsible_if`**: Edition 2024 stabilizes let-chains (`if let Some(x) = foo && condition {}`). Clippy now flags nested `if`/`if let` blocks that can collapse. Fixed with `cargo clippy --fix` across 10 files (20 fixes).
- **`nonminimal_bool`**: Simplified `!(a < b)` to `(a >= b)` in transport tests.
- **`module_inception`**: Suppressed with `#[allow]` on `db::db` (rename deferred).
- **`deprecated` `cargo_bin`**: Suppressed with `#[allow(deprecated)]` on test helper (assert_cmd v3 upgrade deferred).

## Prevention

Pin formatting config in the repo, not just the toolchain version. `rustfmt.toml` with an explicit `style_edition` makes formatting deterministic regardless of which rustfmt version is installed — the style rules are versioned independently of the tool.

## Cross-References

- [xurl transport layer solution](../architecture-patterns/xurl-subprocess-transport-layer.md) — PR #8 where this CI failure surfaced
