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

    # Keep the legacy sites-enabled path inside $test_home so nothing a test
    # does can reach the real /etc/nginx/sites-enabled.
    local sites_enabled="$test_home/nginx-sites-enabled"
    mkdir -p "$sites_enabled"
    export RIKU_NGINX_SITES_ENABLED="$sites_enabled"

    # Deploy and destroy refuse unless nginx loads this root's sites, which
    # `sudo riku init` arranges on a real host. Write the same include line
    # init would, at a per-case path the system nginx never reads.
    local host_include="$test_home/nginx-host-include.conf"
    printf 'include %s/nginx/*.conf;\n' "$test_home" > "$host_include"
    export RIKU_NGINX_HOST_INCLUDE="$host_include"

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
