#!/usr/bin/env bash
# E2E test: after deploying an app, verify an nginx config is generated and passes
# nginx -t syntax validation.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="nginxapp"
setup_app "$APP"

src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/Procfile" << 'EOF'
web: node server.js
EOF
cat > "$src/server.js" << 'EOF'
require('http').createServer((req,res)=>res.end('ok')).listen(process.env.PORT||5000);
EOF

RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# Riku writes nginx configs to $RIKU_ROOT/nginx/{app}.conf
assert_file_exists "$RIKU_ROOT/nginx/${APP}.conf"

# The config must reference the app name
assert_file_contains "$RIKU_ROOT/nginx/${APP}.conf" "$APP"

# Validate the generated config with nginx -t by creating a minimal wrapper
# nginx -t only tests the main config; we test our snippet as an included file.
NGINX_TEST_CONF=$(mktemp /tmp/nginx-test-XXXXXX.conf)
trap 'rm -f "$NGINX_TEST_CONF"' EXIT

cat > "$NGINX_TEST_CONF" << NGINXCONF
events {}
http {
    include $RIKU_ROOT/nginx/${APP}.conf;
}
NGINXCONF

if nginx -t -c "$NGINX_TEST_CONF" 2>&1; then
    echo "nginx config is syntactically valid"
else
    echo "ASSERTION FAILED: nginx -t rejected the generated config"
    echo "Config contents:"
    cat "$RIKU_ROOT/nginx/${APP}.conf"
    exit 1
fi

echo "Nginx config generated and validated successfully"
