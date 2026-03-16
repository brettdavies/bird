---
title: "feat: OpenAPI type foundation with typify codegen"
type: feat
status: completed
date: 2026-02-17
deepened: 2026-02-17
depends_on: null
blocks: "2026-02-17-refactor-entity-store-cache-replacement-plan.md (Phase 3 fields.rs + Phase 4 command migration)"
brainstorm: docs/brainstorms/2026-02-17-entity-store-cache-redesign-brainstorm.md
---

# OpenAPI Type Foundation

## Enhancement Summary

**Deepened on:** 2026-02-17
**Research agents used:** architecture-strategist, code-simplicity-reviewer, performance-oracle, security-sentinel, pattern-recognition-specialist, framework-docs-researcher, best-practices-researcher

### Key Revisions from Research

1. **Switched from `build.rs` to pre-generation** -- All 7 agents converged on this. Oxide Computer (typify's authors) recommend pre-generating and committing types for infrequently-changing specs. Pre-generation eliminates IDE/rust-analyzer issues with `include!()` + `OUT_DIR`, avoids 4-5s clean build penalty, and produces reviewable diffs.
2. **CRITICAL: typify produces 164 compilation errors** against the real X API spec -- 154 `::chrono::` references (date-time fields), 12 conflicting `TryFrom` implementations, 66 `::regress::Regex` references. Phase 1 validation is now a hard gate. **Selective generation of ~15-20 used schemas is the recommended path** given the volume of errors against the full 590-schema spec.
3. **Resolved spec path conflict** -- Spec already vendored at `openapi/x-api-openapi.json` (737KB, 24,735 lines, 414 schemas). Plan no longer proposes creating a `spec/` directory.
4. ~~**Added integrity verification**~~ -- Removed. Git provides integrity tracking; a separate SHA-256 checksum file is redundant (YAGNI).
5. **Fixed import patterns** -- Explicit named imports replace `use crate::fields::*` (zero glob imports exist in codebase).

---

## Overview

Pre-generate typed Rust structs from the vendored X API v2 OpenAPI specification using `cargo-typify`, and define canonical field sets for all API requests. This establishes the single source of truth for all API entity types (Tweet, User, Media, etc.) used across the codebase, replacing the current `serde_json::Value` dynamic parsing. This plan is a standalone deliverable -- no existing behavior changes.

## Problem Statement / Motivation

The codebase currently has **zero typed entity definitions**. Every command parses API responses with `serde_json::Value` and `.get()` chains. Each command defines its own field constants independently:

- `src/search.rs:12-14` -- `TWEET_FIELDS`, `USER_FIELDS`, `EXPANSIONS`
- `src/thread.rs:12-15` -- different `TWEET_FIELDS`, same `USER_FIELDS`
- `src/watchlist.rs:256-258` -- inline query param construction
- `src/bookmarks.rs` -- no field specification at all (uses API defaults)

This violates DRY and makes it impossible to build an entity store (Plan 2) without first having typed, shared entity definitions.

## Proposed Solution

Use **cargo-typify** (by Oxide Computer) to pre-generate Rust structs from the X API's official OpenAPI 3.0.0 spec already vendored at `openapi/x-api-openapi.json`. Generated types are committed to the repo as `src/types/generated.rs` -- no `build.rs` or `include!()` needed.

Additionally, define a canonical field set module that specifies what fields and expansions to always request from the API.

### Why Pre-Generation over build.rs

| Concern | build.rs + include!() | Pre-generate + commit |
|---------|----------------------|----------------------|
| IDE support | rust-analyzer issues: OOM (#13807), no autocomplete (#7400), changes not picked up (#18916), concat! indirection (#11777) | Full IDE support -- it's a regular .rs file |
| Build time | +4-5s on clean build. No incremental compilation for build scripts. | Zero overhead -- types already compiled |
| Reviewability | Generated code invisible in git | Full diff review on spec updates |
| Build deps | typify, schemars, prettyplease, syn in [build-dependencies] | Zero build dependencies (cargo-typify is a dev tool) |
| Spec update frequency | X API spec changes rarely (months) | Pre-generation is ideal for infrequent changes |
| Oxide recommendation | Works but not preferred for stable specs | Recommended approach by typify's authors |

## Technical Approach

### Phase 1: Validate Spec + typify Compatibility (Hard Gate)

This phase MUST succeed before any subsequent work. It de-risks the entire strategy.

**Tasks:**

- [x] ~~SHA-256 checksum file~~ -- Removed (git provides integrity tracking; checksum file is redundant per YAGNI)
- [x] Install cargo-typify: `cargo install cargo-typify`
- [x] Extract `components/schemas` from the OpenAPI spec into a JSON Schema document for typify:
  ```bash
  # typify expects JSON Schema, not raw OpenAPI.
  # Extract schemas section, wrap in JSON Schema envelope.
  # Script: scripts/extract-schemas.sh (new)
  ```
- [x] Run typify against extracted schemas -- document all compilation errors
- [x] Categorize errors by type:
  - `::chrono::` references (154 expected) -- date-time format fields
  - `TryFrom` conflicts (12 expected) -- overlapping numeric range constraints
  - `::regress::Regex` references (66 expected) -- pattern-validated strings
  - Other issues (oneOf/anyOf/allOf edge cases)

**Validation Decision Tree:**

```
typify output compiles cleanly?
├── YES → Proceed to Phase 2a (full generation)
└── NO → Attempt spec patching (Phase 1b)
         ├── Patched spec compiles? → Proceed to Phase 2a
         └── Still failing? → Selective generation (Phase 2b) ← MOST LIKELY PATH
```

**Phase 1b: Spec Patching (if needed):**

- [x] Create `scripts/patch-spec.py` (or `.sh`) that applies reproducible patches:
  - Replace `"format": "date-time"` with `"type": "string"` (fixes chrono refs -- we parse timestamps ourselves)
  - Remove `"pattern"` fields that trigger regress (we don't validate regex patterns at the type level)
  - Resolve oneOf/anyOf conflicts causing TryFrom errors
- [x] Re-run typify against patched schemas
- [x] Document all patches with rationale in `openapi/PATCHES.md`

**Recommended Path -- Selective Generation (~15-20 schemas):**

Given the known 164 compilation errors against the full 590-schema spec, selective generation is the most pragmatic path. Generate types only for schemas bird actually uses.

- [x] Identify the ~15-20 schemas bird actually uses: Tweet, User, Media, Place, Poll, Expansions, Error, etc.
- [x] Extract only those schemas into a focused JSON Schema document
- [x] Generate types from the subset (~3-5K lines vs 34K for full spec)
- [x] Hand-write any remaining types that typify cannot handle

**Files:**

- ~~`openapi/x-api-openapi.json.sha256`~~ (removed -- git provides integrity)
- `scripts/generate-types.sh` (new -- consolidates schema extraction, optional patching, and typify invocation into one idempotent script)
- `openapi/PATCHES.md` (new, if patches applied)

### Phase 2: Pre-Generate and Commit Types

**Phase 2a: Full Generation (preferred path):**

- [x] Run cargo-typify with settings:
  - Derives: `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize`
  - Do NOT derive `Debug` with raw field output for types containing sensitive data -- consider a wrapper or `#[allow(dead_code)]` on unused types
  - Output to `src/types/generated.rs`
- [x] Add `// @generated by cargo-typify — DO NOT EDIT` header to generated file
- [x] Add to `.gitattributes`: `src/types/generated.rs linguist-generated=true`
- [x] Create `src/types/mod.rs` that re-exports generated types:
  ```rust
  #[allow(clippy::all, unused)]
  mod generated;
  pub use generated::*;
  ```
- [x] Add `mod types;` to `src/main.rs` (alphabetical order, between `thread` and `usage`)
- [x] Add runtime dependencies to `Cargo.toml` if generated code requires them:
  - `chrono` -- already present (check: only if date-time types survive patching)
  - `regress` -- new dependency (check: only if pattern types survive patching)
- [x] Verify `cargo build` succeeds
- [x] Verify `cargo clippy` passes (with `#[allow(clippy::all)]` on generated module)
- [x] Add regeneration script `scripts/generate-types.sh` (consolidates schema extraction, patching, and typify invocation):
  ```bash
  #!/usr/bin/env bash
  set -euo pipefail
  # 1. Extract schemas from OpenAPI spec into JSON Schema
  # 2. Apply patches if needed (remove date-time format, regress patterns)
  # 3. Run cargo-typify
  # 4. Prepend @generated header
  # 5. Output to src/types/generated.rs
  ```

**Phase 2b: Selective Generation (fallback):**

- [x] Same as 2a but with focused schema subset
- [x] Hand-write types for any schemas typify cannot handle
- [x] Add test comparing hand-written types against actual API response shapes

**Files:**

- `src/types/mod.rs` (new)
- `src/types/generated.rs` (new -- generated, committed)
- `.gitattributes` (new or modified)
- `src/main.rs` (modified -- add `mod types`)
- `Cargo.toml` (modified -- add runtime deps if needed)
- `Makefile` or `scripts/regenerate-types.sh` (new)

### Phase 3: Canonical Field Sets

**Tasks:**

- [x] Create `src/fields.rs` module defining canonical field sets as `pub const` strings
- [x] Define `TWEET_FIELDS`: all available tweet fields from the spec (created_at, text, author_id, conversation_id, public_metrics, referenced_tweets, in_reply_to_user_id, attachments, entities, etc.)
- [x] Define `USER_FIELDS`: all available user fields (username, name, created_at, public_metrics, description, profile_image_url, location, verified, url, etc.)
- [x] Define `MEDIA_FIELDS`: all available media fields
- [x] Define `EXPANSIONS`: comprehensive expansion set (author_id, referenced_tweets.id, attachments.media_keys, etc.)
- [x] Define helper function to build query parameter pairs:
  ```rust
  pub fn tweet_query_params() -> Vec<(&'static str, &'static str)> {
      vec![
          ("tweet.fields", TWEET_FIELDS),
          ("user.fields", USER_FIELDS),
          ("media.fields", MEDIA_FIELDS),
          ("expansions", EXPANSIONS),
      ]
  }
  ```
- [x] Add `mod fields;` to `src/main.rs` (alphabetical order, between `doctor` and `login`)
- [x] Write unit tests verifying field strings are well-formed (no trailing commas, no leading/trailing spaces, no empty segments)

**Files:**

- `src/fields.rs` (new)
- `src/main.rs` (modified -- add module)

### Phase 4: Migrate Commands to Canonical Fields

This phase is required before Plan 2 begins. Plan 2's entity store depends on all commands using canonical field sets for consistent entity decomposition. Replace per-command field constants with explicit imports from `src/fields.rs`.

- [x] Replace `search.rs` TWEET_FIELDS/USER_FIELDS/EXPANSIONS with `use crate::fields::{TWEET_FIELDS, USER_FIELDS, EXPANSIONS}`
- [x] Replace `thread.rs` field constants with `use crate::fields::{TWEET_FIELDS, USER_FIELDS, EXPANSIONS}`
- [x] Replace `watchlist.rs` inline field construction with canonical fields
- [x] Add field specification to `bookmarks.rs` (currently missing) using canonical fields
- [x] Replace `profile.rs` USER_FIELDS with canonical fields
- [x] Verify all commands still pass tests (129/129 pass)

**Files:**

- `src/search.rs` (modified)
- `src/thread.rs` (modified)
- `src/watchlist.rs` (modified)
- `src/bookmarks.rs` (modified)

## Acceptance Criteria

- [x] `openapi/x-api-openapi.json` vendored (integrity tracked by git)
- [x] `cargo build` succeeds with typify-generated types (pre-generated, committed)
- [x] Generated types include at minimum: Tweet, User, Media, Place, Poll schemas
- [x] All generated types derive `Serialize`, `Deserialize`, `Clone`, `Debug`
- [x] Generated file marked `linguist-generated=true` in `.gitattributes`
- [x] `src/fields.rs` defines canonical field sets with `pub const` visibility
- [x] Commands use explicit named imports (no glob imports)
- [x] No existing tests broken (129/129 pass)
- [x] `cargo clippy` clean (with `#[allow(clippy::all)]` on generated module)
- [x] `cargo fmt` clean
- [x] Regeneration script/Makefile target documented

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| **typify cannot handle full X API spec** | **HIGH (confirmed)** | 164 compilation errors found against real spec. Spec patching (remove date-time format, remove pattern fields) as first fix. Selective generation (~15-20 schemas) as fallback. Phase 1 is a hard validation gate. |
| **Spec patching creates maintenance burden** | Medium | Patches are scripted and reproducible (`scripts/patch-spec.py`). Documented in `openapi/PATCHES.md`. Re-run on spec updates. |
| **Generated types are too large (34K lines, 590 structs)** | Medium | If full generation: `#[allow(unused)]` on module, linker strips unused code. If selective: ~3-5K lines only. Monitor compile time delta. |
| **X API spec has undocumented quirks** | Medium | Compare generated types against actual API responses. Save response fixtures for validation. |
| **Hidden runtime dependencies** | Medium | Generated code may require `chrono` (already present) and `regress` (new). Audit generated code for dependency requirements before committing. |
| **Generated Debug impl leaks sensitive data** | Low | Review generated Debug impls. Consider `#[derive(Debug)]` removal or custom wrapper for types containing tokens/credentials (unlikely for entity types). |
| **Pre-generated file gets manually edited** | Low | `@generated` header + `.gitattributes` + CI check that regeneration produces identical output. |

## References

- Brainstorm: `docs/brainstorms/2026-02-17-entity-store-cache-redesign-brainstorm.md` (Decisions 4, 5)
- typify: https://github.com/oxidecomputer/typify
- cargo-typify CLI: https://crates.io/crates/cargo-typify
- X API OpenAPI spec: https://api.x.com/2/openapi.json
- Vendored spec: `openapi/x-api-openapi.json` (414 schemas, 127 paths, 24,735 lines)
- Current field definitions: `src/search.rs:12-14`, `src/thread.rs:12-15`, `src/watchlist.rs:256-258`
- rust-analyzer include!() issues: #13807, #7400, #18916, #11777
