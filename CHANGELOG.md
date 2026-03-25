# Changelog

All notable changes to this project will be documented in this file.

## [0.1.2] - 2026-03-19

### Fixed

- Isolate config via XDG_CONFIG_HOME in CLI smoke tests (#16)
- Filter auto-changelog commits from cliff.toml (#17)
- Pass CHANGELOG_TOKEN for ruleset bypass (#19)

## [0.1.1] - 2026-03-17

### Changed

- Remove legacy OAuth config fields and cleanup
- Remove remaining legacy auth references
- Remove unused OpenAPI spec, scripts, and references
- Reflow markdown to 120-char lines and fix MD060 table alignment
- Add project-level markdownlint-cli2 config (120-char line length)
- Update RELEASING.md with release branch pattern and Trusted Publishing status

### Fixed

- Fix Trusted Publishing token wiring for crates.io publish
- Fix rustfmt drift in watchlist tests

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

### Changed

- Add dedicated SEARCH_ACCEPTED auth constant
- GA release readiness v0.1.0 (#14)

### Documentation

- Document SQLite cache layer solution
- Document search command implementation pattern
- Add profile and thread commands to CLI design doc
- Document thread/profile command patterns
- Add all plan documents for research commands series

### Fixed

- Resolve 15 code review findings across security, performance, and quality (#1)
- Resolve pre-existing clippy warnings across auth, login, output
- Address P1/P2 code review findings
- Cap --cache-ttl at 24h to prevent stale-forever entries
- Address review findings in thread command
- Correct cargo pkgid version extraction in release workflow
