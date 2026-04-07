#!/usr/bin/env bash
# E2E test: deploy an app, destroy it, verify the directories are cleaned up.
# Riku destroy removes: apps/, repos/, envs/, logs/, workers-available/*.toml,
# workers-enabled/*.toml, and the nginx config.  It preserves data/ and cache/.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="destroyapp"
setup_app "$APP"

src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/package.json" << 'EOF'
{"name":"destroyapp","version":"1.0.0"}
EOF
cat > "$src/Procfile" << 'EOF'
web: node server.js
EOF
cat > "$src/server.js" << 'EOF'
require('http').createServer((req,res)=>res.end('ok')).listen(process.env.PORT||5000);
EOF

RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# Confirm the app was deployed before destroying
assert_dir_exists "$RIKU_ROOT/apps/${APP}"
assert_file_exists "$RIKU_ROOT/workers-available/${APP}-web-1.toml"

# CLI syntax: riku destroy APP  (or: riku apps destroy APP — both route to cmd_destroy)
RIKU_ROOT="$RIKU_ROOT" riku destroy "$APP"

# Core directories must be removed
assert_dir_not_exists "$RIKU_ROOT/apps/${APP}"
assert_dir_not_exists "$RIKU_ROOT/envs/${APP}"

# Worker config must be gone
assert_file_not_exists "$RIKU_ROOT/workers-available/${APP}-web-1.toml"
assert_file_not_exists "$RIKU_ROOT/workers-enabled/${APP}-web-1.toml"

echo "App destroyed and cleanup verified successfully"
