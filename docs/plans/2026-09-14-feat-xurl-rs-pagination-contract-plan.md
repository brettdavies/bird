---
title: xurl-rs pagination contract and scope policy - Plan
type: feat
date: 2026-09-14
topic: xurl-rs-pagination-contract
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
target_repo: brettdavies/xurl-rs
---

# xurl-rs pagination contract and scope policy - Plan

This plan is for work in `xurl-rs`. It is staged in `bird` because `xurl-rs` had in-flight work on
`chore/align-with-repo-standard` when it was written; move it to `xurl-rs/docs/plans/` when that repo is free.

## Goal Capsule

- **Objective:** `xurl-rs` 3.2.0 ships the pagination and result-bounding contract as working code: an endpoint
  registry, a pure plan, a page loop bound to an explicit budget, crate-wide error classification, the lookups a list
  consumer needs, and the published documents with version constants. Any consumer, `bird` or otherwise, gets floors,
  ceilings, dedup, lossless cursors, and structured stops without writing a loop.
- **Means:** Add the registry and its two drift tests, `plan()` and `Budget`, `paginate()` over a pluggable page source,
  the classified error type, the user and usage operations, the `xr` surface for all of it, and the seam enforcement the
  scope policy names.
- **Product authority:** This plan owns the crate and `xr` changes the contract requires, the vendored prose evidence,
  and the seam enforcement hosted here. It does not own `bird`'s adoption, which is the cutover plan's scope.

## Sources

- Contract: `docs/designs/2026-09-03-pagination-and-result-bounding.md` (normative; D-rule numbers below refer to it).
- Scope policy: `docs/designs/2026-09-04-xurl-bird-seam.md` (placement and enforcement).
- Both currently live in `bird/docs/designs/`; U7 moves them here, which is their canonical home.

## Key Technical Decisions

- KTD1. **The registry is data with two drift tests, not generated code.** Spec-derived values (bounds, defaults, cursor
  parameter names and descriptions, time filters, expansion enums, `meta` keys, lookup ids ceilings) are re-derived from
  `vendor/x-api-openapi.json` by a test that fails on disagreement (D2.6). Prose-derived values (depth caps by request
  shape, expansion fan-out) carry the `content_sha256` of the vendored page that states them, and a second test fails
  when that page's body no longer matches (D2.4). Generation was rejected: a generated table is harder to review than a
  reviewed table with a failing test.
- KTD2. **`plan()` is pure and `paginate()` takes a `Budget`.** `plan()` performs no I/O and reads no clock, so every
  arithmetic row of the test matrix is a unit test. `paginate()` binds to the budget it is given, not to the plan, so a
  consumer with a spend model can let short pages fill toward `limit` while spend stays capped (D4.4).
- KTD3. **Page sizes sum to exactly `limit`.** `requests_max = ceil(L / C)`; every page is the ceiling except that when
  `L mod C` is positive and below the floor, the second-to-last page is shortened so the last lands exactly on the floor
  (D4.2). A `limit` below the floor is refused before any request, and a remaining count that falls below the floor
  mid-run stops the loop with `below_floor` rather than fetching more than was asked for (D1.5, D5).
- KTD4. **Error classification is crate-wide, status-first.** Every response, list or not, maps through one function:
  429 is `rate_limited` when the problem type says so or `x-rate-limit-remaining` is 0; 429 with `usage-capped` is
  `request_failed` carrying that problem; any other 429 is `request_failed` with an `ambiguous_429` problem carrying the
  raw body and observed headers; other 4xx and 5xx are `request_failed` (D2.8). This closes the plan-era gap where
  `XurlError::Api` carried only `status` and `body`, so no consumer could see a reset time.
- KTD5. **Transports are crate types.** The HTTP client, the per-request timeout, the page source, and the lookup source
  are all crate-defined, and no HTTP-client type appears in a public signature. A compile-time conformance test
  constructs each from `xurl_rs` paths only, so a leak fails to compile here rather than forcing a client to add
  `reqwest` to spell a type.
- KTD6. **`StopReason` is closed for this contract version and marked non-exhaustive**, so adding a stop later is not a
  breaking change for consumers that match on it (D3.4).
- KTD7. **The documents ship through rustdoc, never through a binary.** `#![doc = include_str!(...)]` puts the contract
  and the scope policy on docs.rs for every published version; `CONTRACT_VERSION` and `SEAM_VERSION` are constants equal
  to each document's frontmatter `version`, set when its `status` is `accepted`; `xr --help` links to the contract at
  the running binary's version tag (D9.2). No `xr contract` subcommand.

## Requirements

- R1. The registry records, per list endpoint: path, list class, floor, ceiling, API default where the spec declares
  one, cursor parameter, time-filter applicability, depth cap as a function of request shape, primary resource kind, and
  per accepted expansion the included kind and its fan-out.
- R2. The registry records, per lookup endpoint a list consumer needs, the ids ceiling per request (`/2/users` `ids` and
  `/2/users/by` `usernames`, both 100).
- R3. A test re-derives every spec-sourced registry value from the vendored OpenAPI document and fails on disagreement.
- R4. A test asserts every prose-sourced registry value's evidence checksum still matches the vendored page.
- R5. `plan(&ListRequest) -> Plan` is pure and returns `effective_limit`, `depth_capped`, `requests_max`, `objects_max`
  by kind, `ceiling_is_bound`, and the default `Budget`.
- R6. `plan_lookup(n) -> LookupPlan` returns the requests a lookup of `n` ids needs at that endpoint's ids ceiling.
- R7. `paginate(&ListRequest, page source, Budget)` yields pages and a terminal stop, binding to the budget, sizing
  pages per KTD3, deduplicating by id, guarding two consecutive empty pages, and never exceeding the budget.
- R8. Every stop carries the envelope fields of D3.5: `returned`, `requests_made`, `objects_fetched` by kind,
  `duplicates_dropped`, the rate-limit triple with `reset_at` as ISO 8601, and a lossless `next_cursor` per D3.2.
- R9. `--wait-for-rate-limit` is opt-in and lives in the loop: a `rate_limited` stop whose reset falls before the
  deadline becomes a sleep and a continuation, recorded in `waits` (D3.9).
- R10. Error classification per KTD4, exposed as a public error type carrying the mapped problem.
- R11. `lookup_users(ids, lookup source)`, `lookup_users_by_username(usernames, lookup source)`, `usage_credits()`, and
  `usage_posts()` exist as crate operations, chunking by the registry's ids ceilings.
- R12. The registry is public and printable; `xr endpoints [--json]` prints it.
- R13. Every endpoint in the registry, list or lookup, has an `xr` subcommand.
- R14. `xr` list commands take `--limit`, `--cursor`, `--page-size`, `--dry-run`, `--wait-for-rate-limit`, `--quiet`,
  `--verbose`, `--timeout`; time-ordered lists add `--since-id`, `--until-id`, `--start-time`, `--end-time`, and the
  endpoint's own `--exclude` and `--sort-order` where the API accepts them; `--expand` takes API expansion names.
- R15. `xr` exit codes: 0 natural, 75 partial data, 80 bounded stop with nothing delivered. 81 is reserved for a
  consumer's pre-flight refusal and `xr` never emits it.
- R16. The contract and the scope policy are included in rustdoc, `CONTRACT_VERSION` and `SEAM_VERSION` exist, and a
  test parses each included document's frontmatter and asserts the constant matches.
- R17. `xr --help` prints a link to the contract at the running binary's version tag.
- R18. The vendored prose evidence lives at `vendor/x-api-docs/` with `INDEX.md` and its re-vendor procedure.
- R19. Seam enforcement per the scope policy: ast-grep rule files, the Seam-section reusable workflow, the money ban on
  crate and `xr`, the page-loop ban on `xr`, the subcommand-coverage check, the transport conformance test, and a golden
  `ListMeta` snapshot the envelope tests assert against.

## Implementation Units

### U1. Vendored evidence

- **Goal:** `vendor/x-api-docs/` holds the prose sources the registry and the contract cite, byte-exact and checksummed.
- **Requirements:** R18.
- **Dependencies:** none.
- **Files:** `vendor/x-api-docs/*.md`, `vendor/x-api-docs/INDEX.md`, `vendor/README.md`.
- **Approach:** Fetch each `source_url` with its `.md` suffix, write the frontmatter above the verbatim body, and verify
  the body hashes to the recorded `content_sha256`. Thirteen files, listed in the manifest below, all verified against
  upstream on 2026-09-14 with zero drift. Endpoint reference pages are deliberately excluded: each embeds the OpenAPI
  document almost in full, which `vendor/x-api-openapi.json` already supplies. `INDEX.md` carries one row per file and
  the re-vendor procedure (re-fetch, recompute, diff, re-open any decision the `grounds` column names).
- **Execution note:** `vendor/**` is already in the repo's markdownlint ignores, which is why the evidence belongs here.
  A formatter pass over these files breaks every checksum; that is exactly how the first staging copy was corrupted.
- **Verification:** every body hashes to its recorded checksum; `INDEX.md` lists every file.

| file                                              | source_url                                                                                     | content_sha256     |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ------------------ |
| `agentnative-p7-bounded-high-signal-responses.md` | github.com/brettdavies/agentnative blob 6952816 principles/p7-bounded-high-signal-responses.md | `34ff917a373e3d9d` |
| `fundamentals-data-dictionary.md`                 | docs.x.com/x-api/fundamentals/data-dictionary                                                  | `fc4db015c23b8a22` |
| `fundamentals-fields.md`                          | docs.x.com/x-api/fundamentals/fields                                                           | `0f24f43a5dcd6b79` |
| `fundamentals-pagination.md`                      | docs.x.com/x-api/fundamentals/pagination                                                       | `1d994c15cc47e10f` |
| `fundamentals-post-cap.md`                        | docs.x.com/x-api/fundamentals/post-cap                                                         | `d322ff5e90a162fd` |
| `fundamentals-rate-limits.md`                     | docs.x.com/x-api/fundamentals/rate-limits                                                      | `243859fc611b9077` |
| `fundamentals-response-codes-and-errors.md`       | docs.x.com/x-api/fundamentals/response-codes-and-errors                                        | `4789041f5d4b8448` |
| `getting-started-pricing.md`                      | docs.x.com/x-api/getting-started/pricing                                                       | `df0b3dfe34fd0e1b` |
| `media-introduction.md`                           | docs.x.com/x-api/media/introduction                                                            | `da1baff93e177ae9` |
| `posts-search-integrate-paginate.md`              | docs.x.com/x-api/posts/search/integrate/paginate                                               | `8ee040e9318e424c` |
| `posts-timelines-integrate.md`                    | docs.x.com/x-api/posts/timelines/integrate                                                     | `6d447f496c3b32c5` |
| `usage-get-usage-credits.md`                      | docs.x.com/x-api/usage/get-usage-credits                                                       | `7d8b3fc08f0a9181` |
| `usage-introduction.md`                           | docs.x.com/x-api/usage/introduction                                                            | `93a700fcc118ccb9` |

### U2. Endpoint registry and its drift tests

- **Goal:** One reviewed table of API facts, guarded against both spec drift and prose drift.
- **Requirements:** R1, R2, R3, R4, R12.
- **Dependencies:** U1.
- **Files:** `src/registry/` (new), `tests/registry_spec_drift.rs`, `tests/registry_evidence_drift.rs`.
- **Approach:** Encode the eleven list endpoints of the contract's bounds table plus the two lookup endpoints. Depth cap
  is a function of request shape, not a scalar: user posts 3,200 or 800 with `exclude=replies`, mentions 800, home
  timeline 3,200 with its seven-day bound recorded as unplannable. Fan-out follows the contract's D2.3 table, including
  the expansions marked unbounded.
- **Patterns to follow:** `build.rs` already reads `vendor/x-api-openapi.json` for the auth matrix; the drift test reads
  it the same way.
- **Test scenarios:** every spec-sourced value matches the vendored spec; every prose-sourced value's checksum matches;
  an edited vendored page fails the evidence test; a changed bound fails the spec test.
- **Verification:** both tests green; `cargo test` fails when either vendored input is altered.

### U3. Plan, Budget, and lookup plan

- **Goal:** The worst case of any list or lookup is a pure function.
- **Requirements:** R5, R6.
- **Dependencies:** U2.
- **Files:** `src/list/plan.rs` (new), `src/list/budget.rs` (new).
- **Approach:** Implement D4.1 to D4.3. `effective_limit = min(limit, depth cap)`. Page sizes per KTD3. Expansions add
  `objects_max[primary] × fan-out` per requested expansion; an unbounded expansion clears `ceiling_is_bound` and reports
  that kind as unbounded. `Budget` carries requests, objects by kind, deadline, and the wait flag.
- **Test scenarios:** matrix rows 3 to 7. mentions limit 3 is refused naming the floor of 5; search limit 103 gives [93,
  10]; search limit 205 gives [100, 95, 10]; user posts with `exclude=replies` and limit 1000 gives `effective_limit`
  800 and `depth_capped`; a pinned page size gives `ceil(L/P)` requests and is the only plan that may exceed `L`;
  `plan_lookup(100)` is one request and `plan_lookup(101)` is two.
- **Verification:** table-driven unit tests over every registry row, no network, no clock.

### U4. The page loop

- **Goal:** One loop that every consumer shares, bound to a budget, with a closed stop set and a lossless cursor.
- **Requirements:** R7, R8, R9.
- **Dependencies:** U3, U5.
- **Files:** `src/list/paginate.rs` (new), `src/list/page_source.rs` (new), `src/list/meta.rs` (new).
- **Approach:** Implement D2.7, D3.1 to D3.9, D5. The loop carries the unique-id set, the consecutive-empty counter, the
  last rate-limit triple, the deadline, the in-flight cursor, the waits taken, and the partial-error accumulator. The
  page source is a trait with one operation answering with a typed page, a decline (`cache_miss`), or an error; the
  crate's HTTP source never declines.
- **Execution note:** Build test-first against a scripted page source: short pages, duplicates, empty pages, a 429 on
  page three, a decline, a deadline.
- **Test scenarios:** matrix rows 8 to 18 and 21 to 22. Short pages and duplicates never exceed the budget and end with
  `plan_cap`; a second run from `next_cursor` delivers the rest with no gap and no repeat; two consecutive empty pages
  with a cursor stop `empty_pages` and one does not; a remaining count under the floor stops `below_floor` with a
  cursor; `depth_cap` reports no cursor; `newest_id` is the maximum over delivered items under both sort orders; a
  declining source stops `cache_miss` and a source answering without a request adds nothing to the counts; an exhaustive
  downstream match on `StopReason` needs a wildcard.
- **Verification:** all scenarios green; no test touches the network.

### U5. Error classification

- **Goal:** One mapping for every response the crate makes, list or not.
- **Requirements:** R10.
- **Dependencies:** U2.
- **Files:** `src/error/classify.rs` (new), `src/error/mod.rs`.
- **Approach:** Implement D2.8. Carry the problem `type`, `title`, `detail`, and status when present, and the legacy
  `errors[].code` body as-is. Surface the `x-rate-limit-*` triple on every response so a consumer can see a reset time.
- **Test scenarios:** matrix rows 19 to 20. A typed 429, a 429 with `x-rate-limit-remaining` 0, a `usage-capped` 429, a
  typeless 429 with remaining above zero, a 4xx, a 5xx; a single lookup and a write map identically to the list loop.
- **Verification:** scenarios green; the ambiguous case carries the raw body and the observed headers.

### U6. Lookups and usage operations

- **Goal:** The operations a list consumer needs, chunked and injectable.
- **Requirements:** R11.
- **Dependencies:** U2, U5.
- **Files:** `src/lookup/mod.rs` (new), `src/usage/mod.rs` (new).
- **Approach:** `lookup_users` and `lookup_users_by_username` chunk by the registry ids ceiling and take a lookup source
  so tests can script them; `usage_credits` and `usage_posts` are typed so no consumer builds a `/2/` path.
- **Test scenarios:** matrix row 48. 100 ids make one request, 101 make two, and `plan_lookup` agrees with both.
- **Verification:** scenarios green through a scripted lookup source.

### U7. Published documents and version constants

- **Goal:** Any consumer reads the rules for the exact crate version it builds against.
- **Requirements:** R16, R17.
- **Dependencies:** U1.
- **Files:** `docs/designs/2026-09-03-pagination-and-result-bounding.md`, `docs/designs/2026-09-04-xurl-bird-seam.md`,
  `src/lib.rs`, `src/cli/help.rs`, `Cargo.toml` (packaging includes).
- **Approach:** Move both documents here from `bird/docs/designs/`, set `status: accepted`, include them with `#![doc =
  include_str!(...)]`, and expose `CONTRACT_VERSION` and `SEAM_VERSION`. A test parses each included document's
  frontmatter and asserts the constant matches. `xr --help` prints the contract link at the binary's version tag.
- **Execution note:** the constants exist only once the documents reach `accepted`; that is the release a client pins
  before it can assert its cited versions.
- **Verification:** `cargo doc` renders both; the frontmatter test is green; `xr --help` shows the versioned link.

### U8. The `xr` surface

- **Goal:** Every crate capability is reachable from the command line with the contract's flags and exit codes.
- **Requirements:** R13, R14, R15.
- **Dependencies:** U4, U6, U7.
- **Files:** `src/cli/` (list commands, `endpoints`, lookup and usage commands), `src/cli/exit.rs`.
- **Approach:** Wire the flag set, print the envelope without `cost` or `checkpoint`, and map stop categories to exit
  codes 0, 75, and 80. Reserve 81 without emitting it. `xr endpoints [--json]` prints the registry.
- **Test scenarios:** matrix rows 23 to 26. Every list command accepts the flag set; `meta` core equals the exported
  golden snapshot; `--dry-run` prints the plan and makes no request; `xr endpoints --json` and the help link render;
  exit codes per category with no collision with existing codes.
- **Verification:** smoke tests green; no existing exit code is reused.

### U9. Seam enforcement

- **Goal:** The scope policy is checked, not remembered.
- **Requirements:** R19.
- **Dependencies:** U7, U8.
- **Files:** `docs/designs/seam-rules/*.yml` (ast-grep), `.github/workflows/seam-section.yml` (reusable),
  `tests/seam_transport.rs`, `tests/seam_money.rs`, `tests/seam_subcommand_coverage.rs`, `src/list/meta.rs` (golden
  export).
- **Approach:** One ast-grep rule per anti-pattern, scanning non-test source only, never comments or fixtures, with
  exact item-name matches and the money ban as a snake_case-segment match. The Seam-section workflow parses each design
  doc's `## Seam` section, validates the table shape and the value sets, accepts the pointer and `n/a` forms, and emits
  a deterministic placement index as a build artifact that nothing commits. Every check fails with the same message
  shape: anti-pattern name, file and line or document and row, and a link to the section of the policy that states the
  rule at the version in use.
- **Test scenarios:** a planted violation of each rule fails red before the rule is enabled; a design doc without a `##
  Seam` section fails; a doc with an invalid Surface value fails naming the allowed set.
- **Verification:** every rule proven red on a planted violation, then green.

## Sequencing

U1 and U2 first, then U3 and U5 in parallel, then U4, then U6, then U7, U8, and U9. U7 gates the release that `bird`
pins. `bird`'s cutover plan cannot start its list work until this plan ships a release.

## Verification Contract

- Crate and `xr` rows of the contract's test matrix, all green.
- Both registry drift tests fail on an altered vendored input.
- No test reaches the network.
- Every seam rule proven red on a planted violation before it is enabled.

## Definition of Done

- `xurl-rs` 3.2.0 is released with the registry, plan, budget, page loop, classification, lookups, usage operations, the
  `xr` surface, and the published documents with version constants.
- `vendor/x-api-docs/` holds the thirteen verified evidence files with `INDEX.md`.
- The seam rules, the Seam-section workflow, and the placement-index artifact exist and are green.

## Seam

Pointer: every placement this plan implements is recorded in the worked matrix of
`docs/designs/2026-09-04-xurl-bird-seam.md`.
