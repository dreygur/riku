#!/usr/bin/env bash
set -euo pipefail

# Builds run under RLIMIT_AS (default 512MB). node/v8 reserves multiple GB of
# *virtual* address space at startup and aborts under a tight RLIMIT_AS, so the
# e2e suite raises the build memory ceiling. This is test-environment config
# only — it does not change riku's production default.
export RIKU_MAX_MEMORY_MB="${RIKU_MAX_MEMORY_MB:-4096}"

PASS=0
FAIL=0
ERRORS=()

run_test() {
    local name="$1"
    local script="$2"
    local test_home
    test_home=$(mktemp -d /tmp/riku-test-XXXXXX)
    export RIKU_ROOT="$test_home"

    echo ""
    echo "━━━ $name ━━━"
    if RIKU_ROOT="$test_home" bash "$script" 2>&1; then
        echo "✓ PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "✗ FAIL: $name"
        FAIL=$((FAIL + 1))
        ERRORS+=("$name")
    fi
    rm -rf "$test_home"
}

# Run all test scripts in lexicographic order
for script in /riku-src/tests/e2e/cases/[0-9]*.sh; do
    [ -f "$script" ] || continue
    name=$(basename "$script" .sh)
    run_test "$name" "$script"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Results: $PASS passed, $FAIL failed"

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "Failed tests:"
    for e in "${ERRORS[@]}"; do
        echo "  - $e"
    done
    exit 1
fi

exit 0
