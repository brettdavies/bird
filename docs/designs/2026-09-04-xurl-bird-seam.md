---
title: The xurl-rs scope policy and the seam to its clients
status: draft
version: 1
canonical: xurl-rs/docs/designs/2026-09-04-xurl-bird-seam.md
reference_client: bird
---

# The xurl-rs scope policy and the seam to its clients

This document is the scope policy of the `xurl-rs` crate. It decides two things about every proposed feature: whether
its implementation belongs inside the crate or in a client, and which command line exposes it. A contributor applies it
before opening a discussion, so the discussion is about the feature and not about the boundary. `bird` is the reference
client and supplies the worked matrix below; any other client conforms to the same rules in the same way. The pagination
and result-bounding contract (`docs/designs/2026-09-03-pagination-and-result-bounding.md`) is the crate's behaviour
contract and the source of the matrix rows.

## The three layers

| Layer           | What it is                                                                                          | Who consumes it                                        |
| --------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `xurl-rs` crate | The X API v2 as a Rust library: facts about the API and the mechanics every consumer needs the same | `xr`, `bird`, and any third-party Rust program         |
| `xr` CLI        | The crate on the command line: flag parsing, output formatting, and discovery commands              | People, agents, and any program driving a subprocess   |
| client          | A program with an account, a machine, and a memory, built on the crate. `bird` is the reference     | Its own users; `bird` serves agents and its maintainer |

The crate and `xr` are public. Anything placed in a client is unavailable to every other consumer; anything placed in
the crate is available to all of them. The third layer is named by its role so the rules below read as roles, the same
way the first two do.

## Two axes

Every concern gets two answers, recorded together:

- **Implementation:** `crate`, `client`, or `presentation`. Decided by the decision tree below. `presentation` means the
  contract defines the behaviour and each surface implements it; there is no shared code to place. Flag parsing is
  always presentation; a row named for a capability records where the behaviour behind its flags lives.
- **Surface:** `xr`, `client`, `both`, or `API`. Decided by the surface rule below. `API` means the concern is reachable
  only as crate API or rustdoc, with no command-line exposure. In the matrix below, `client` is `bird`.

A concern implemented in the crate and surfaced by a client is a passthrough. A passthrough is a surface decision, never
an implementation decision, and it never moves logic.

## Axis 1: implementation

A concern lives in the **crate** when it is true for every consumer with any account. It lives in a **client** when it
depends on that client's account, machine, or users' ergonomics. Apply the questions in order; the first "yes" decides.

```text
                       Is it a fact about the X API?
                       (endpoint, parameter, bound, wire shape, ordering,
                        cap, fan-out, error type, rate-limit header,
                        an ids ceiling on a lookup)
                                    |
                     yes ---------- + ---------- no
                      |                          |
                   CRATE                Is it a mechanical consequence of API facts
                (registry, types)       that every consumer must get identically?
                                        (page loop, floors and overflow, dedup,
                                         cursor losslessness, budget binding,
                                         error classification on any request,
                                         waiting on reset, chunking ids for a lookup,
                                         the worst case of a lookup)
                                                    |
                                     yes ---------- + ---------- no
                                      |                          |
                                   CRATE               Does it need a number or a policy
                                 (mechanics)           about money?
                                                       (prices, spend guard, hard cap, defaults)
                                                                   |
                                                    yes ---------- + ---------- no
                                                     |                          |
                                                   CLIENT             Does it persist across invocations
                                            (the crate provides         on one machine?
                                             the shape, such as         (cache, checkpoints, configuration,
                                             plan, budget, objects       a history of observations)
                                             by kind)                              |
                                                                    yes ---------- + ---------- no
                                                                     |                          |
                                                                   CLIENT             Is it ergonomics for the client's
                                                            (the crate provides         own users?
                                                             the hook, such as a        (hydration from cache, checkpoint
                                                             page source or a           mode, cache-only, refusal text
                                                             budget parameter)          that names a flag, a trend
                                                                                        computed from stored observations)
                                                                                                    |
                                                                                     yes ---------- + ---------- no
                                                                                      |                          |
                                                                                    CLIENT            PRESENTATION: flags,
                                                                                                      formatting, exit codes,
                                                                                                      envelope core. The contract
                                                                                                      fixes it once; every surface
                                                                                                      that exposes it implements
                                                                                                      it identically.
```

## Axis 2: surface

1. `xr` exposes every capability the crate implements, where a capability is an operation a caller can invoke (a list, a
   lookup, a usage query, a plan). Types, constants, and documentation are crate API and rustdoc, surface `API`. `xr`
   holds nothing but flag parsing, output formatting, and discovery commands that print what the crate knows (`xr
   endpoints`, the versioned link to the contract in `--help`).
2. A client exposes a crate capability when its users need it. That surface is a **passthrough**: it calls the crate,
   applies the contract's presentation rules, and adds only client fields (in `bird`: `cost`, `checkpoint`, `cache`,
   `hydration`) where they apply. It never re-implements, filters, reorders, or alters what the crate returns; the
   crate's own `meta` fields keep the crate's values. A client capability invoked by its own flag (hydration) may add
   objects to `includes` in the API's shape. A passthrough MAY refuse an input before any request for a money or
   ergonomics reason, and that refusal is its own matrix row.
3. Surface records command-line exposure only. A client capability that calls a crate capability internally (hydration
   calling the user lookup) does not make that crate capability a passthrough; the internal call is recorded on the
   client row.
4. A client exposes the capabilities that `xr` cannot have (in `bird`: the spend guard, hydration, checkpoint mode, and
   cache-only).
5. Presentation (flags, exit codes, envelope core) is defined once by the contract and implemented identically on every
   surface that exposes it, so the same request through `xr` and through a client passthrough returns the same `data`
   and the same `meta` core.
6. A concern with implementation `crate` and surface `both` is a passthrough in the client by definition. A concern with
   implementation `client` has surface `client` only.

## Tie-breakers

When a concern seems to sit on the line, these settle it.

1. **The third-party test.** If a developer using the crate without `bird` would need it, it is crate. A page loop, a
   budget, an error classification, id chunking for a lookup, and a wait-on-reset all pass this test. A price table does
   not: prices are the account holder's concern and change on a schedule the spec does not know.
2. **The divergence test.** If placing it in a client would make `xr` and that client answer the same request
   differently, and the difference is not about money, state, or ergonomics, it belongs in the crate. Two surfaces that
   page a list differently is a bug in the seam, not a feature of either tool.
3. **The shape-versus-value test.** The crate owns the shape of a concern that a client fills with values, when the
   shape describes API data or API mechanics that a third-party consumer would also need. The crate defines `Budget`;
   `bird` derives its numbers from the spend guard. The crate defines the page source; `bird` supplies the cache-backed
   one. The crate defines `objects_max` by resource kind; `bird` multiplies by prices. A shape that exists only for a
   client's own state (a stored observation, a checkpoint record) stays in the client.
4. **No temporary forks.** A mechanical concern a client needs before the crate has it goes into the crate first and the
   client waits for the release. A fork in a client is the seam failing.
5. **Vocabulary is defined once.** The crate's contract owns every word. A client adds words only for its own mechanisms
   (in `bird`: `hydrate`, `checkpoint`, `cache only`, `hard cap`, `spend`) and never redefines a crate word.

## Worked matrix from the pagination contract

Implementation `client` and surface `client` are `bird` in every row.

| Concern                                                                   | Implementation | Surface | Question that decided it                  |
| ------------------------------------------------------------------------- | -------------- | ------- | ----------------------------------------- |
| Bounds, cursor parameter names, list class, depth caps, fan-out           | crate          | xr      | API fact; surfaced by `xr endpoints`      |
| Lookup-endpoint bounds (the ids ceilings on `/2/users` and `/2/users/by`) | crate          | xr      | API fact; surfaced by `xr endpoints`      |
| Evidence checksums on prose-sourced facts                                 | crate          | API     | API fact (its provenance); tests only     |
| Page loop, page-size rule, overflow, dedup by id                          | crate          | both    | mechanical consequence; passthrough       |
| User lookup by ids or usernames, chunked by the ids ceiling               | crate          | both    | API fact plus mechanical consequence      |
| Worst case of a lookup (`plan_lookup`)                                    | crate          | both    | mechanical consequence, pure arithmetic   |
| Usage and credits queries                                                 | crate          | both    | API fact; passthrough                     |
| Error classification on every request, including the typeless 429         | crate          | both    | API fact plus mechanical consequence      |
| List plan, default `Budget`, dry run                                      | crate          | both    | mechanical consequence, pure arithmetic   |
| `ceiling_is_bound` on the plan                                            | crate          | both    | mechanical consequence                    |
| Budget binding and `plan_cap`                                             | crate          | both    | mechanical consequence                    |
| Deadline in `Budget` and the `timeout` stop                               | crate          | both    | mechanical consequence                    |
| Wait on reset                                                             | crate          | both    | mechanical consequence of the headers     |
| Time and id filters passed on every page                                  | crate          | both    | API fact (the polling guide)              |
| API-native query parameters, field selections, expansion names            | crate          | both    | API fact; passthrough                     |
| Pinned page size validation                                               | crate          | xr      | crate mechanics; `bird` derives page size |
| `ListMeta` and `Page` types                                               | crate          | API     | mechanical consequence; public types      |
| Flag parsing and help text                                                | presentation   | both    | contract, implemented identically         |
| Serialization of the crate's `ListMeta`                                   | presentation   | both    | contract, implemented identically         |
| Exit codes 75 and 80                                                      | presentation   | both    | contract, implemented identically         |
| Exit code 81 (pre-flight refusal)                                         | presentation   | client  | contract; only a client refuses           |
| `StopReason` set and its non-exhaustive marking                           | crate          | API     | public API for third parties              |
| `CONTRACT_VERSION`, `SEAM_VERSION`, rustdoc inclusion                     | crate          | API     | third-party test; version guard           |
| Link to the contract at the binary's version                              | presentation   | xr      | discovery                                 |
| `xr endpoints`                                                            | presentation   | xr      | discovery                                 |
| Price table and its checksum                                              | client         | client  | money                                     |
| `estimate_max_usd` and the owned-read tier                                | client         | client  | money                                     |
| Spend guard, hard cap, defaults, refusal                                  | client         | client  | money policy                              |
| Refusal of an unbounded expansion                                         | client         | client  | money (the estimate is not a ceiling)     |
| Refusal of `--expand author_id` on post lists                             | client         | client  | money (a cached author costs nothing)     |
| Budget values derived from `max_spend`                                    | client         | client  | money (crate owns the shape)              |
| Entity cache, query cache, cache-only, the `cache` field                  | client         | client  | persistent state                          |
| Checkpoint record and checkpoint mode                                     | client         | client  | persistent state                          |
| Author hydration (cache first, then the crate lookup internally)          | client         | client  | ergonomics on top of the cache            |
| `cost`, `checkpoint`, `cache`, `hydration` envelope fields                | client         | client  | client mechanisms                         |
| Atomic per-page cache writes                                              | client         | client  | persistent state                          |

## Anti-patterns the seam forbids

- A table of API facts in a client. A client imports the registry; it never carries a copy.
- A page loop in a client or in `xr`. The crate pages; the surfaces call it.
- Prices, spend policy, or account configuration in the crate or in `xr`.
- Logic in `xr` beyond flag parsing, output formatting, and discovery commands.
- A client implementation of something a third-party crate user would need.
- A client passthrough that re-implements, filters, reorders, or alters what the crate returns.
- A crate capability with no `xr` surface.
- A word with one meaning in `xr` and another in a client.

## What a conforming client does

A client conforms to this policy when it can show, by its own tests, that it holds no copy of API facts, runs no page
loop, makes no HTTP request of its own to the X API, and reaches the registry only by import from the crate. The crate
makes this possible by exposing its client, per-request timeout, page source, and lookup source as crate types, so no
HTTP-client type appears in a public signature. `bird` lists the tests it runs in its client-conformance document; any
other client chooses its own.

## Enforcement in the crate and xr

The seam is checked where a check is possible and reviewed where it is not.

1. **Every design doc carries a `## Seam` section** in one of three forms: one row per concern with Concern,
   Implementation, Surface, and the deciding question; a pointer to the document whose matrix holds this document's
   rows; or the single line `n/a` with the reason no placement question arises. The check is a reusable workflow in
   this repository that parses the section: a four-column table with the exact header and values from the allowed
   sets (`crate`, `client`, `presentation`; `xr`, `client`, `both`, `API`), or the pointer line, or the `n/a` line
   with its reason; a failure names the file, the row, and the allowed values. The workflow also emits a deterministic
   placement index
   of every row it parsed as a build artifact; nothing generated is committed. `xurl-rs` runs it on its own
   `docs/designs/`, and a client that adopts the policy calls the same workflow on its own.
2. **Source and manifest checks**, each named for the anti-pattern it guards. The literal and definition scans are
   ast-grep rule files published in this repository beside this document, one rule per anti-pattern, applied to
   non-test source only, never to comments, documentation, or test fixtures. Name matches are exact item names, not
   substrings. `xr` and the crate run the rules from this repository; a client runs the same rule files at the crate
   version it pins.
   - crate: a compile-time conformance test constructs the client, the page source, and the lookup source from
     `xurl_rs` paths only and asserts their types are crate-defined; a public signature that needed an HTTP-client
     type would fail to compile there.
   - crate and `xr`: no identifier with a snake_case segment equal to `usd`, `price`, or `spend` (the money ban; the one
     segment-level check, so `suspended` does not match).
   - `xr`: no `pagination_token` or `next_token` string literal (the page-loop ban).
   - `xr`: every endpoint in the registry, list or lookup, has a subcommand (the capability-without-surface ban for
     registry endpoints).
   - Passthrough parity: the crate exports a golden `ListMeta` snapshot that `xr`'s envelope tests, and any client's,
     assert against, since no CLI's tests touch the network or another binary.
   Every check, in the crate, in `xr`, in the workflow, and in a client, fails with the same message shape: the
   anti-pattern name, the file and line (or the document and row), and a link to the section of this document that
   states the rule, at the crate version in use. The ast-grep rule ids are the anchors of those sections.
3. **Review-time checks.** These are judgment, not scans: a word with two meanings; a passthrough that alters what the
   crate returns beyond what the parity test can see; logic in `xr` beyond parsing, formatting, and discovery; a client
   implementation of something a third-party crate user would need; and an `xr` surface for every crate operation
   outside the registry (`plan`, `plan_lookup`, the usage queries). The `## Seam` section is where a reviewer checks
   them.
4. **Versioned with the crate.** The crate includes this document and the contract in its rustdoc (`#![doc =
   include_str!(...)]`), so docs.rs shows both for every published version, and exposes `SEAM_VERSION` and
   `CONTRACT_VERSION`, each set when the document's `status` is `accepted` and equal to its frontmatter `version`. A
   crate test parses each included document's frontmatter and asserts the constant matches. `xr --help` prints a link to
   the contract at the tag of the running binary's version; the contract links here. A client cites the versions it
   conforms to and MAY assert them against the constants of the crate it builds against; the constants exist once the
   documents reach `accepted`. No document ships inside a binary, and no client holds a copy. A change to a rule bumps
   `version` and is a crate release; a wording fix ships with the next one.

## How to apply it

Before a feature discussion, name each concern, walk axis 1 for its implementation and axis 2 for its surface, and
record the two answers with the deciding question as one row in the feature's `## Seam` section. A feature that needs a
crate shape and a client value produces two rows. A concern the tree cannot decide is a change to this document,
reviewed on its own and shipped with a `version` bump.

## Seam

n/a: this document defines the seam.
