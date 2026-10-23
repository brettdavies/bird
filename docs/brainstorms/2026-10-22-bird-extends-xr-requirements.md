---
title: bird extends xr in-process - Requirements
type: feat
date: 2026-10-22
topic: bird-extends-xr
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
superseded_by: docs/plans/2026-10-22-005-feat-bird-0-3-0-remaining-work-plan.md
---

# bird extends xr in-process - Requirements

## Goal Capsule

- **Objective:** A bird user drives every `xr` API command with `xr`'s verbs, flags, and output, and gets bird's cache,
  spend control, and its own commands on top, from one binary; an `xr` user sees no change at all.
- **Means:** xdk-rs gains a request hook and a list loop; xurl-rs exposes `xr`'s command layer as a supported library;
  bird composes that command layer and installs its cache and spend hooks on the client it supplies.
- **Product authority:** the maintainer. The constraints come from the parity backlog
  (`docs/plans/2026-10-22-001-xurl-rs-parity-backlog.md`, Rules and stack 3) and two statements of direction: bird does
  not recreate `xr`'s commands, and xurl-rs stays fully usable on its own. Stacks 4 and 5 of the backlog are context,
  not scope.
- **Open blockers:** bird's composition waits for the xdk-rs and xurl-rs releases that carry R3 to R16 and R30 to R34.
  The xurl-rs team owns their design and timing; bird states what it needs and adapts to what ships.

## Product Contract

### Summary

bird stops re-implementing `xr`. Its command set becomes `xr`'s API commands, composed in-process from the xurl-rs
library, plus the commands only bird has (thread, watchlist, cache, doctor, schema, completions, skill, a local spend
ledger, and post ranking). Every request either tool sends goes through one xdk-rs client, and bird's cache and spend
policy ride on that client as hooks, so a composed command is cached, priced, and recorded the same way bird's own
commands are. bird remembers the order of every list it fetches, so `--cache-only` replays a list at no cost.

### Problem Frame

bird and `xr` cover the same X API verbs twice. bird carries its own clap tree, its own request builders, its own
pagination loops (`src/search.rs`, `src/thread.rs`), and its own output envelope, and every one of them has drifted from
`xr`: bird calls the create verb `tweet` and the current-user lookup `me`, pages search with `next_token` where the
crate sends `pagination_token`, and prints `{"error","kind",...}` where `xr` prints `{"status":"error","reason",...}`.
Each `xr` release widens the gap (the `xr` verbs bird lacks: `quote`, `read`, `timeline`, `mentions`, `bookmark`,
`unbookmark`, `likes`, `following`, `followers`, `muted`, `blocked`, `dms`, `broadcasts`, `media`, `usage credits`).

The product split is settled: `xr` is the limited CLI that X can approve, and bird is the extended client for people who
integrate with local knowledge bases, import into databases, and cut pay-per-use cost with a cache. What bird adds is
state and money, not verbs. Composing `xr`'s commands lets bird inherit every verb, flag, and envelope at the version it
pins, and keeps bird's code to what only bird does.

Two things stand in the way today. The xurl-rs library target is `#[doc(hidden)]` and states it is not an embedder API
(`crates/xurl-cli/src/lib.rs`), so bird has no supported way to run `xr`'s commands. And xdk-rs sends every request
itself with no point where an embedder can answer from a store or record what X returned
(`crates/xdk/src/api/request/transport.rs`, `Client::send_request_with`), so a composed command would bypass bird's
cache entirely.

### Key Decisions

- **bird composes `xr`'s API commands instead of mapping its verbs onto `xr`'s names.** Every bird verb `xr` has retires
  and `xr`'s arrives; the renames the post-vocabulary cutover planned (`tweet` to `post`, `me` to `whoami`, `profile` to
  `user`) happen by retirement. Governs R17.
- **The hook lives on the client, not in the command layer.** xr's commands stay unaware of caching; any embedder of
  xdk-rs gets the same hook whether or not it composes `xr`. Governs R3 to R8, R18.
- **bird asks; xdk-rs and xurl-rs decide.** bird writes no code in xurl-rs: it states each need with the interface it
  codes against and the behavior it tests, and adapts to the names and shapes that ship. Governs R3 to R16, R23, R24,
  R30 to R34.
- **bird asks for a client factory, not a built client.** `xr`'s `--app`, `--timeout`, and `--wait-on-rate-limit` shape
  the client, so the command layer builds it from the parsed flags through a factory the embedder supplies. Governs R13.
- **Spend control is a second hook, on the list loop's plan.** The request hook sees one request at a time and cannot
  refuse a run before its first page; a budget policy sees the whole plan. Governs R10, R18.
- **block, unblock, and blocked stay.** The vendored X spec lists `/2/users/{id}/blocking` and `xr` ships all three
  verbs, so bird gets them by composition instead of removing them. Governs R17.
- **bird's raw HTTP verbs do not return.** `xr` is the raw tool; composition takes `xr`'s API commands and leaves its
  raw mode out. Governs R12, R21.
- **bird keeps its `BIRD_*` environment variables.** A user's `XURL_OUTPUT` set for `xr` does not change bird's output,
  and bird's documented variables keep working. Governs R19.
- **xr's `auth` family composes, and `bird login` is removed.** Sign-in is an `xr` capability bird shares the token
  store with. Governs R17.
- **bird's local ledger is `bird spend`.** `usage` belongs to `xr`, and `spend` is the word the pagination contract
  reserves for bird. Governs R22.
- **The query cache ships in 1.0.0; checkpoints and hydration do not.** A replayable list is what makes `--cache-only`
  useful for list commands. Governs R25 to R28.
- **Ranking is a bird command, not a `search` flag.** `xr`'s `search` composes flag for flag, so `--sort` and
  `--min-likes` leave it, and `bird rank` fetches through the same search request. Governs R29.

### Requirements

**xr stays whole**

- R1. The request hook and the library surface change nothing `xr` does: every command, flag, envelope, and exit code
  stays as it is, proved by its existing tests and goldens. `xr`'s list verbs paging through the list loop (R9) is the
  one `xr` change, an `xr` feature with its own changelog entry.
- R2. Nothing in xdk-rs or xurl-rs depends on bird or names a bird concept (cache, price, spend), and `xr` installs no
  hook.

**Needed from xdk-rs: request hook**

- R3. An embedder installs a request hook on an xdk-rs client built from credentials or from a token store.
- R4. Before a request is sent, the hook can answer it with a response body, which the client decodes as it would
  decode X's answer, sending nothing and needing no credential.
- R5. Before any request is sent, the hook can decline it, and the caller receives an error in place of a response.
- R6. After a request completes, the hook observes the request as sent (method, rendered URL, the auth scheme and user
  it went out under) and its outcome (the body, or the error with its HTTP status), once per logical request after any
  rate-limit retry.
- R7. Every request on the client's JSON path reaches the hook, including each page of a list and the caller-id lookup a
  shortcut makes on the caller's behalf; multipart uploads can be declined and are observed, never answered; streaming
  requests do not reach the hook.
- R8. A client with no hook behaves as it does today.

**Needed from xdk-rs: list loop**

- R9. A list call returns up to `limit` unique items across as many pages as it needs, with a lossless next cursor and a
  closed-set stop reason, as the pagination contract defines
  (`docs/designs/2026-09-03-pagination-and-result-bounding.md`, D1 to D3).
- R10. Before the first page the loop computes its plan (requests and objects by kind) and binds a budget; an
  embedder-supplied budget policy sees the plan and either refuses the run or returns the budget the loop binds to (D4).
- R11. Sending one page with `Call::send` and a `pagination_token` keeps working unchanged; the loop is a new way to
  send a list call beside it.
- R32. Each page request the loop sends carries, on the hook's views, the list's canonical request identity (every
  parameter except page size and cursor), its list class, its primary resource kind, the page's index in the run, and
  the cursor that addressed it.
- R33. An embedder can install a local list source that serves a list run with no request; such a run plans zero
  requests, skips the endpoint's floor, and carries the source's own fields into the run's list metadata.
- R34. The contract's exit codes 75 (partial data), 80 (bounded stop, nothing delivered), and 81 (pre-flight refusal)
  are xdk-rs constants, and a list run's metadata names the code its stop maps to.

**Needed from xurl-rs: command layer**

- R12. xurl-rs exposes `xr`'s API commands and its global flags as a supported library: a clap subcommand set and a
  global-flags struct an embedder flattens into its own clap tree, separate from `xr`'s meta commands (schema,
  completions, version, examples, skill, validate) and from raw mode.
- R13. The library runs one parsed API command with the parsed global flags, the embedder's writers, the token store
  path, the environment as data, and a client factory, and returns `xr`'s exit code.
- R14. A composed command prints exactly what `xr` prints for the same parsed input: data, envelopes, hints, and exit
  codes.
- R15. The library exposes `xr`'s schema documents and its envelope renderer, so an embedder's own commands can list
  `xr`'s schemas beside their own and print `xr`'s envelope.
- R16. The library surface carries a semver contract: it is documented, checked for breaking changes in CI, and its
  types allow additive change without a major release.
- R30. xdk-rs exposes its post-vocabulary normalizer, or a public typed decode, so a body bird stored or requested
  itself reads `post_count` where `xr`'s output does.
- R31. `xr schema --list --output json` prints one `ok` document, as every other command does, not one `{"message"}`
  per row.

**bird composition**

- R17. bird's command set is `xr`'s API commands, `auth` included, plus bird's own: `thread`, `watchlist`, `cache`,
  `doctor`, `schema`, `completions`, `skill`, `spend` (R22), and `rank` (R29). Every bird verb `xr` has retires with no
  alias, `login` among them.
- R18. Every command bird runs, composed or its own, goes through a client bird builds with its cache hook and budget
  policy installed, so cache reads, entity writes, the usage ledger, and the spend guard apply to both.
- R19. bird's global flags are `xr`'s global flags plus bird's `--refresh`, `--no-cache`, `--cache-only`, `--max-spend`,
  and `--examples`; where a bird flag and an `xr` flag differ in name or meaning, `xr`'s wins; each flag that reads an
  environment variable reads its `BIRD_*` name.
- R20. Under `--cache-only` a composed read answers from the store or fails with bird's cache-miss error before any
  request, and a composed write refuses.
- R21. bird has no raw HTTP verbs.
- R22. `bird usage` and `bird usage credits` are `xr`'s; bird's local estimate ledger and its estimate-against-actual
  view move to a bird verb, `bird spend`.

**Query cache (bird)**

- R25. For each list request identity, bird stores the ordered ids an online run observes, each id's observation time,
  and the API cursor where observation stopped: a run from the head replaces the stored order of a stable list and
  merges by id descending into a time-ordered one, a run from the stored end cursor appends, and a run from any other
  cursor updates entities only (contract D7.2).
- R26. Under `--cache-only` a list command replays the stored list for its request identity in stored order up to
  `limit` and sends nothing; its envelope carries a `cache` field with the observation-time range it served, and a
  stored list that runs out first stops with `cache_miss` and the stored end cursor (D7.4).
- R27. Each page's entity writes, query-cache update, and ledger entry commit in one transaction, so a page is either
  fully recorded or absent after a crash (D7.6).
- R28. An online list run always fetches; the query cache serves only `--cache-only` (D7.3).

**Ranking (bird)**

- R29. `bird rank <query> [--by likes|reposts|replies|quotes] [--min-likes <n>]`, bounded by the global `--limit`,
  sends exactly the requests `bird search <query>` sends for the same `--limit`, ranks the delivered posts by the chosen
  public metric after the last page (likes by default), and drops posts below `--min-likes`; under `--cache-only` it
  ranks the stored list for that query and sends nothing.

**Needed from xurl-rs: seam and documents**

- R23. The seam document's flag-parsing rule reads: flag parsing is presentation, implemented once in `xr`'s command
  layer, and a client passes an `xr` capability through by composing that command, never by re-implementing its flags.
- R24. The seam document and the pagination contract live in xurl-rs in the vocabulary of the xdk-rs and xurl-rs split;
  bird keeps only its conformance document, citing their versions.

### Key Flows

- F1. A composed read served from the cache
  - **Trigger:** `bird read 1234567890` a second time on the same UTC day.
  - **Steps:** bird parses with its composed tree and hands the `read` command to the xurl-rs library with bird's client
    factory; `xr`'s handler builds its call; the client asks bird's hook before sending; the hook finds the post fresh
    in the store and answers.
  - **Outcome:** stdout is byte-identical to the first run's; no request reaches X; the usage ledger records a cache hit
    at no cost.
- F2. A composed list refused on cost
  - **Trigger:** `bird search "rust" --limit 5000` with the default `--max-spend`.
  - **Steps:** `xr`'s search handler starts the list loop; the loop plans the run and asks bird's budget policy; the
    policy prices the plan above the spend limit.
  - **Outcome:** no request is sent; stderr carries bird's refusal naming the estimate and `--max-spend`; the exit code
    is the contract's pre-flight refusal code.
- F3. A retired verb
  - **Trigger:** a script runs `bird tweet hi`, `bird me`, or `bird get /2/users/me`.
  - **Outcome:** clap's unknown-command error at exit 2, the same as any unknown verb; no request.

### Acceptance Examples

- AE1. **Covers R4, R18.** Given a post cached today, when `bird read <id>` runs, then the hook answers, no request is
  sent, and stdout equals what `xr read <id>` prints for the same post.
- AE2. **Covers R1, R8.** Given xurl-rs with the hook and the library surface, when its existing CLI test suite and
  goldens run, then every one passes unchanged.
- AE3. **Covers R14, R19.** Given the same arguments, when `bird like <id> --output json` and
  `xr like <id> --output json` run against the same scripted responses, then their stdout and exit codes match.
- AE4. **Covers R10.** Given a budget policy that refuses, when a list call starts, then the policy's error reaches the
  caller and the client sends nothing.
- AE5. **Covers R20.** Given `--cache-only` and an empty store, when `bird user jack` runs, then it fails with bird's
  cache-miss error and sends nothing.
- AE6. **Covers R25, R26.** Given an online `bird search rust --limit 30` that stored 30 ids, when
  `bird search rust --limit 20 --cache-only` runs, then it prints the first 20 stored posts in stored order, sends
  nothing, and carries `meta.cache` with the observation-time range; with `--limit 40` it prints 30 and stops with
  `cache_miss` and the stored end cursor.
- AE7. **Covers R29.** Given the same query and `--limit`, when `bird rank` and `bird search` each run online against
  the same scripted answers, then they send the same requests, and `bird rank`'s `data` is `bird search`'s posts sorted
  by like count with those under `--min-likes` removed.

### Success Criteria

- `xr`'s behavior is unchanged apart from its list verbs paging (AE2), and the xdk-rs and xurl-rs releases that carry
  this work pass their semver checks as additive.
- bird's `--help` lists `xr`'s API verbs and bird's own, and no other; bird holds no request builder for an endpoint
  `xr` has a command for.
- A composed verb and its `xr` twin print the same output for the same input (AE3).
- Repeating a cached read on the same UTC day costs one request in total (AE1).
- A stored list replays under `--cache-only` at no cost (AE6), and `bird rank` costs what `bird search` costs for the
  same query and limit (AE7).

### Scope Boundaries

- In, bird: composition and the retirement of its overlapping verbs, the spend guard on composed lists, the query
  cache with `--cache-only` list replay, `bird spend`, and `bird rank`.
- Needed from upstream, owned by the xurl-rs team: the xdk-rs request hook, list loop, budget policy, page context,
  local list source, and exit codes; the xurl-rs command-layer surface; `xr`'s list verbs on the loop; the seam and
  contract documents in xurl-rs with R23's revision; the vocabulary normalizer export; the `xr schema --list` envelope.
- Out: `xr`'s raw mode and meta commands in bird; `xr validate` in bird; any change to `xr`'s verbs, flags, or
  envelopes.
- Deferred to follow-up work: checkpoint mode (D6), author hydration (D7.5), the `cost`, `checkpoint`, and `hydration`
  envelope fields, vendored prose evidence with checksums (D2.4), the `SEAM_VERSION` and `CONTRACT_VERSION` constants,
  `xr endpoints`, and the seam's ast-grep rules and CI workflow.

### Dependencies / Assumptions

- The xurl-rs release in flight ships first; bird then takes xdk-rs and xurl-rs from crates.io.
- The xdk-rs and xurl-rs releases that carry R3 to R16 and R32 to R34 ship before bird's 1.0.0 cut; R23, R24, R30,
  and R31 do not gate it. bird adapts to the names and shapes they ship with.
- Assumption: xdk-rs keeps one JSON send path (`Client::send_request_with`) that every shortcut and every page goes
  through, which is where the hook sits.
- Assumption: bird stays synchronous and drives the async xurl-rs library on its current-thread runtime, as it drives
  xdk-rs today.

### Outstanding Questions

#### Deferred to Planning

- (Affects R13; technical) Whether the library renders the run's `tracing` diagnostics itself or hands the renderer to
  the embedder.
- (Affects R4; technical) Whether an answer is restricted to GET by the client or by the hook's own policy.

### Sources / Research

- xurl-rs `origin/dev` at `f0f2bc6`: `crates/xdk/src/api/request/{mod,call,transport,builder}.rs` (one JSON send path,
  `Call<T>`, the token-refresh hook precedent `OnTokenRefreshed`), `crates/xdk/src/api/shortcuts.rs` (49 shortcuts, the
  `.paginated()` marker), `crates/xurl-cli/src/cli/{runner,mod}.rs` and `commands/mod.rs` (the runner, the
  `Cli`/`Commands` tree, `make_client` at every command).
- xurl-rs `docs/plans/2026-09-15-2343-refactor-adoption-grade-crate-plan.md` KTD18: a later paging terminator is a new
  method on `Call<T>`; `send()` does not change.
- bird `docs/designs/2026-09-03-pagination-and-result-bounding.md` and `docs/designs/2026-09-04-xurl-bird-seam.md`
  (drafts that name xurl-rs as their home and are not in xurl-rs), `docs/designs/2026-09-04-bird-client-conformance.md`.
- bird `src/xurl_client/mod.rs`, `src/db/client/` (the cache path the hook replaces), `src/cli/mod.rs` (the verb set).
- `docs/plans/2026-09-02-2314-feat-xurl-v3-post-vocabulary-cutover-plan.md` (the verb mapping composition supersedes).

<!-- ce-section: work-relationships -->
### How This Work Fits Together

This document covers backlog stack 3 and the list loop of stack 5 item 3. The rest is the current understanding, not a
committed roadmap.

- Stack 4 (output contract parity) can proceed independently of this work: bird's own commands print `xr`'s envelope
  shape now, and R15 later lets them print it through `xr`'s renderer.
- Stack 5 item 1 (wire vocabulary for bird's own commands and the store) can proceed independently.
- Stack 5 item 2 shrinks: composition delivers the verb renames; what remains is removing bird's raw HTTP verbs, which
  must land before composition because bird's raw `post` and `delete` collide with `xr`'s.
- Stack 6 (release) depends on this work: bird's 1.0.0 cut waits for the xdk-rs and xurl-rs releases that carry it.
- Stack 7 (bird-skill) depends on this work: the skill describes the composed verb set.
