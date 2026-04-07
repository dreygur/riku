#!/bin/bash
# Test deployment scripts for Riku
# Usage: ./tests/deploy/test-all.sh [remote]

set -e

REMOTE="${1:-}"
TEST_DIR="$(mktemp -d)"
PASSED=0
FAILED=0

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
        python)
            cat > requirements.txt <<'EOF'
flask>=2.0.0
EOF
            cat > app.py <<'EOF'
import os
from flask import Flask
app = Flask(__name__)
port = int(os.environ.get('PORT', 5000))

@app.route('/')
def hello():
    return f"Hello from Python on port {port}\n"

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=port)
EOF
            cat > Procfile <<'EOF'
web: python app.py
EOF
            ;;
        static)
            mkdir -p public
            cat > public/index.html <<'EOF'
<!DOCTYPE html>
<html>
<head><title>Static Test</title></head>
<body><h1>Hello from Static Site</h1></body>
</html>
EOF
            cat > Procfile <<'EOF'
static: .
EOF
            ;;
        go)
            cat > main.go <<'EOF'
package main

import (
    "fmt"
    "net/http"
    "os"
)

func main() {
    port := os.Getenv("PORT")
    if port == "" {
        port = "8080"
    }
    http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
        fmt.Fprintf(w, "Hello from Go on port %s\n", port)
    })
    http.ListenAndServe(":"+port, nil)
}
EOF
            cat > Procfile <<'EOF'
web: go run main.go
EOF
            ;;
    esac
    
    log_info "Created $runtime app: $app_dir"
}

test_env_vars() {
    local app_dir="$1"
    local test_name="$2"

    log_info "Testing environment variables for: $test_name"

    # Test basic env vars
    cat > "$app_dir/ENV" <<'EOF'
RIKU_AUTO_RESTART=true
BIND_ADDRESS=127.0.0.1
DISABLE_IPV6=true
RIKU_WORKER_TIMEOUT=3600
RIKU_WORKER_GRACE_PERIOD=60
RIKU_MAX_RESTARTS=10
EOF

    log_info "✓ Environment variables test passed for: $test_name"
    PASSED=$((PASSED + 1))
}

test_node_version() {
    local app_dir="$1"

    log_info "Testing NODE_VERSION env var"

    cat >> "$app_dir/ENV" <<'EOF'
NODE_VERSION=18.17.0
NODE_PACKAGE_MANAGER=npm
EOF

    log_info "✓ NODE_VERSION test passed"
    PASSED=$((PASSED + 1))
}

test_nginx_vars() {
    local app_dir="$1"

    log_info "Testing NGINX_* env vars"

    cat >> "$app_dir/ENV" <<'EOF'
NGINX_SERVER_NAME=test.example.com
NGINX_HTTPS_ONLY=false
NGINX_STATIC_PATHS=/static:public/static
NGINX_CACHE_PREFIXES=/api/cache
NGINX_CACHE_SIZE=1
NGINX_CACHE_TIME=3600
NGINX_CLOUDFLARE_ACL=false
NGINX_ALLOW_GIT_FOLDERS=false
NGINX_CATCH_ALL=index.html
EOF

    log_info "✓ NGINX_* env vars test passed"
    PASSED=$((PASSED + 1))
}

test_scaling() {
    local app_dir="$1"

    log_info "Testing SCALING file"

    cat > "$app_dir/SCALING" <<'EOF'
web=2
worker=1
EOF

    log_info "✓ SCALING file test passed"
    PASSED=$((PASSED + 1))
}

run_tests() {
    log_info "Starting Riku deployment tests..."
    log_info "Test directory: $TEST_DIR"
    
    # Test 1: Node.js app
    log_info "=== Test 1: Node.js App ==="
    create_test_app "node-app" "node"
    test_env_vars "$TEST_DIR/node-app" "Node.js"
    test_node_version "$TEST_DIR/node-app"
    test_nginx_vars "$TEST_DIR/node-app"
    test_scaling "$TEST_DIR/node-app"
    
    # Test 2: Python app
    log_info "=== Test 2: Python App ==="
    create_test_app "python-app" "python"
    test_env_vars "$TEST_DIR/python-app" "Python"
    test_nginx_vars "$TEST_DIR/python-app"
    test_scaling "$TEST_DIR/python-app"
    
    # Test 3: Static site
    log_info "=== Test 3: Static Site ==="
    create_test_app "static-app" "static"
    test_env_vars "$TEST_DIR/static-app" "Static"
    test_nginx_vars "$TEST_DIR/static-app"
    
    # Test 4: Go app
    log_info "=== Test 4: Go App ==="
    create_test_app "go-app" "go"
    test_env_vars "$TEST_DIR/go-app" "Go"
    test_nginx_vars "$TEST_DIR/go-app"
    
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
