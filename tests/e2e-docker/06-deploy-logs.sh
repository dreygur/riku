#!/usr/bin/env bash
# E2E test: verify that a deploy log file is created and that
# `riku logs --deploy` prints its contents.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="logapp"
setup_app "$APP"

# Build a minimal Node.js app tree
src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/package.json" << 'EOF'
{"name":"logapp","version":"1.0.0"}
EOF

cat > "$src/Procfile" << 'EOF'
web: node server.js
EOF

cat > "$src/server.js" << 'EOF'
require('http').createServer((req, res) => res.end('ok')).listen(process.env.PORT || 5000);
EOF

# Skip npm install — we only care about logging, not the actual build
RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# The deploy log must exist after a successful push.
assert_file_exists "$RIKU_ROOT/logs/${APP}/deploy.log"

# The log must contain the deploy start marker written by DeployLogger.
assert_file_contains "$RIKU_ROOT/logs/${APP}/deploy.log" "Deploying"

# `riku logs --deploy` must print the log to stdout.
output=$(RIKU_ROOT="$RIKU_ROOT" riku logs "$APP" --deploy)
if ! echo "$output" | grep -q "Deploying"; then
    echo "ASSERTION FAILED: deploy log output missing expected content"
    echo "Got:"
    echo "$output"
    exit 1
fi

echo "Deploy logs verified successfully"
