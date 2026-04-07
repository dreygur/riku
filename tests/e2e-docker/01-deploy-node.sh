#!/usr/bin/env bash
# E2E test: deploy a minimal Node.js application and verify worker config is created.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="nodeapp"
setup_app "$APP"

# Build a minimal Node.js app tree
src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/package.json" << 'EOF'
{"name":"testapp","version":"1.0.0","scripts":{"start":"node server.js"}}
EOF

cat > "$src/Procfile" << 'EOF'
web: node server.js
EOF

cat > "$src/server.js" << 'EOF'
const http = require('http');
http.createServer((req, res) => res.end('ok')).listen(process.env.PORT || 5000);
EOF

# Skip npm install — we only care about worker config creation
RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# The worker config is written to workers-available as {app}-{kind}-{ordinal}.toml
# and symlinked into workers-enabled.
assert_file_exists "$RIKU_ROOT/workers-available/${APP}-web-1.toml"
assert_file_exists "$RIKU_ROOT/workers-enabled/${APP}-web-1.toml"

# The toml must reference 'node' in the command
assert_file_contains "$RIKU_ROOT/workers-available/${APP}-web-1.toml" "node"

echo "Node app deployed successfully"
