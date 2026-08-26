#!/usr/bin/env bash
set -euo pipefail

# The deploy path reloads nginx after writing a config. With no nginx running
# that reload fails on an empty /run/nginx.pid and the tests never exercise it.
nginx

PASS=0
FAIL=0
ERRORS=()

run_test() {
    local name="$1"
    local script="$2"
    local test_home
    test_home=$(mktemp -d /tmp/riku-test-XXXXXX)
    export RIKU_ROOT="$test_home"

    # Riku symlinks each generated config into the nginx sites-enabled
    # directory, which is system-wide and so outside $test_home. Point it
    # inside instead: otherwise the rm -rf below leaves a dangling symlink in
    # /etc/nginx/sites-enabled and every later test's nginx reload fails on it.
    local sites_enabled="$test_home/nginx-sites-enabled"
    mkdir -p "$sites_enabled"
    export RIKU_NGINX_SITES_ENABLED="$sites_enabled"

    echo ""
    echo "━━━ $name ━━━"
    if RIKU_ROOT="$test_home" RIKU_NGINX_SITES_ENABLED="$sites_enabled" bash "$script" 2>&1; then
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
