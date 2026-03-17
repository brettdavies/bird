# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Shell completion generation via `bird completions <shell>` (bash, zsh, fish, powershell, elvish)
- `bird doctor` now works without xurl installed (reports `xurl.available: false`)
- SIGPIPE handling for clean pipe behavior (`bird completions bash | head`)
- Distribution via crates.io (`cargo install bird`) and cargo-binstall
- Dual license: MIT OR Apache-2.0

### Changed

- `main()` restructured to gate xurl fail-fast by command need
- Release binaries built with `codegen-units = 1` and `panic = "abort"` for smaller size
- GitHub Actions pinned by commit SHA for supply chain security
