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
cat > "$src/package.json" << 'EOF'
{"name":"testapp","version":"1.0.0","private":true}
EOF

RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

# The router helper installs the site as root in /etc/riku/nginx/{app}.conf
SITE="/etc/riku/nginx/${APP}.conf"
assert_file_exists "$SITE"

# The config must reference the app name
assert_file_contains "$SITE" "$APP"

# The deploy user's tree holds no nginx config of its own
assert_file_not_exists "$RIKU_ROOT/nginx/${APP}.conf"

# The system nginx loads /etc/riku/nginx/*.conf, so its own check covers the site.
if nginx -t 2>&1; then
    echo "nginx config is syntactically valid"
else
    echo "ASSERTION FAILED: nginx -t rejected the config"
    echo "Config contents:"
    cat "$SITE"
    exit 1
fi

echo "Nginx config generated and validated successfully"
