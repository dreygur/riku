#!/usr/bin/env bash
# E2E test: deploy a minimal Python application and verify worker config is created.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

APP="pythonapp"
setup_app "$APP"

src=$(mktemp -d)
trap 'rm -rf "$src"' EXIT

cat > "$src/requirements.txt" << 'EOF'
gunicorn
flask
EOF

cat > "$src/Procfile" << 'EOF'
web: gunicorn app:application
EOF

cat > "$src/app.py" << 'EOF'
from flask import Flask
application = Flask(__name__)

@application.route('/')
def index():
    return 'ok'
EOF

# Skip pip install — only care about config generation
RIKU_SKIP_BUILD=1 push_app "$APP" "$src"

assert_file_exists "$RIKU_ROOT/workers-available/${APP}-web-1.toml"
assert_file_exists "$RIKU_ROOT/workers-enabled/${APP}-web-1.toml"

assert_file_contains "$RIKU_ROOT/workers-available/${APP}-web-1.toml" "gunicorn"

echo "Python app deployed successfully"
