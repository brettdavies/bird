---
title: Result bounding and pagination for xurl-rs and bird
status: draft
version: 1
canonical: xurl-rs/docs/designs/2026-09-03-pagination-and-result-bounding.md
reference_client: bird
evidence: vendor/x-api-docs/INDEX.md
---

# Result bounding and pagination for the X API tooling pair

This document is the contract for how `xurl-rs` (crate and `xr` CLI) and `bird` bound, page, price, and resume list
calls against the X API v2. It is a clean-sheet design derived from the vendored X API evidence and the agent-native P7
principle. Every external claim cites a vendored file by path under the evidence directory `vendor/x-api-docs/` of
`xurl-rs`; `vendor/x-api-docs/INDEX.md` maps each file to the decisions it grounds and holds the re-vendor procedure.
Where a copy of this document lives in a repository without `vendor/x-api-docs/`, the same files are at
`docs/references/x-api/`. This is the crate's behaviour contract; `bird` is its reference client, and every placement
below follows the
crate's scope policy in `docs/designs/2026-09-04-xurl-bird-seam.md`, which carries the worked matrix for this
contract.

## Problem statement

Two products share one maintainer and one API. `xurl-rs` (binary `xr`, crate `xurl-rs`) owns authentication, request
construction, endpoint knowledge, typed wire shapes, and error mapping. `bird` is the agent-facing consumer built on the
crate; it owns a local SQLite entity cache, cost accounting, and ergonomics for agents. The crate and `xr` are public
and are used by developers other than the maintainer, as a dependency and as a subprocess. Both products need one answer
to "how many items do I get, what does it cost, and how do I get the next ones", written down once where every consumer
can cite it.

The API facts that shape the answer:

- **Every list endpoint pages by an opaque cursor.** A response carries `meta.next_token` (and usually
  `meta.previous_token` and `meta.result_count`); the caller sends the token back as `pagination_token` until no token
  is returned (`vendor/x-api-docs/fundamentals-pagination.md`). Search additionally accepts the token as `next_token`
  (`vendor/x-api-openapi.json`,
  `/2/tweets/search/recent` parameters). The parameter description calls the token base32hex; its schema is a
  string with a minimum length (`vendor/x-api-openapi.json`, `pagination_token` parameters). Tokens may
  expire in general (`vendor/x-api-docs/fundamentals-pagination.md`, "Tokens may expire after some time"); recent-search
  tokens do not (`vendor/x-api-docs/posts-search-integrate-paginate.md`).
- **Page sizes have per-endpoint floors and ceilings.** `max_results` is bounded 10..100 on recent search, 5..100 on
  mentions, liked posts, and user posts, 1..100 on the home timeline, bookmarks, and DM events, and 1..1000 on followers
  and following; only recent search and DM events declare an API default (`vendor/x-api-openapi.json`, each endpoint's
  `max_results` schema). A caller who wants three mentions
  cannot ask for three.
- **Two list classes exist.** Followers, following, bookmarks, liked posts, and DM events are stable-order lists whose
  `meta` carries only cursors and a count. Search, mentions, user posts, and the home timeline are time-ordered lists
  whose `meta` also carries `newest_id` and `oldest_id`, and which accept `since_id`, `until_id`, `start_time`, and
  `end_time` (`vendor/x-api-openapi.json`, response `meta` schemas and per-path parameters). Recent search
  delivers posts newest-first within and across pages, and X documents a polling pattern built on `since_id` and the
  first page's `newest_id` (`vendor/x-api-docs/posts-search-integrate-paginate.md`). Timelines stop at a depth cap:
  3,200 user posts, or 800 with `exclude=replies`; 800 mentions; 3,200 posts or seven days on the home timeline. Beyond
  the cap the API returns success with no data (`vendor/x-api-docs/posts-timelines-integrate.md`, "Volume limits").
- **Pages can be short, empty, or repetitive.** A page may hold fewer items than `max_results` while a `next_token` is
  still present (`vendor/x-api-docs/fundamentals-pagination.md`, example loop). A timeline can answer `result_count: 0`
  with a `next_token` when non-public metrics are requested for posts older than 30 days
  (`vendor/x-api-docs/posts-timelines-integrate.md`, "Edge cases"). That items can repeat across pages of a list whose
  head moves between requests is an inference from the cursor model, not a documented statement.
- **Every object returned costs money.** Reads are billed per resource returned: $0.005 per post, $0.010 per user,
  $0.010 per DM event, $0.010 per follower or following entry, and $0.001 per resource on owned reads, which apply when
  the subject is the authenticated user and that user owns the developer app (own posts, mentions, likes, bookmarks,
  followers, following, blocks, mutes, and lists). Charges are deduplicated per resource within a 24-hour UTC window as
  a soft guarantee; only successful responses that return data are billed; requests are blocked at zero balance or at a
  console spending limit; pay-per-use is capped at 3 million post reads per month
  (`vendor/x-api-docs/getting-started-pricing.md`, `vendor/x-api-docs/fundamentals-post-cap.md`). Expanded objects
  arrive in `includes` as users, posts, media, polls, places, or topics (`vendor/x-api-openapi.json`,
  `Expansions` schema). The pricing page bills "per resource returned in the response" and lists a user price; the
  inference in this document, labelled as such, is that an expanded author is a user resource and is billed as one.
- **Attachments per post are bounded.** A post may include up to 4 photos, 1 animated GIF, or 1 video
  (`vendor/x-api-docs/media-introduction.md`); the create-post request carries a single `poll` object and a single
  `quote_tweet_id` and `reply` (`vendor/x-api-openapi.json`, `CreatePostsRequest`); a referenced post has
  exactly one of three reference types (`vendor/x-api-openapi.json`, `PostReferencedPosts`); a post can be
  edited up to 5 times (`vendor/x-api-docs/posts-timelines-integrate.md`, "Post edits").
- **Rate limits are per endpoint per window, and exhaustion is a 429.** Each response carries `x-rate-limit-limit`,
  `x-rate-limit-remaining`, and `x-rate-limit-reset`. A 429 can mean rate limit or usage cap; the documented 429 body is
  a legacy `errors[].code` shape without a problem `type`, while the problem-type catalogue lists `rate-limit-exceeded`
  and `usage-capped` (`vendor/x-api-docs/fundamentals-rate-limits.md`, "Handling rate limits";
  `vendor/x-api-docs/fundamentals-response-codes-and-errors.md`, "Error types"). A 200 can carry partial errors beside
  data.
- **Balance and consumption are queryable.** `GET /2/usage/credits` returns `total_balance`, `prepaid_balance`, and
  `free_balance` in USD, and `GET /2/usage/tweets` reports post consumption
  (`vendor/x-api-docs/usage-get-usage-credits.md`, `vendor/x-api-docs/usage-introduction.md`).
- **Agents need bounded output.** P7 requires a `--quiet` flag and a documented default clamp with a truncation signal
  on every list command, and recommends an exact-count `--limit`, `--verbose`, and `--timeout`
  (`vendor/x-api-docs/agentnative-p7-bounded-high-signal-responses.md`).
- **The spec's post vocabulary is `post.fields`.** The guide prose still says `tweet.fields`
  (`vendor/x-api-docs/fundamentals-fields.md`); the OpenAPI parameter on every list endpoint in this document is
  `post.fields` (`vendor/x-api-openapi.json`, `PostFieldsParameter`).

The account is pay-as-you-go and agents are the first user, so the design has to make the worst-case spend of a call
knowable before the first request, give every list call a lossless resumable position, give every failure a structured
shape, and never require a TTY.

## What makes this cool

Cost becomes an output. Asked for 230 posts with their authors, `bird` can say before it spends a cent "at most 230
posts and 230 users, at most $3.45, in at most 6 requests", refuse if that exceeds the caller's guard, and afterwards
say exactly how far it got, what it fetched, and where to resume. An agent can budget an X API call the way it budgets
tokens, and any consumer can read the rules on docs.rs for the crate version they build against.

## Constraints

- Pay-as-you-go: never request more objects than the caller asked for beyond what the endpoint floor forces; prefer free
  endpoints and the local cache.
- Agents first: JSON envelopes, non-interactive operation, structured errors, resumable position, no TTY.
- The two list classes are handled deliberately, not by one generic loop pretending they are the same.
- The per-endpoint floor means someone absorbs the gap when a caller asks for fewer items than the floor.
- Solo maintainer, next release of each repo is a major: nothing is preserved for compatibility. The crate and `xr` are
  public, so their surfaces carry semver weight from that major onward.
- One word, one meaning, across the crate, `xr`, and `bird`.

## Premises

1. **Agents want an item count, not a page count.** P7 frames the need as "request exactly the number of items they
   want"; an agent's constraint is context size, which is measured in items, not HTTP responses.
2. **Worst-case spend is computable from the endpoint bounds alone, and the loop must honour a budget.** Page sizes,
   floors, depth caps, and expansion fan-out are known before the first request; a ceiling only stays a ceiling if the
   loop refuses to exceed the budget it was given.
3. **The API's own tokens are the right resume handle.** Cursors are opaque; wrapping them in a tool-specific handle
   adds nothing an agent can reason about. A resume is lossless only if everything fetched was delivered, so nothing
   fetched is discarded.
4. **Endpoint knowledge belongs in exactly one place, the crate.** Bounds, cursor parameter names, list class, depth
   caps, expansion fan-out, error classification, and the primary resource kind of each endpoint are facts about the
   API, so the layer that owns the API owns them, and every consumer reads them through the crate rather than keeping a
   second table.
5. **Prices are not API facts, so they belong to the cost layer.** The pricing page changes independently of the spec;
   `bird` owns the price table and its provenance.
6. **Refusal beats prompting.** Without a TTY the only honest confirmation is a re-run with an explicit override, so a
   spend guard refuses with a structured error that names the override rather than asking. The override itself is
   bounded by a limit only a person sets.

### Premise challenge

- *Is item-count the right primary knob, or should spend be?* A spend-first knob ("fetch as much as $0.20 buys") reads
  well for pay-as-you-go but fails P7's exact-count requirement and makes the number of items a function of prices,
  which change. Spend stays as a guard and as the loop's budget; items stay as the knob.
- *Could the caller page and the crate stay stateless?* It could, and it is the simplest crate. It also forces both CLIs
  and every future consumer to know floors, ceilings, depth caps, fan-out, and the cursor parameter names, which is the
  duplication premise 4 exists to prevent.
- *Should floor-forced items be discarded to keep `limit` exact?* Discarding them makes the cursor lossy (the next page
  starts after items the caller never saw) and wastes objects that are already billed. Delivering them separately keeps
  `data` exact and the cursor lossless.
- *Should the loop bind to the plan it was priced from?* Binding to the plan's request count makes every short page an
  early stop, so a caller with budget to spare gets fewer items than asked. Binding to an explicit budget that the
  consumer derives from its own guard keeps the promise agents care about ("never more than `--max-spend`") while
  letting short pages fill up to `limit`.
- *Does a page cache earn its place?* X deduplicates billing per resource within a UTC day, so re-fetching a page you
  saw this morning is free; an online page cache saves rate limit and latency, not money, and needs freshness rules the
  evidence does not supply. What agents need is a zero-request re-read of what a query has already returned. That is a
  per-query list of observed ids, not a cache of pages.
- *What happens if we do nothing?* Each CLI keeps an ad hoc loop, cost is discovered on the invoice, and the two repos
  drift. The pain is concrete: a single `followers --limit 1000` is a $10 request.
- *Does anything existing solve this?* The SDK pattern on the timelines page (`for page in client.posts.get_user_posts`)
  shows the shape the crate should offer, a page iterator, but says nothing about cost, floors, or resume.

## Approaches considered

### Approach A: caller pages (minimal)

The crate exposes one request per call with typed `meta`; `xr` makes one request per invocation; `bird` runs its own
loop and its own bounds table.

- Effort: S. Risk: Medium.
- Pros: smallest crate surface; `xr` stays curl-like; no page loop to test in the crate.
- Cons: bounds, floors, depth caps, fan-out, and cursor names live twice; cost arithmetic in `bird` depends on a table
  the crate does not vouch for; every other crate consumer rewrites the loop and the floor handling.
- Reuses: the crate's request builder and typed responses.

### Approach B: the crate owns the registry, the plan, and the loop (chosen)

The crate holds an endpoint registry, computes a pure `Plan` from a `ListRequest`, and runs a page loop under an
explicit budget, fetching through a pluggable page source. `xr` and `bird` are thin: `xr` prints what the crate returns;
`bird` prices the plan, applies the spend guard, derives the budget from it, supplies its page sources, and adds
hydration and the checkpoint mode.

- Effort: M. Risk: Low.
- Pros: one home for endpoint facts with drift tests against the evidence; identical semantics in both CLIs and for
  every third-party consumer; the plan is testable without network; every consumer gets floors, dedup, a binding budget,
  and structured stops for free.
- Cons: the crate grows a stateful loop and a page-source abstraction; a registry mistake affects every consumer at once
  (mitigated by the drift tests).
- Reuses: the vendored spec already in `xurl-rs/vendor/` as the registry's test oracle.

### Approach C: spend-first fetching (lateral)

The primary knob is a USD budget; the tool fetches pages until the next page could exceed it.

- Effort: M. Risk: High.
- Pros: matches how the account is billed; makes the cost ceiling the contract itself.
- Cons: violates P7's exact-count expectation; two agents with the same budget get different context sizes on different
  days.
- Reuses: the plan arithmetic from B.

**Chosen: B.** It is the only approach where "defined once" holds for bounds, list class, fan-out, and cursor names, and
it makes the cost ceiling a pure function the review can check by hand. C's budget idea survives as the spend guard and
the loop budget in B.

## The contract

The rules below are normative. "MUST", "MUST NOT", "SHOULD", and "MAY" carry their usual meaning. Decision IDs (D1 to
D9) tie each rule to the decision log and to the `grounds` column of `vendor/x-api-docs/INDEX.md`.

### Vocabulary (D8)

Each word has exactly one meaning everywhere: crate API, `xr`, `bird`, `--help`, and JSON.

| Word              | Meaning                                                                                                  |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| `limit`           | The maximum number of unique items the caller receives in `data` from this invocation, across all pages. |
| `effective limit` | `min(limit, depth cap)` for the request; every plan figure derives from it.                              |
| `returned`        | The number of items in `data`.                                                                           |
| `page`            | One request to the page source and its response.                                                         |
| `page size`       | The `max_results` sent on one page.                                                                      |
| `page source`     | What the loop asks for a page: the crate's HTTP client, or a wrapper a consumer supplies.                |
| `cursor`          | The API's own opaque pagination token, unchanged.                                                        |
| `head page`       | The page fetched without a cursor for one request identity.                                              |
| `floor`           | The endpoint's minimum `max_results`.                                                                    |
| `ceiling`         | The endpoint's maximum `max_results`.                                                                    |
| `depth cap`       | The most items an endpoint ever serves for one request shape, regardless of paging.                      |
| `overflow`        | Items a floor forced the tool to fetch beyond `limit`; delivered in their own array, not counted.        |
| `fan-out`         | The most included objects one primary object can add for one expansion.                                  |
| `plan`            | The pre-flight worst case for a request: requests, objects by kind, overflow, and (in `bird`) USD.       |
| `budget`          | The ceiling the loop binds to: requests, objects by kind, and a deadline. Derived from the plan by       |
|                   | default; `bird` derives it from the spend guard.                                                         |
| `plan cap`        | The stop that fires when the next page would exceed the budget's request or object ceiling.              |
| `stop`            | Why a page loop ended, as a closed set of reasons in three categories: natural, bounded, failure.        |
| `complete`        | The stop was natural (`limit_reached`, `exhausted`, `depth_cap`).                                        |
| `truncated`       | A resume position is present: more items may exist beyond what was delivered.                            |
| `checkpoint`      | `bird`'s stored record for one exact query: the `newest_id` last drained to, plus any pending drain.     |
| `drain`           | Fetching the rest of a poll's backlog from a stored cursor under the same `since_id`.                    |
| `spend`           | Money, in USD.                                                                                           |
| `cost`            | The `bird` envelope field that reports spend figures for one invocation.                                 |
| `hard cap`        | The per-invocation spend limit set in `bird` configuration, which no flag can raise.                     |
| `dry run`         | Compute and print the plan; make no request.                                                             |
| `cache only`      | Serve from the local cache; make no request.                                                             |
| `expand`          | Ask the API to include related objects, by the API's expansion names; priced by fan-out.                 |
| `hydrate`         | Fill related objects from the local cache first, then one batched lookup for the misses.                 |
| `wait`            | Sleep until the rate-limit window resets and continue the same run, within the deadline.                 |
| `timeout`         | The deadline for the whole invocation.                                                                   |
| `quiet`           | Suppress everything but requested data and errors.                                                       |
| `verbose`         | Add diagnostic detail on stderr.                                                                         |

Flag and field names are composed from vocabulary words (`--limit`, `--cursor`, `--page-size`, `--max-spend`,
`--dry-run`, `--cache-only`, `--expand`, `--hydrate`, `--wait-for-rate-limit`, `--timeout`, `--quiet`, `--verbose`,
`--since-checkpoint`, `next_cursor`, `overflow_count`) or are the API's own parameter names verbatim (`--since-id`,
`--until-id`, `--start-time`, `--end-time`, `--exclude`, `--sort-order`, and every `--expand` value). No flag or field
reuses a vocabulary word with a second meaning.

#### Exit codes

Exit codes are part of the contract and identical in `xr` and `bird`.

| Category                                 | Code | When                                                                                                         |
| ---------------------------------------- | ---- | ------------------------------------------------------------------------------------------------------------ |
| natural stop                             | 0    | `limit_reached`, `exhausted`, `depth_cap`                                                                    |
| partial data                             | 75   | a bounded or failure stop with at least one item delivered                                                   |
| bounded stop, nothing delivered          | 80   | `plan_cap`, `empty_pages`, or `cache_miss` with `returned` 0                                                 |
| pre-flight refusal                       | 81   | spend guard, hard cap, unbounded expansion, or `--expand author_id` on a post list (`bird`); no request made |
| failure stop, nothing delivered          | tool | the failure's own code in that tool (auth, config, request)                                                  |
| argument conflict or invalid pinned size | tool | the tool's usage-error code; no request was made                                                             |

75 is the conventional "temporary failure, retry later" code, which matches a stop that hands back a cursor. Each tool's
existing codes for auth, configuration, and usage errors stay as they are; 75, 80, and 81 are reserved by this contract
and MUST NOT collide with them.

### D1: what a limit means

1. `limit` is a count of unique items delivered in `data` for the whole invocation. It has the same meaning in the
   crate's `ListRequest`, in `xr`, and in `bird`.
2. `limit` defaults to 10 on every list command (`vendor/x-api-docs/agentnative-p7-bounded-high-signal-responses.md`
   makes the default clamp a MUST). Ten is at or above every floor in the registry, so the default never overflows.
3. There is no fixed item maximum. The bounds above the default are the spend guard (D4) and the depth cap (D5).
4. Items are deduplicated by `id` within one invocation, first occurrence wins, and only unique items count toward
   `limit`. The envelope reports `duplicates_dropped`.
5. Items a floor forced beyond `limit` are delivered in `overflow`, deduplicated against `data`, not counted toward
   `limit`, and counted in `meta.overflow_count`. Nothing fetched is discarded. Overflow can only arise on the page that
   reaches the limit, so at most one page of a run carries it.
6. `truncated` is true exactly when `next_cursor` is present. `complete` is true exactly when the stop is natural. The
   two are independent: a run can be complete and truncated (limit reached, more exist), incomplete and truncated
   (failure, resumable), or complete and not truncated (exhausted, or depth cap reached).

### D2: who pages, and where endpoint knowledge lives

1. The crate owns the page loop. Neither CLI implements one. The loop fetches through a **page source**: the crate's
   HTTP client by default, or a wrapper the consumer supplies that may answer a page without a request or decline to
   answer. A page answered without a request adds nothing to `requests_made`, `objects_fetched`, or cost.
2. The crate owns an **endpoint registry**. For every list endpoint it records: path; list class (`stable` or
   `time_ordered`); floor, ceiling, and the API default where the spec declares one (informational, since the crate
   always sends `max_results`); the cursor parameter the crate sends (`pagination_token` on every endpoint; search also
   accepts `next_token` but the crate does not send it); whether time filters apply; the depth cap as a function of the
   request shape (user posts: 3,200, or 800 with `exclude=replies`; mentions: 800; home timeline: 3,200, with the
   seven-day bound recorded as unplannable); the primary resource kind (`post`, `user`, `dm_event`, `follow`); and, per
   expansion the endpoint accepts, the included kind and its fan-out. Each row's bounds and parameters are stated in the
   vendored OpenAPI document, which the drift test in D2.6 reads. The registry also records, for the lookup
   endpoints a list consumer needs, the ids ceiling per request (`/2/users` `ids` and `/2/users/by` `usernames`: 100
   each), and the crate's lookup
   operations chunk by it (`vendor/x-api-openapi.json`, `/2/users` `ids` and `/2/users/by` `usernames`).
3. **Fan-out is an upper bound, and every bound names its evidence.**

| Expansion                                                       | Primary  | Included kind     | Fan-out   | Evidence                                                                          |
| --------------------------------------------------------------- | -------- | ----------------- | --------- | --------------------------------------------------------------------------------- |
| `author_id`, `in_reply_to_user_id`                              | post     | user              | 1         | single id fields (`x-api-openapi.json`, `Post`)                                   |
| `username`                                                      | post     | user              | 1         | one author per post (`x-api-openapi.json`, `PostExpansionsParameter`)             |
| `referenced_posts`                                              | post     | post              | 3         | one reference per type, three types (`x-api-openapi.json`, `PostReferencedPosts`) |
| `edit_history_post_ids`                                         | post     | post              | 6         | up to 5 edits plus the original (`posts-timelines-integrate.md`, "Post edits")    |
| `attachments.media_keys`                                        | post     | media             | 4         | up to 4 photos, 1 GIF, or 1 video (`media-introduction.md`)                       |
| `attachments.media_source_tweet`                                | post     | post              | 4         | one source post per attached media (`x-api-openapi.json`, `PostAttachments`)      |
| `attachments.poll_ids`                                          | post     | poll              | 1         | single `poll` object on create (`x-api-openapi.json`, `CreatePostsRequest`)       |
| `geo.place_id`                                                  | post     | place             | 1         | single id field (`x-api-openapi.json`, `Post`)                                    |
| `entities.mentions.username`                                    | post     | user              | unbounded | no documented maximum                                                             |
| `article.cover_media`, `article.media_entities`                 | post     | media             | unbounded | no documented maximum                                                             |
| `sender_id`                                                     | dm_event | user              | 1         | single id field (`x-api-openapi.json`, `DmEvent`)                                 |
| `participant_ids`, `referenced_posts`, `attachments.media_keys` | dm_event | user, post, media | unbounded | no documented maximum                                                             |
| `affiliation`, `most_recent_post_id`, `pinned_post_id`          | user     | user, post, post  | 1         | single id fields (`x-api-openapi.json`, `User`)                                   |

   The crate accepts every expansion the spec lists. For an unbounded expansion the plan sets `ceiling_is_bound` to
   false and reports `objects_max[k]` as unbounded for that kind; the budget still binds requests and primary
   objects, so `xr` proceeds and its `--dry-run` names the unbounded kind; `bird` refuses before any request
   because it cannot price it.
4. **Prose-sourced facts carry their evidence checksum.** Depth caps and every fan-out that rests on a prose page record
   the vendored file's `content_sha256`; a crate test fails when the vendored file's body no longer matches, so a
   re-vendor that changes a page forces a registry review before release.
5. The registry is a public crate API with semver weight. `xr endpoints [--json]` prints it for people and agents;
   `bird` consumes it by import.
6. A crate test re-derives floors, ceilings, defaults, cursor parameter names and descriptions, time-filter parameters,
   expansion enums, and `meta` shapes from the vendored OpenAPI document and fails when the registry disagrees, so a
   spec refresh surfaces drift before release.
7. **Page size at runtime.** With `remaining = effective limit - unique items so far` and `object budget left =
   budget.objects[primary] - objects requested so far`: `page size = min(ceiling, max(floor, remaining), object budget
   left)`. Before each page, if `requests_made == budget.requests` or `object budget left < floor`, the loop stops with
   `plan_cap` instead of fetching. When the caller pins `page size` (allowed in the crate and in `xr`), the pinned value
   is sent on every page and only the request gate applies; a pinned value outside `[floor, ceiling]` is an error before
   any request, and a pinned value is never silently clamped. `bird` derives `page size` and exposes no flag.
8. **Error classification is crate-wide.** The crate maps every response, list or not, and consumers never re-classify.
   HTTP status decides first. A 429 is `rate_limited` when the problem `type` is `rate-limit-exceeded` or
   `x-rate-limit-remaining` is 0; a 429 with problem `type` `usage-capped` is `request_failed` with that problem; any
   other 429 is `request_failed` with an `ambiguous_429` problem carrying the raw body and the observed headers. Other
   4xx and 5xx are `request_failed`. The problem `type`, `title`, `detail`, and status are carried when present; the
   legacy `errors[].code` body is carried as-is. The list loop reuses this mapping to choose its stop.

### D3: position, resume, and what the caller sees

1. Position is the API's own cursor, exposed as `next_cursor` (and `previous_cursor` when the API returns one) and
   accepted as `--cursor`. No wrapping handle, no offset.
2. `next_cursor` is lossless: after any stop it addresses the first page not yet delivered. After `limit_reached`,
   `exhausted`, or a bounded stop it is the last delivered page's `next_token` (absent when the API returned none).
   After a failed page it is the cursor that addressed the failed page (absent if the head page failed, in which case
   the run is resumed by repeating it). After `depth_cap` it is absent, because pages beyond the cap hold nothing.
3. Time-ordered lists additionally report `newest_id` and `oldest_id`, the maximum and minimum id over everything
   delivered (`data` and `overflow`). Under `sort_order=recency` this equals the first page's `newest_id`, which is the
   value the next poll needs (`vendor/x-api-docs/posts-search-integrate-paginate.md`); under `relevancy` the maximum
   over delivered items is the only correct definition. A run that delivers nothing carries neither field.
4. Every invocation ends with a `stop` object whose `reason` is one of a closed set in three categories: natural
   (`limit_reached`, `exhausted`, `depth_cap`), bounded (`plan_cap`, `empty_pages`, `cache_miss`), and failure
   (`rate_limited`, `timeout`, `request_failed`). `retry_after_seconds` (from `x-rate-limit-reset`) accompanies
   `rate_limited`; the mapped problem (D2.8) accompanies `request_failed`. `cache_miss` is raised by a page source that
   declines to answer; the crate's HTTP page source never raises it. The set is closed for this contract version and
   marked non-exhaustive in the crate so a future addition is not a breaking change for consumers.
5. On any stop, the items fetched so far are delivered, with `returned`, `requests_made`, `objects_fetched` by kind
   (objects returned, counted every time they are returned, so the figure stays an upper bound), `overflow_count`,
   `duplicates_dropped`, the last observed `x-rate-limit-*` triple (with the reset instant rendered as ISO 8601
   `reset_at`), and `next_cursor` per D3.2.
6. Exit codes follow the table in the vocabulary section.
7. Partial errors inside a 200 (`errors` beside `data`, `vendor/x-api-docs/fundamentals-response-codes-and-errors.md`)
   do not stop the loop; they are collected into the envelope's `errors` array.
8. `--timeout` is the deadline for the whole invocation; the loop stops with `timeout` when the remaining time is below
   the client's per-request timeout, and after a page that overran. The per-request HTTP timeout is a crate client
   setting, not a list flag.
9. **Waiting is opt-in.** With `--wait-for-rate-limit`, a `rate_limited` stop whose `reset_at` falls before the deadline
   becomes a sleep until `reset_at` followed by the same run continuing; the envelope reports `waits: [{reset_at,
   seconds}]`. Without the flag, or when `reset_at` is past the deadline, the loop stops with `rate_limited` as above.
   The wait lives in the crate loop, so `xr` and `bird` behave identically.

### D4: cost, knowable before the first request and binding on the loop

1. The crate's `plan(&ListRequest) -> Plan` is pure: no I/O, no clock. It returns `effective_limit`, `depth_capped`,
   `requests_max`, `objects_max` by resource kind, `overflow_max`, `ceiling_is_bound` (false only when an unbounded
   expansion is present), and the default `budget` derived from those figures.
2. **Derived page sizes.** With `L = effective limit`, `C = ceiling`, `F = floor`: if `L < F`, one page of `F`;
   otherwise `L div C` pages of `C` plus, when `L mod C > 0`, one page of `max(F, L mod C)`. `requests_max` is the page
   count, `objects_max[primary]` is the sum of page sizes, and `overflow_max = objects_max[primary] - L`. **Pinned page
   size `P`:** `requests_max = ceil(L / P)`, `objects_max[primary] = requests_max × P`, `overflow_max =
   objects_max[primary] - L`.
3. **Expansions.** For each requested expansion with kind `k` and fan-out `f`: `objects_max[k] += objects_max[primary] ×
   f`. **Hydration** (`bird` only, `--hydrate authors`, valid only when the primary kind is `post`): `objects_max[user]
   += objects_max[primary]` and `hydration.requests_max = requests_max`, because `bird` hydrates
   each page as it arrives and every post list's ceiling is at or below the user lookup's ids ceiling, so a page
   never needs more than one lookup; the crate's `plan_lookup(n)` gives `ceil(n / ids ceiling)` requests for `n`
   ids (D2.2) and is what a single batched lookup uses. The estimate's request total is the sum. `bird` reports the
   lookups under
   `meta.hydration` (`requests`, `looked_up`, `from_cache`), so `requests_made` and `objects_fetched` keep the
   crate's values. `bird` refuses `--expand author_id` on
   post lists with a message naming `--hydrate`, because a cached author costs nothing and the API expansion always
   bills.
4. **The budget is binding.** `paginate()` takes a budget and never issues a request that would take `requests_made`
   past `budget.requests` or objects requested past `budget.objects[primary]` (D2.7). `xr` and every consumer without a
   spend model pass the plan's default budget, so for them the estimate is the ceiling whenever `ceiling_is_bound`; an
   unbounded kind has no object ceiling in
   the budget. `bird` scales the plan: with `s
   = max_spend / estimate_max_usd` (at least 1 whenever the call is not refused), `budget.objects[k] =
   floor(objects_max[k] × s)` for every kind and `budget.requests = ceil(budget.objects[primary] / floor)`; hydration is
   bounded separately by
   `plan_lookup(budget.objects[primary])`, so short pages and duplicates fill toward `limit` while spend never exceeds
   `max_spend`. A run
   that exhausts its budget stops with `plan_cap`, `complete: false`, and a lossless `next_cursor`.
5. `bird` owns a **price table** keyed by resource kind with an owned-read tier, sourced from
   `vendor/x-api-docs/getting-started-pricing.md` and carrying that file's `content_sha256` so a re-vendor that changes
   the page fails a test until the table is reviewed. Resource kinds with no listed price contribute $0 and are named in
   the estimate as unpriced.
6. `bird` computes `estimate_max_usd = Σ objects_max[kind] × price[kind, owned]`. Owned pricing applies only when the
   `bird` configuration asserts that the authenticated user owns the developer app (default: not asserted), the endpoint
   is on the owned-read list, and the subject is the authenticated user. The estimate is a ceiling whenever
   `ceiling_is_bound`: the 24-hour deduplication, short pages, and duplicates only lower the bill.
7. **Two-tier spend guard.** `bird` configuration holds `spend.hard_cap_usd` (default $5.00), which no flag can raise,
   and `spend.max_spend_usd` (default $1.00), which `--max-spend` overrides up to the hard cap. `bird` refuses before
   the first request when `--max-spend` exceeds the hard cap, when `estimate_max_usd` exceeds the effective `max_spend`,
   or when `ceiling_is_bound` is false. The refusal is a structured error naming the estimate, the tier that fired, and
   the exact override (a flag for `max_spend`, a configuration edit for the hard cap); it never prompts.
8. After the run, `bird` reports `cost.fetched_max_usd`, the price table applied to `objects_fetched` plus
   `price[user] × hydration.looked_up`, and `cost.budget_usd`, the ceiling the run bound to. `fetched_max_usd` is the
   most the run can have cost; deduplication
   can only make the invoice lower.
9. `--dry-run` prints the plan (and in `bird` the estimate and the budget) and makes no request. It is the confirmation
   step for agents.
10. `bird` does not call `/2/usage/credits` or `/2/usage/tweets` inside a list call. Balance and consumption are crate
operations surfaced as separate commands in `xr` and `bird`
    (`vendor/x-api-docs/usage-introduction.md`); an exhausted balance surfaces as `request_failed`
    with the `usage-capped` or `ambiguous_429` problem (D2.8).

### D5: floors, ceilings, and caps end to end

| Situation                                   | Behaviour                                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `limit` below the floor                     | One page of `floor`; `limit` items in `data`, the rest in `overflow`; never refused.        |
| Last page remainder below the floor         | Last page of `floor`; remainder in `data`, the rest in `overflow`.                          |
| `limit` above the ceiling                   | Pages of `ceiling`, then a final page per D4.2.                                             |
| `limit` above the depth cap                 | `effective limit = depth cap`; plan reports `depth_capped: true`; every figure derives from |
|                                             | it. The home timeline's seven-day bound is not counted; the loop simply ends early.         |
| Run ends while `depth_capped`               | Effective limit reached, absent token, or the empty-page guard: all stop with `depth_cap`,  |
|                                             | with no `next_cursor`.                                                                      |
| `--max-spend` above the hard cap            | Pre-flight refusal naming the hard cap (D4.7), no request made.                             |
| Estimate above the effective `max_spend`    | Pre-flight refusal naming the estimate and the override (D4.7), no request made.            |
| Unbounded expansion requested in `bird`     | Pre-flight refusal (D4.7), no request made.                                                 |
| `--expand author_id` on a post list in bird | Pre-flight refusal (81) naming `--hydrate authors`, no request made.                        |
| `--hydrate authors` on a non-post list      | Error before any request.                                                                   |
| Explicit `page size` outside the bounds     | Error before any request; the crate never silently clamps a value the caller pinned.        |
| Next page would exceed the budget           | Stop with `plan_cap` and a lossless `next_cursor` (D4.4).                                   |
| `x-rate-limit-remaining` reaches 0 mid-run  | Stop before the next page with `rate_limited` and `retry_after_seconds`, or wait (D3.9); do |
|                                             | not spend a request on a certain 429.                                                       |
| Two consecutive empty pages with a cursor   | Stop with `empty_pages` and the cursor, so the documented empty-page edge case cannot loop. |

### D6: incremental polling is a first-class mode

1. Time-ordered lists accept `--since-id`, `--until-id`, `--start-time`, and `--end-time`; the crate passes them on
   every page of the run, unchanged, exactly as the polling guide requires
   (`vendor/x-api-docs/posts-search-integrate-paginate.md`).
2. `bird` adds `--since-checkpoint` for time-ordered lists. Only runs with this flag read or write checkpoints. The
   checkpoint key is the query's identity: endpoint, subject, query string, `exclude`, and `sort_order`. It excludes
   `limit`, `page size`, `cursor`, expansions, fields, hydration, and every time or id filter.
3. The checkpoint record holds `newest_id` (the id every reachable newer item has been delivered up to), and, while a
   backlog is being drained, `drain_cursor` and `drain_since_id` (the cursor the last bounded run returned and the
   `since_id` it was minted under).
4. `--since-checkpoint` conflicts with `--cursor`, `--since-id`, `--until-id`, `--start-time`, and `--end-time`; any
   conflict is an error before any request. The caller holds no state: repeating the same command until
   `checkpoint.draining` is false drains and then polls.
5. With no stored record, the run sends no `since_id`, returns the newest `limit` items, and on any natural stop that
   delivered at least one item creates the record with `newest_id`. A first run that delivers nothing creates nothing.
6. With a stored record and no pending drain, the run sends `since_id = newest_id`. With a pending drain, the run sends
   `since_id = drain_since_id` and `pagination_token = drain_cursor` instead, and continues the backlog.
7. The record advances (`newest_id` set to the run's `newest_id`, drain fields cleared) only when the stop is
   `exhausted` or `depth_cap`, because only then has every reachable newer item been delivered. Any other stop that
   returned a `next_cursor`, including `limit_reached` on a backlog larger than `limit`, stores it as
   `drain_cursor` with the `since_id` the run used, and leaves `newest_id` unchanged. A run that delivers nothing
   changes nothing.
8. The envelope's `checkpoint` object is present only on `--since-checkpoint` runs and reports `newest_id` (the stored
   value after the run), `created` (true when this run created the record), `advanced` (true when this run moved
   `newest_id`), and `draining` (true when a drain cursor is pending after the run).
9. `sort_order=relevancy` changes nothing above: `since_id` still bounds the set and `newest_id` is the maximum over
   delivered items (D3.3).

### D7: how the bird cache takes part

1. **Entity cache.** Entities (posts, users, DM events) are cached by `id` from every page, including `overflow` and
   `includes`, with the page's observation time. `id` is the entity identity the data dictionary defines for each object
   (`vendor/x-api-docs/fundamentals-data-dictionary.md`). This is the only cache layer used for lookups by id.
2. **Query cache.** For every list request identity (endpoint, subject, query string, every filter, `exclude`,
   `sort_order`, expansions, fields; never `limit`, `page size`, or `cursor`) `bird` stores the ordered ids observed,
   the observation time of each, and the API cursor at which observation stopped. A run that starts at the head replaces
   the stored order for a stable list and merges by id descending for a time-ordered list; a run that starts from the
   stored end cursor appends; a run that starts from any other cursor updates entities only.
3. **No online substitution.** An online run always fetches; X's 24-hour billing deduplication already makes a same-day
   re-fetch free (`vendor/x-api-docs/getting-started-pricing.md`, "Deduplication"), so the only thing a served-from-
   cache page would save is rate limit, at the cost of freshness rules the evidence does not supply.
4. **`--cache-only`** supplies a page source that never fetches: it serves the stored ids for the request identity in
   stored order, up to `limit`, with entities from the entity cache, and reports a `cache` envelope field with
   `observed_from` and `observed_to`, the observation-time range of what it served. When the stored list runs out before
   `limit`, the run stops with `cache_miss`; `next_cursor` is the stored
   end cursor when one exists, so the caller can continue online from where the cache ends. The plan reports zero
   requests and $0.
5. **Hydration.** `--hydrate authors` fills each post's author from the entity cache, then hands the misses to the
   crate's user lookup, which chunks them by the registry's ids ceiling; the users land in `includes.users` in the
   API's own shape. `bird` never sends `expansions=author_id` on a
   list request. Other expansions ride the list
   request as API expansions and are priced by fan-out (D4.3).
6. **Atomic per-page writes.** For each page, the entity upserts, the query-cache update, and any checkpoint change
   commit in one transaction, so a page is either fully recorded or absent after a crash; hydration cache lookups are
   one batched query per page.

### D9: one contract, cited by every consumer

1. The contract is a `xurl-rs` document at `docs/designs/2026-09-03-pagination-and-result-bounding.md`. No client
   holds a copy; a client cites the `version` it conforms to.
2. The crate includes the text in its rustdoc (`#![doc = include_str!(...)]`), so docs.rs shows the contract for
   every published version, and exposes `CONTRACT_VERSION`, a constant set when this document's `status` is
   `accepted` and equal to its frontmatter `version`; a crate test parses the included frontmatter and asserts the
   match. `xr --help` prints a link to the document at the tag of the running binary's version. `bird` asserts that
   the version its client-conformance document cites equals the `CONTRACT_VERSION` of the crate it builds against,
   offline, so it cannot document a version it does not implement. A change to the rules bumps `version` and is a
   crate release; a wording fix ships with the next one. No document ships inside a binary.
3. Endpoint metadata is shared by import, not by copy: a client reads the crate registry and keeps no table of its
   own.
4. The vendored prose evidence is `xurl-rs/vendor/x-api-docs/`, with `INDEX.md` and its re-vendor procedure,
   beside the OpenAPI document that `xurl-rs` already tracks at `vendor/x-api-openapi.json` with
   `vendor/spec-metadata.json`. Endpoint reference pages are not vendored: each one embeds the OpenAPI document
   almost in full, so the spec is the single source for every bound and parameter. A client cites those paths at the
   crate
   version it pins and keeps no copy of its own.

## Request flow

```text
caller (agent or person)
  |  bird <list> --limit 230 --hydrate authors --max-spend 5 [--since-checkpoint] [--dry-run] [--cache-only]
  v
+------------------------------- bird ---------------------------------------+
| 1. resolve subject and auth; with --since-checkpoint, load the record      |
| 2. ListRequest -> crate::plan()  (pure)                                D4  |  Plan{requests_max, objects_max, overflow_max, budget}
| 3. price(plan) with the owned-read tier, add hydration figures         D4  |  estimate_max_usd
| 4. --max-spend > hard cap, estimate > max_spend, or unbounded expansion    |
|    -> refusal, no request;  --dry-run -> print plan, estimate, budget      |
| 5. budget = plan scaled by max_spend / estimate (D4.4)                     |
| 6. crate::paginate(request, page source, budget)                           |
|    page source = HTTP (online) or query cache (--cache-only)               |
| 7. per page, one transaction: upsert entities, update query cache,         |
|    hydrate authors (D7.5), update checkpoint (D6.7)                        |
| 8. envelope: data, overflow, includes, errors, meta{stop, cursors, ids,    |
|    counts, rate_limit, waits, cost, checkpoint, cache, hydration}          |
+----------------------------------------------------------------------------+
          |                                   ^
          | crate::paginate(request, page source, budget)   pages / stop
          v                                   |
+------------------------------ xurl-rs -------------------------------------+
| registry lookup (D2) -> plan (D4.2, D4.3) -> loop below under the budget   |
| page size rule (D2.7) -> request builder -> page source -> classify (D2.8) |
| -> typed pages, dedup, data/overflow split, stop                           |
+----------------------------------------------------------------------------+
          |  page source (HTTP): GET /2/... ?max_results=&pagination_token=&since_id=...
          v
       api.x.com  --> data[], includes{}, meta{next_token, newest_id...}, x-rate-limit-*
```

`xr` runs steps 2, 6 (with the crate's HTTP page source and the plan's default budget), and 8 without `cost`,
`checkpoint`, `cache`, and `hydration`; `--dry-run` in `xr` prints the plan in requests and objects. Steps 1, 3, 4, 5,
and 7 are `bird` only.

## Page-loop state machine

```text
                      +-----------+
                      |   PLAN    |  pure: effective_limit, requests_max, objects_max, overflow_max, default budget
                      +-----+-----+
                            |
                            v
        +----------> GATE: requests_made == budget.requests, or object budget left < floor ?
        |                   |  yes -> STOP plan_cap
        |                   |  no
        |                   v
        |            ASK page source for page (max_results = page size per D2.7, cursor, filters)
        |                   |
        |     +-------------+--------------+---------------------+-------------------+------------------+
        |     |             |              |                     |                   |                  |
        |     v             v              v                     v                   v                  v
        | 200 with data  200 no data   429 (classify D2.8)   4xx/5xx           deadline hit      source declines
        |     |             |              |                     |                   |                  |
        |     v             |    rate_limited ?                  v                   v                  v
        | dedup by id       |    no  -> STOP request_failed   STOP request_failed  STOP timeout    STOP cache_miss
        | split data /      |    yes -> wait flag and reset_at < deadline ?
        | overflow, yield   |             yes -> SLEEP until reset_at, record wait, re-enter GATE
        |     |             |             no  -> STOP rate_limited (retry_after from reset)
        |     |             v
        |     |      empty_pages == 2 ? yes -> STOP empty_pages (depth_cap when depth_capped)
        |     |             | no
        |     v             |
        | unique == effective_limit ? yes -> STOP limit_reached (depth_cap when depth_capped)
        |     | no          |
        |     v             v
        | next_token absent ? yes -> STOP exhausted (depth_cap when depth_capped)
        |     | present
        |     v
        | x-rate-limit-remaining == 0 ? yes -> same wait-or-stop branch as the 429 rate_limited case
        |     | no
        +-----+
```

Every STOP yields the same envelope shape; only `stop.reason`, `complete`, `truncated`, and the exit code differ. Pages
are yielded after dedup and the `data`/`overflow` split, so a streaming consumer never sees an item it must later
retract; only the final page of a run can carry `overflow`. The loop carries the unique-id set (bounded by
`budget.objects[primary]`), the consecutive-empty counter, the last rate-limit triple, the deadline, the cursor of the
page in flight, the waits taken, and the partial-error accumulator.

## Crate surface (shape, not signatures)

- `ListRequest`: endpoint; subject (path parameters); query parameters the endpoint accepts (`query`, `exclude`,
  `sort_order`, field selections, API expansions); `limit`; optional pinned `page_size`; optional `cursor`; and, on
  time-ordered lists, optional `since_id`, `until_id`, `start_time`, `end_time`.
- `plan(&ListRequest) -> Plan` (D4.1), including the default `Budget`.
- `Budget`: `requests`, `objects` by kind (an unbounded kind has none), `deadline` (the `--timeout` deadline),
  `wait_for_rate_limit`.
- `plan_lookup(n) -> LookupPlan`, `lookup_users(ids, lookup source)`, and `lookup_users_by_username(usernames, lookup
  source)`, each chunking by its endpoint's ids ceiling; the lookup source is the crate's HTTP client by default or a
  scripted responder in tests; `usage_credits()` and `usage_posts()`.
- The classified error type carrying the mapped problem (D2.8), returned by every operation.
- `paginate(&ListRequest, page source, Budget)`: a stream of `Page { data, overflow, includes, errors, meta }` followed
  by the final `ListMeta` and `stop`.
- Page source: one operation, "give me this page", answering with a typed page, a decline (`cache_miss`), or an error.
- `StopReason`: the closed set in D3.4 with its payloads, marked non-exhaustive.
- The registry as a public type; a user lookup that chunks by the registry's ids ceiling; `CONTRACT_VERSION` (D9.2)
  beside the seam document's `SEAM_VERSION`. No HTTP-client type appears in a public signature: the client, the
  per-request timeout, and the page source are crate types.

## Envelope (shape, not schema)

The example is a first `--since-checkpoint` run: no record existed, 230 posts were requested with author hydration, and
41 authors were cache misses.

```json
{
  "data": [ "...items, at most limit..." ],
  "overflow": [ "...floor-forced items beyond limit, possibly empty..." ],
  "includes": { "users": [ "..." ] },
  "errors": [ "...partial errors from 200 responses..." ],
  "meta": {
    "returned": 230,
    "overflow_count": 0,
    "complete": true,
    "truncated": true,
    "stop": { "reason": "limit_reached" },
    "next_cursor": "7140w9gefhslx3",
    "previous_cursor": "77qp89slxjd",
    "newest_id": "1204860593741553664",
    "oldest_id": "1204860580630278147",
    "requests_made": 3,
    "objects_fetched": { "post": 230 },
    "duplicates_dropped": 0,
    "hydration": { "requests": 1, "looked_up": 41, "from_cache": 189 },
    "rate_limit": { "limit": 300, "remaining": 297, "reset_at": "2030-01-01T00:00:00Z" },
    "waits": [],
    "cost": {
      "estimate_max_usd": 3.45,
      "budget_usd": 5.00,
      "fetched_max_usd": 1.56,
      "by_kind": { "post": 1.15, "user": 0.41 },
      "owned_read": false
    },
    "checkpoint": { "newest_id": "1204860593741553664", "created": true, "advanced": false, "draining": false }
  }
}
```

`cost`, `checkpoint`, `cache`, and `hydration` are `bird` fields; the crate and `xr` omit them, and `cache` appears
only on `--cache-only` runs. `requests_made` and `objects_fetched` are the crate's list figures; `hydration` reports the
one lookup for 41 misses,
which is where `by_kind.user` comes from; the estimate assumed six requests in total. The budget is the `--max-spend` of
$5.00 scaled onto the plan.

## Decision log

| ID | Decision                                                                                       | Alternatives rejected                                                                                                                                         |
| -- | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1 | `limit` is total unique items in `data`; default 10; overflow delivered                        | Per-request semantics in `xr` (two meanings for one word); discarding floor-forced items (lossy cursor).                                                      |
| D2 | Crate owns registry (bounds, caps, fan-out with evidence checksums),                           | Caller pages (approach A); registry generated at build time from the spec (opaque, harder to review);                                                         |
|    | crate-wide error classification, plan, and a loop over a page source                           | classifying every typeless 429 as a rate limit (out-of-credits would loop forever).                                                                           |
| D3 | Raw cursor, lossless by construction; three stop categories with fixed exit                    | Cursor plus skip offset (re-fetch cost, wrong under a moving head); exit 0 on partial (scripts miss it);                                                      |
|    | codes; opt-in bounded wait on rate limit                                                       | exit codes left to each tool (consumers cannot write one wrapper).                                                                                            |
| D4 | Pure plan; explicit budget binds the loop; prices, two-tier guard with                         | Prices in the crate (not an API fact); balance check per call; binding to the plan's request count                                                            |
|    | defaults, and refusal in `bird`; `--dry-run`                                                   | (short pages under-deliver); a single agent-settable guard (agent holds the key).                                                                             |
| D5 | Floors absorbed by overflow; pinned `page size` never clamped; `plan_cap`                      | Refuse below-floor requests (punishes the common small ask); silent clamp (hides spend).                                                                      |
| D6 | `--since-id` passthrough; checkpoint record with pending drain cursor;                         | Advance on the first page's `newest_id` regardless (skips items when a bounded run stops early);                                                              |
|    | advanced on `exhausted` or `depth_cap`; caller holds no state                                  | caller passes `--cursor` back (state on the agent, stale `since_id` hazard).                                                                                  |
| D7 | Entity cache by id; query cache of ordered ids per identity; no online                         | Page cache keyed by page size and cursor with freshness windows (fragile under dedup, no evidence);                                                           |
|    | substitution; hydration from cache then batched lookup; atomic page writes                     | API `author_id` expansion (pays for cached users); a second loop in `bird` for replay.                                                                        |
| D8 | One word, one meaning; names composed from vocabulary or API names;                            | `--max-results` (P7 alias; "results" is not a word this design uses elsewhere); `--expand author` (one                                                        |
|    | `--hydrate` distinct from `--expand`                                                           | flag, two mechanisms).                                                                                                                                        |
| D9 | A `xurl-rs` document with no client copies, in rustdoc, with `CONTRACT_VERSION` and a          | Contract in `bird` (the layer that owns the API should own its contract); a runtime constant printed by                                                       |
|    | versioned link in `--help`; clients cite the version; evidence in `xurl-rs/vendor/x-api-docs/` | `xr contract` (prose inside a binary is not a common pattern); a byte-identical mirror in every client (sync machinery for text the crate already publishes). |

## Open questions

1. **"Like: Read" at $0.001.** The pricing page lists it without naming an endpoint. This design prices liked posts as
   post reads, because the billing page counts "Liked posts" among tracked post endpoints; if X bills them as likes, the
   table is conservative, not wrong.
2. **Unbounded expansions.** Mentions, article media, and the DM participant, referenced-post, and media expansions have
   no documented maximum, so `bird` refuses them. A documented bound or a maintainer-chosen constant, cited as such,
   would lift each refusal.

## Test matrix

Every row is a test that exists before its code is written. Layers: `crate` (unit, no network), `xr` (CLI smoke),
`bird` (integration through the crate with a scripted page source; never the real API).

| #  | Layer | Path                                 | Assertion                                                                                                                    |
| -- | ----- | ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| 1  | crate | registry vs spec                     | bounds, defaults, cursor names and descriptions, filters, expansion enums, `meta` keys match the spec                        |
| 2  | crate | registry vs prose evidence           | every depth cap and prose-sourced fan-out row's checksum matches its vendored file                                           |
| 3  | crate | `plan()` below floor                 | mentions, limit 3: 1 request, 5 objects, overflow 2                                                                          |
| 4  | crate | `plan()` remainder below floor       | search, limit 103: pages [100, 10], 2 requests, 110 objects, overflow 7                                                      |
| 5  | crate | `plan()` above ceiling and depth cap | user posts `exclude=replies`, limit 1000: effective 800, `depth_capped`, 8 requests                                          |
| 6  | crate | `plan()` pinned page size            | `P` outside bounds errors; inside bounds `requests_max = ceil(L/P)`                                                          |
| 7  | crate | `plan()` expansions                  | `objects_max[k]` is primary times fan-out; an unbounded expansion clears `ceiling_is_bound`                                  |
| 8  | crate | loop budget gate                     | short pages and duplicates never exceed the budget's requests or objects; stop is `plan_cap`                                 |
| 9  | crate | loop resume after `plan_cap`         | a second run from `next_cursor` delivers the rest with no gap and no repeat                                                  |
| 10 | crate | dedup and overflow split             | first occurrence wins; overflow only on the final page; `overflow_count` correct                                             |
| 11 | crate | empty-page guard                     | two consecutive empty pages with a cursor stop `empty_pages`; one does not                                                   |
| 12 | crate | depth cap stops                      | each natural end under `depth_capped` reports `depth_cap` with no `next_cursor`                                              |
| 13 | crate | `next_cursor` per stop               | the lossless table in D3.2 holds for every stop reason, including head-page failure                                          |
| 14 | crate | `newest_id` and `oldest_id`          | max and min over delivered items under recency and relevancy; absent when empty                                              |
| 15 | crate | partial errors                       | `errors` beside `data` are collected and the loop continues                                                                  |
| 16 | crate | rate-limit remaining 0               | the loop stops before the next fetch with `retry_after_seconds` and `reset_at`                                               |
| 17 | crate | `--wait-for-rate-limit`              | sleeps and continues when `reset_at` precedes the deadline, else stops; `waits` recorded                                     |
| 18 | crate | timeout                              | stops `timeout` when remaining time is below the per-request timeout; partial data delivered                                 |
| 19 | crate | 429 classification                   | typed or remaining 0 is `rate_limited`; `usage-capped` fails; typeless with remaining is `ambiguous_429`                     |
| 20 | crate | non-list classification              | a single lookup and a write map 429s and problems identically to the list loop                                               |
| 21 | crate | page source decline                  | a declining source stops `cache_miss`; a source answering without a request adds 0                                           |
| 22 | crate | `StopReason` non-exhaustive          | a downstream exhaustive match must carry a wildcard (compile-time check)                                                     |
| 23 | xr    | flags and envelope                   | every list command accepts the D8 flags; `meta` core equals the crate's exported `ListMeta` snapshot                         |
| 24 | xr    | `--dry-run`                          | prints requests and objects by kind; zero requests                                                                           |
| 25 | xr    | `xr endpoints --json`, `--help` link | the registry prints; the help link names the running binary's version tag                                                    |
| 26 | xr    | exit codes                           | 0, 75, 80 per category; 81 reserved, never emitted by xr; no collision with existing codes                                   |
| 27 | bird  | price table checksum                 | the test fails when the pricing page body changes                                                                            |
| 28 | bird  | owned-read conditions                | the tier applies only with the config assertion, an owned-read endpoint, and the own subject                                 |
| 29 | bird  | unpriced kinds                       | $0 contribution and named in the estimate                                                                                    |
| 30 | bird  | two-tier refusal                     | over hard cap, over `max_spend`, unbounded expansion, and `--expand author_id` each refuse with tier and override            |
| 31 | bird  | defaults                             | $1.00 and $5.00 apply when configuration is absent                                                                           |
| 32 | bird  | budget scaling                       | objects scale by `max_spend / estimate`; short pages fill to `limit` within `max_spend`                                      |
| 33 | bird  | `--dry-run`                          | prints plan, estimate, and budget in USD; zero requests                                                                      |
| 34 | bird  | hydration                            | cache first; misses to the crate lookup through a scripted lookup source; `hydration` reports requests and misses            |
| 35 | bird  | `--hydrate` on a non-post list       | errors before any request                                                                                                    |
| 36 | bird  | cost fields                          | `fetched_max_usd` from `objects_fetched` plus hydration lookups; `budget_usd` reported                                       |
| 37 | bird  | entity cache                         | upserts from `data`, `overflow`, and `includes` with observation time                                                        |
| 38 | bird  | query cache merge                    | head run replaces (stable) or merges by id (time-ordered); end cursor appends; other cursors do not touch it                 |
| 39 | bird  | `--cache-only`                       | serves stored ids in order up to `limit`; `cache_miss` with the stored end cursor when short; $0                             |
| 40 | bird  | atomic page writes                   | a failure injected after the entity upsert leaves no partial page in the query cache                                         |
| 41 | bird  | checkpoint create                    | a first run with items creates the record; a first run with nothing creates nothing                                          |
| 42 | bird  | checkpoint advance                   | advances only on `exhausted` or `depth_cap`; other stops store the drain cursor and `since_id`                               |
| 43 | bird  | drain                                | repeated `--since-checkpoint` until `draining` is false drains a backlog larger than `limit`, then polls; no skip, no repeat |
| 44 | bird  | flag conflicts                       | `--since-checkpoint` with `--cursor` or any time or id filter errors before any request                                      |
| 45 | bird  | envelope parity                      | `meta` core equals the crate's exported `ListMeta` snapshot, plus `cost`, `checkpoint`, `cache`, `hydration`                 |
| 46 | bird  | exit codes                           | 0, 75, 80, 81 per category; no collision with existing codes                                                                 |
| 47 | bird  | versions cited                       | the version `bird`'s client-conformance doc cites equals `CONTRACT_VERSION` of the crate it builds against                   |
| 48 | crate | lookup chunking                      | 100 ids or usernames make 1 request, 101 make 2; `plan_lookup` agrees with both lookups                                      |

## Seam

Every placement in this contract is recorded in the worked matrix of
`docs/designs/2026-09-04-xurl-bird-seam.md`, one row per concern with its implementation, its surface, and the
question that decided it.

## Apply to each repo

Contract level only; implementation is planned separately in each repository.

### xurl-rs (crate)

- Endpoint registry as public API, per list endpoint in the bounds table (search recent, mentions, liked posts, user
  posts, home timeline, bookmarks, followers, following, DM events and both per-conversation variants): list class,
  floor, ceiling, API default where declared, cursor parameter, time filters, depth cap as a function of request shape,
  primary resource kind, per-expansion kind and fan-out, the evidence checksum for every prose-sourced value, and,
  per lookup endpoint, the ids ceiling.
- `plan_lookup()`, `lookup_users()`, `lookup_users_by_username()`, `usage_credits()`, and `usage_posts()` as crate
  operations.
- `ListRequest`, `Plan`, `Budget`, `plan()`, `paginate()` over a page source under a budget, `Page`, `ListMeta`, and the
  closed, non-exhaustive `StopReason` set with its payloads.
- Deduplication by id within a run; the runtime page-size rule under the budget; pinned page-size validation;
  rate-limit-aware stop and the opt-in wait; empty-page guard; crate-wide error classification per D2.8.
- Registry-versus-spec drift test and registry-versus-prose checksum test against `vendor/`.
- The canonical text included in rustdoc, and `CONTRACT_VERSION`, set when `status` is `accepted` and equal to the
  frontmatter `version`.

### xurl-rs (`xr` CLI)

- Every list command: `--limit`, `--cursor`, `--page-size`, `--dry-run`, `--wait-for-rate-limit`, `--quiet`,
  `--verbose`, `--timeout`; time-ordered lists add `--since-id`, `--until-id`, `--start-time`, `--end-time`, and the
  endpoint's own `--exclude` and `--sort-order` where the API accepts them; `--expand` with API expansion names.
- `xr endpoints [--json]` prints the registry; `--help` links to the contract at the binary's version tag.
- User lookup and usage commands as passthroughs of the crate operations.
- Envelope as above without `cost`, `checkpoint`, `cache`, and `hydration`; exit codes 75 and 80 (81 is reserved).

### xurl-rs (docs and evidence)

- This document and the scope policy at `docs/designs/`, packaged so rustdoc can include them; the reusable
  Seam-section workflow clients call.
- `vendor/x-api-docs/` holding the vendored prose evidence with its `INDEX.md` and re-vendor procedure;
  `vendor/x-api-openapi.json` and `vendor/spec-metadata.json` stay the spec and its provenance record.

### bird (CLI)

- Every list command: the `xr` flag set minus `--page-size`, plus `--max-spend`, `--hydrate`, `--cache-only`, and, on
  time-ordered lists, `--since-checkpoint`.
- Price table with the owned-read tier, the app-ownership assertion, `spend.hard_cap_usd` and `spend.max_spend_usd` in
  configuration, and the pricing page's checksum; the two-tier refusal error; budget derivation; `cost`, `checkpoint`,
  `cache`, and `hydration` envelope fields; exit codes 75, 80, 81.
- User lookup and usage commands as passthroughs of the crate operations.
- A page source wrapping the crate's HTTP source for per-page cache writes, and a cache-only page source; entity cache,
query cache, and checkpoint records
  in SQLite with atomic per-page writes; hydration per D7.5.
- Registry consumed by import; no table of its own.

### bird (docs and evidence)

- A client-conformance document that cites this contract's `version` and the scope policy's, lists `bird`'s
  conformance tests, and carries the version test against the crate constants; no copy of either document.
- Citations to `vendor/x-api-docs/<file>` at the pinned crate version; no evidence copy of its own.
