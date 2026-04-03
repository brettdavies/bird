#!/usr/bin/env bash
# Local CI mirror — runs the same checks as the GitHub Actions CI pipeline.
# Use as a pre-push hook or run manually before pushing.
#
# Exit codes: 0 = all pass, 1 = failure
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { echo -e "  ${GREEN}✓${RESET} $1"; }
fail() { echo -e "  ${RED}✗${RESET} $1"; exit 1; }

echo -e "${BOLD}Running local CI checks...${RESET}"

# 1. Format check
cargo fmt -- --check 2>/dev/null || fail "cargo fmt -- --check"
pass "fmt"

# 2. Clippy with warnings-as-errors (matches CI's RUSTFLAGS=-Dwarnings)
RUSTFLAGS="-Dwarnings" cargo clippy --quiet 2>&1 || fail "cargo clippy -Dwarnings"
pass "clippy"

# 3. Tests
cargo test --quiet 2>&1 || fail "cargo test"
pass "test"

# 4. Security audit (licenses + advisories) — skip if cargo-deny not installed
if cargo deny --version &>/dev/null; then
    cargo deny check 2>&1 || fail "cargo deny check"
    pass "deny"
else
    echo "  - deny (skipped, cargo-deny not installed)"
fi

# 5. Windows cross-check: verify no unconditional use of unix-only APIs
# This catches the libc::SIGPIPE issue without needing a Windows toolchain.
if rg -n 'libc::(SIGPIPE|SIG_DFL|signal)' --type rust src/ | rg -v '#\[cfg(unix)\]' | rg -v '// *#\[cfg' | rg -qv '^$'; then
    # Found libc unix calls — check they're inside #[cfg(unix)] blocks
    violations=$(rg -n 'libc::(SIGPIPE|SIG_DFL|signal)' --type rust src/ 2>/dev/null || true)
    if [ -n "$violations" ]; then
        # Check each file for proper cfg gating
        while IFS= read -r line; do
            file=$(echo "$line" | cut -d: -f1)
            lineno=$(echo "$line" | cut -d: -f2)
            # Look for #[cfg(unix)] in the 3 lines before the libc call
            before=$(sed -n "$((lineno > 3 ? lineno - 3 : 1)),${lineno}p" "$file")
            if ! echo "$before" | rg -q 'cfg\(unix\)'; then
                fail "libc unix-only API used without #[cfg(unix)] at $file:$lineno"
            fi
        done <<< "$violations"
    fi
    pass "windows compat"
else
    pass "windows compat"
fi

echo -e "${BOLD}${GREEN}All checks passed.${RESET}"
