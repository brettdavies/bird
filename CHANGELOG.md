# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- Remove legacy OAuth config fields and cleanup
- Remove remaining legacy auth references
- Remove unused OpenAPI spec, scripts, and references

## [0.1.0] - 2026-03-17

### Added

- Add BirdDb SQLite cache + cost estimation modules
- Wire CachedClient into all handlers
- Add bird cache clear/stats, doctor integration, login auto-clear
- Add search command with filtering, sorting, and pagination
- Add bird profile command for user lookup
- Add bird thread command for conversation reconstruction
- Add watchlist and usage commands
- Add watchlist and usage commands (#5)
- Add xurl subprocess transport layer
- Add shell completions subcommand and restructure main()
- Add --quiet flag to suppress informational stderr output
- Structured JSON error output with --output flag

### Changed

- Add dedicated SEARCH_ACCEPTED auth constant
- Unify cache layer across auth types and fix resolution order (#7)
- Replace request-level cache with entity store
- Decouple ApiResponse from reqwest types
- Complete Phase 2 migration to xurl transport
- Complete Phase 3 cleanup and add write commands
- Rename BOOKMARKS_ACCEPTED to OAUTH2_ONLY and add sync test
- Extract validate_username to schema.rs
- Remove unnecessary json.clone() in get and batch_get
- Polish — account validation, dead params, HashMap, URL parse, Cow<str>
- Rename --account to --username for xurl alignment
- Rename account_username to username in bookmarks schema
- Support xr (xurl-rs) binary alongside xurl
- Extract clap definitions from main.rs to cli.rs
- Delete unused generated types module (10,343 lines)
- GA release readiness v0.1.0 (#14)

### Documentation

- Document SQLite cache layer solution
- Document search command implementation pattern
- Add profile and thread commands to CLI design doc
- Document thread/profile command patterns
- Add all plan documents for research commands series
- Add brainstorm and plan for xurl transport layer refactor
- Mark Phase 1 tasks complete in transport layer plan
- Mark Phase 2 tasks complete in transport plan
- Mark transport layer and terminology plans completed
- Add xurl transport layer solution document
- Add CI formatting drift solution document
- Update status to completed on finished plans and brainstorms
- Add brainstorms and plans from 2026-03-16 sessions
- Add AGENTS.md for AI-assisted development context
- Update brainstorm and plan status for GA readiness work

### Fixed

- Resolve 15 code review findings across security, performance, and quality (#1)
- Resolve pre-existing clippy warnings across auth, login, output
- Address P1/P2 code review findings
- Cap --cache-ttl at 24h to prevent stale-forever entries
- Address review findings in thread command
- Use semver crate for xurl version comparison
- Forward --account flag to write commands
- Restore exit code 77 for auth errors with map_cmd_error helper
- Validate and canonicalize BIRD_XURL_PATH
- Correct cargo pkgid version extraction in release workflow
