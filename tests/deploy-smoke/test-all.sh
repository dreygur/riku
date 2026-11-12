#!/bin/bash
# Test deployment scripts for Riku
# Usage: ./tests/deploy-smoke/test-all.sh

set -e

TEST_DIR="$(mktemp -d)"
PASSED=0
FAILED=0

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The settings tests drive a built riku against a throwaway RIKU_ROOT.
RIKU_BIN="${RIKU_BIN:-$REPO_ROOT/target/release/riku}"
RIKU_TEST_ROOT="$TEST_DIR/riku-root"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

cleanup() {
    log_info "Cleaning up test directory: $TEST_DIR"
    rm -rf "$TEST_DIR"
}

trap cleanup EXIT

create_test_app() {
    local name="$1"
    local runtime="$2"
    local app_dir="$TEST_DIR/$name"
    
    mkdir -p "$app_dir"
    cd "$app_dir"
    
    case "$runtime" in
        node)
            cat > package.json <<'EOF'
{
  "name": "test-node-app",
  "version": "1.0.0",
  "scripts": {
    "start": "node server.js"
  }
}
EOF
            cat > server.js <<'EOF'
const http = require('http');
const port = process.env.PORT || 3000;
const server = http.createServer((req, res) => {
  res.writeHead(200, {'Content-Type': 'text/plain'});
  res.end(`Hello from Node.js on port ${port}\n`);
});
server.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port ${port}`);
});
EOF
            cat > Procfile <<'EOF'
web: node server.js
EOF
            ;;
    esac
    
    log_info "Created $runtime app: $app_dir"
}

# riku with the test root, never forwarded to a registered server.
riku_local() {
    RIKU_ROOT="$RIKU_TEST_ROOT" "$RIKU_BIN" --local "$@"
}

# Succeeds when `riku config set APP KEY=1` is refused with a pointer to
# the riku settings key PATH and no KEY line reached the app's ENV.
setting_refused_in_env() {
    local app="$1" key="$2" path="$3"
    local output
    if output=$(riku_local config set "$app" "$key=1" 2>&1); then
        log_error "config set accepted the riku setting $key"
        return 1
    fi
    if [[ "$output" != *"riku settings set $app $path="* ]]; then
        log_error "refusing $key did not name $path: $output"
        return 1
    fi
    if grep -q "^$key=" "$RIKU_TEST_ROOT/envs/$app/ENV" 2>/dev/null; then
        log_error "$key reached the ENV of $app"
        return 1
    fi
}

# Each "KEY PATH" pair must be refused by setting_refused_in_env.
check_settings_refused() {
    local app="$1" label="$2"
    shift 2
    local pair failed=0
    for pair in "$@"; do
        # shellcheck disable=SC2086 # "KEY PATH" splits into two arguments.
        setting_refused_in_env "$app" $pair || failed=1
    done
    if [ "$failed" -eq 0 ]; then
        log_info "✓ $label stay out of ENV"
        PASSED=$((PASSED + 1))
    else
        FAILED=$((FAILED + 1))
    fi
}

# Worker and bind address settings come only from riku.toml or
# `riku settings`, so `riku config set` must refuse them.
test_env_vars() {
    local app="$1"
    log_info "Testing that worker settings are refused as env vars for: $app"
    check_settings_refused "$app" "Worker settings" \
        "RIKU_BIND_ADDRESS routing.bind_address" \
        "RIKU_WORKER_TIMEOUT workers.timeout" \
        "RIKU_WORKER_GRACE_PERIOD workers.grace_period" \
        "RIKU_MAX_RESTARTS workers.max_restarts"
}

# A riku.toml `[runtime] version` that this machine's node does not match
# must fail the build in the node plugin, naming both versions. The plugin
# checks npm first, so without npm the check cannot be reached.
test_node_version() {
    local app_dir="$1"
    local app="node-version-app"
    local output

    log_info "Testing that a [runtime] version mismatch fails the node build"
    if ! command -v npm >/dev/null 2>&1; then
        log_warn "npm not found, node version test skipped"
        return
    fi
    riku_local install-plugins --plugins node >/dev/null
    create_riku_app "$app"
    printf '[runtime]\nname = "node"\nversion = "0.1"\n' > "$app_dir/riku.toml"

    if output=$(riku_local deploy "$app" --from "$app_dir" 2>&1); then
        log_error "deploy succeeded with [runtime] version 0.1"
        FAILED=$((FAILED + 1))
        return
    fi
    if [[ "$output" != *"RIKU_NODE_VERSION is 0.1, but this server's node is"* ]]; then
        log_error "node plugin did not report the version mismatch: $output"
        FAILED=$((FAILED + 1))
        return
    fi
    log_info "✓ node plugin refused [runtime] version 0.1"
    PASSED=$((PASSED + 1))
}

# The nginx route reads domains, HTTPS and static paths only from riku.toml
# or `riku settings`, so `riku config set` must refuse their env keys.
test_nginx_vars() {
    local app="$1"
    log_info "Testing that routing settings are refused as env vars for: $app"
    check_settings_refused "$app" "Routing settings" \
        "RIKU_DOMAINS routing.domains" \
        "RIKU_HTTPS_ONLY routing.https_only" \
        "RIKU_NGINX_STATIC_PATHS routing.static_paths" \
        "RIKU_NGINX_ALLOW_GIT_FOLDERS routing.allow_git_folders"
}

# A riku app in the test root for the settings tests.
create_riku_app() {
    local app="$1"
    if [ ! -x "$RIKU_BIN" ]; then
        log_error "riku binary not found at $RIKU_BIN; run cargo build --release or set RIKU_BIN"
        exit 1
    fi
    if ! riku_local apps create "$app" >/dev/null 2>&1; then
        log_error "riku apps create $app failed"
        exit 1
    fi
}

# `riku ps APP --scale worker=2` must record the count and write a worker
# config per copy. The app runs `sleep` through a stub runtime plugin, so no
# toolchain is needed. No supervisor runs under the test root, so the
# deploy's exit status after the worker configs are written is not checked;
# `riku stop` cleans up either way.
test_scaling() {
    local app="scale-app"
    local src="$TEST_DIR/$app-src"
    local available="$RIKU_TEST_ROOT/workers-available"
    local failed=0

    log_info "Testing riku ps --scale"
    mkdir -p "$src" "$available" "$RIKU_TEST_ROOT/workers-enabled" \
        "$RIKU_TEST_ROOT/plugins"
    # shellcheck disable=SC2016 # $1 belongs to the plugin, not this script.
    printf '#!/bin/sh\ncase "$1" in start) echo true ;; esac\n' \
        > "$RIKU_TEST_ROOT/plugins/stub"
    chmod +x "$RIKU_TEST_ROOT/plugins/stub"
    echo "worker: sleep 600" > "$src/Procfile"
    create_riku_app "$app"

    riku_local deploy "$app" --from "$src" >/dev/null 2>&1 || true
    riku_local ps "$app" --scale worker=2 >/dev/null 2>&1 || true

    if ! grep -qx "worker=2" "$RIKU_TEST_ROOT/envs/$app/SCALING" 2>/dev/null; then
        log_error "SCALING does not record worker=2"
        failed=1
    fi
    for copy in 1 2; do
        if [ ! -f "$available/$app@worker.$copy.toml" ]; then
            log_error "no worker config for copy $copy of $app"
            failed=1
        fi
    done
    riku_local stop "$app" >/dev/null 2>&1 || true

    if [ "$failed" -eq 0 ]; then
        log_info "✓ ps --scale wrote two worker configs"
        PASSED=$((PASSED + 1))
    else
        FAILED=$((FAILED + 1))
    fi
}

run_tests() {
    log_info "Starting Riku deployment tests..."
    log_info "Test directory: $TEST_DIR"
    
    # Test 1: riku settings in ENV. The refusal does not depend on the
    # runtime, so one app covers it.
    log_info "=== Test 1: Settings stay out of ENV ==="
    create_riku_app "settings-app"
    test_env_vars "settings-app"
    test_nginx_vars "settings-app"

    # Test 2: Node.js app
    log_info "=== Test 2: Node.js App ==="
    create_test_app "node-app" "node"
    test_node_version "$TEST_DIR/node-app"

    # Test 3: Scaling, which does not depend on the runtime.
    log_info "=== Test 3: Scaling ==="
    test_scaling

    # Summary
    echo ""
    log_info "================================"
    log_info "Test Summary"
    log_info "================================"
    log_info "Passed: $PASSED"
    log_info "Failed: $FAILED"
    log_info "================================"
    
    if [ $FAILED -gt 0 ]; then
        log_error "Some tests failed!"
        exit 1
    else
        log_info "All tests passed! ✓"
        exit 0
    fi
}

# Run tests
run_tests
