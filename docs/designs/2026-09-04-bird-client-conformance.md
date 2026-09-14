---
title: bird client conformance to the xurl-rs scope policy
status: draft
conforms_to:
  seam: xurl-rs docs/designs/2026-09-04-xurl-bird-seam.md version 1
  contract: xurl-rs docs/designs/2026-09-03-pagination-and-result-bounding.md version 1
---

# bird client conformance to the xurl-rs scope policy

`bird` is the reference client of the `xurl-rs` crate. The crate's scope policy (the seam) and its behaviour contract
live in the `xurl-rs` repository and on docs.rs for every published crate version; `bird` holds no copy. This document
lists what `bird` does to conform and the tests that prove it.

## What bird cites

- The seam and the contract at the versions in this document's frontmatter, which equal the `SEAM_VERSION` and
  `CONTRACT_VERSION` constants of the crate version `bird` builds against. A test asserts both equalities, so `bird`
  cannot document a version it does not implement. That test lands in the
  same change that pins the first crate release exposing the constants, so there is neither a red window nor a
  skipped test.
- The vendored X API evidence in `xurl-rs/vendor/x-api-docs/` at the pinned crate version, for every API claim in
  `bird`'s own design docs.

## What bird never does

- Hold a table of API facts. The registry is reached by import from the crate.
- Run a page loop. The crate pages; `bird` supplies page sources.
- Make an HTTP request of its own to the X API. The crate's client, page source, and lookup source are the only
  transports.
- Re-implement, filter, reorder, or alter what the crate returns on a passthrough; `bird` adds only its own envelope
  fields (`cost`, `checkpoint`, `cache`, `hydration`).
- Redefine a contract word. `bird`'s own words are `hydrate`, `checkpoint`, `cache only`, `hard cap`, and `spend`.

## Conformance tests

Each test is named for the anti-pattern it guards. The literal and definition scans run the ast-grep rule files
`xurl-rs` publishes beside the seam, at the pinned crate version, on non-test source only, never on comments,
documentation, or test fixtures. Name matches are exact item names.

| Test                | Assertion                                                                                                                                     |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| no HTTP client      | none of `reqwest`, `hyper`, `ureq`, `isahc`, or `curl` among `bird`'s direct dependencies (`cargo metadata`)                                  |
| no API path literal | no string literal in `bird` source constructs an X API path (`/2/`)                                                                           |
| no registry copy    | no `max_results` token; no type, constant, or static named `Endpoint`, `Registry`, `ListClass`, `ResourceKind`, `FanOut`, or `DepthCap`       |
| registry by import  | a required `use` of the crate's registry type and a type-level assertion on `bird`'s accessor                                                 |
| no page loop        | no `pagination_token` or `next_token` string literal                                                                                          |
| passthrough parity  | `bird`'s envelope tests assert the `meta` core against the crate's exported golden `ListMeta` snapshot                                        |
| versions cited      | frontmatter versions above equal `SEAM_VERSION` and `CONTRACT_VERSION`                                                                        |
| Seam section valid  | every file under `docs/designs/` carries a `## Seam` section whose table, pointer, or `n/a` line passes the reusable workflow `xurl-rs` hosts |

## Seam

n/a: this document records `bird`'s conformance to the seam; its placements are the `client` rows of the seam's worked
matrix.
