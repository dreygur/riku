#!/bin/bash
# Quick deployment test for a single app
# Usage: ./tests/deploy/quick-test.sh <app-name> <runtime>

set -e

APP_NAME="${1:-test-app}"
RUNTIME="${2:-node}"
TEST_DIR="/tmp/riku-quick-test-$$"

echo "Creating test app: $APP_NAME ($RUNTIME)"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

case "$RUNTIME" in
    node)
        cat > package.json <<'EOF'
{"name":"test","version":"1.0.0","scripts":{"start":"node server.js"}}
EOF
        cat > server.js <<'EOF'
const http = require('http');
http.createServer((req, res) => {
  res.end(`Hello from ${process.env.PORT || 3000}\n`);
}).listen(process.env.PORT || 3000);
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
@app.route('/')
def hello(): return f"Port: {os.environ.get('PORT', 5000)}\n"
if __name__ == '__main__':
    app.run(host='0.0.0.0', port=int(os.environ.get('PORT', 5000)))
EOF
        ;;
    static)
        mkdir -p public
        echo "<h1>Static Test</h1>" > public/index.html
        ;;
esac

cat > Procfile <<EOF
web: $(if [ "$RUNTIME" = "static" ]; then echo "static: ."; else echo "$RUNTIME start"; fi)
EOF

cat > ENV <<EOF
RIKU_AUTO_RESTART=true
BIND_ADDRESS=127.0.0.1
NGINX_SERVER_NAME=${APP_NAME}.example.com
EOF

cat > SCALING <<EOF
web=1
EOF

echo ""
echo "Test app created at: $TEST_DIR"
echo ""
echo "To deploy:"
echo "  cd $TEST_DIR"
echo "  git init && git add . && git commit -m 'test'"
echo "  git remote add riku deploy@your-server:$APP_NAME"
echo "  git push riku main"
echo ""
echo "To cleanup:"
echo "  rm -rf $TEST_DIR"
