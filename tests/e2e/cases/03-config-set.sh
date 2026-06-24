#!/usr/bin/env bash
# E2E test: set an environment variable and verify it appears in the ENV file.
# config set requires the app directory to exist (exit_if_invalid checks app_root).
# We deploy a minimal app first so the directory is present, then set a config key.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="configapp"
setup_app "$APP"

# Deploy a minimal app so the app directory is populated
src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/Procfile" << 'EOF'
web: node server.js
EOF
cat > "$src/server.js" << 'EOF'
require('http').createServer((req,res)=>res.end('ok')).listen(process.env.PORT||5000);
EOF

RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# Now set a config key via CLI.
# CLI syntax (from src/cli/cmds.rs): riku config set APP KEY=VALUE
RIKU_ROOT="$RIKU_ROOT" RIKU_SKIP_BUILD=1 riku config set "$APP" MYAPP_SECRET=hello

# The ENV file lives at $RIKU_ROOT/envs/$APP/ENV
assert_file_exists "$RIKU_ROOT/envs/${APP}/ENV"
assert_file_contains "$RIKU_ROOT/envs/${APP}/ENV" "MYAPP_SECRET=hello"

echo "Config set verified successfully"
