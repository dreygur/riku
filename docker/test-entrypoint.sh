#!/usr/bin/env bash
set -euo pipefail

# Only the router helper writes nginx config and reloads nginx, so it runs as
# root before any case deploys. nginx loads what it installs in /etc/riku/nginx.
mkdir -p /etc/riku /etc/nginx/conf.d
chmod 755 /etc/riku
printf 'include /etc/riku/nginx/*.conf;\n' > /etc/nginx/conf.d/riku.conf
nginx
riku router-helper &
for _ in $(seq 1 50); do
    [ -S /run/riku-router.sock ] && break
    sleep 0.1
done
if [ ! -S /run/riku-router.sock ]; then
    echo "the router helper did not create /run/riku-router.sock" >&2
    exit 1
fi

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
    if bash "$script" 2>&1; then
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
