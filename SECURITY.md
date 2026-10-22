# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in bird, report it through GitHub's private vulnerability reporting rather than
a public issue:

[Report a vulnerability](https://github.com/brettdavies/bird/security/advisories/new)

The form opens a private advisory that only the maintainer can see. Include the `bird --version` output, the command or
input that triggers the problem, and what an attacker could do with it.

## Disclosure Window

Reports are acknowledged within 5 business days. A fix or mitigation ships within 90 days of the initial report.
Coordinated disclosure is preferred: the public-disclosure date is agreed with the reporter so that users are protected
and the finder is credited.

## Scope

In scope:

- The `bird` binary: argument handling, the entity store and config under `~/.config/bird`, the `bird skill install`
  clone, and anything that could expose a credential or send it somewhere other than the configured API origin.
- Release artifacts: the crates.io package, the GitHub release binaries, and the Homebrew formula.

Out of scope:

- Token storage, OAuth flows, and request signing, which live in [`xdk-rs`](https://github.com/brettdavies/xurl-rs).
  Report those to xurl-rs.
- The X API itself and the behavior of X's servers. Report those to X.
- The agent skill bundle in [`bird-skill`](https://github.com/brettdavies/bird-skill); a bundle finding is welcome here,
  but describe which repository it concerns.
- Rate limits, pricing, and account enrollment decisions made by X.

## Supported Versions

Only the latest tagged release receives security fixes. Older tags are immutable historical records.
