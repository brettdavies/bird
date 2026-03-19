# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.2] - 2026-03-19

### Fixed

- Isolate config via XDG_CONFIG_HOME in CLI smoke tests (#16)
- Filter auto-changelog commits from cliff.toml (#17)

### Changed

- Migrate bird workflows to reusable workflows in brettdavies/.github (#15)
- Add GitHub rulesets for main and development branch protection (#18)
- Consolidate CI tokens into single CI_RELEASE_TOKEN (#19, #20)

## [0.1.1] - 2026-03-17

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
