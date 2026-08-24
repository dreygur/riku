#!/usr/bin/env bash
# run_demo.sh: thin wrapper around compose.yml: builds the riku release
# binary (compose.yml can't do this itself: see its header comment), then
# `docker compose up --build`, replacing any previous instance.
#
# Prefer running compose.yml directly if you don't need this convenience:
#   cargo build --release
#   docker compose -f tests/demo/compose.yml up --build
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if command -v docker >/dev/null 2>&1; then
    COMPOSE=(docker compose)
elif command -v podman >/dev/null 2>&1; then
    COMPOSE=(podman compose)
else
    echo "FATAL: neither 'docker' nor 'podman' found on PATH" >&2
    exit 1
fi

echo "--- step 1: building riku release binary ---"
(cd "$REPO_ROOT" && cargo build --release)
if [ ! -x "$REPO_ROOT/target/release/riku" ]; then
    echo "FATAL: $REPO_ROOT/target/release/riku not found after build" >&2
    exit 1
fi

echo "--- step 2: docker compose up --build ---"
"${COMPOSE[@]}" -f "$SCRIPT_DIR/compose.yml" up --build -d

cat <<EOF

=====================================================================
riku demo environment is up.

*.localhost addresses resolve to 127.0.0.1 on their own on virtually
every modern OS/browser: no /etc/hosts edit needed. Open:

  Dashboard:     http://dashboard.localhost:8080
  hello-node:    http://hello-node.localhost:8080
  hello-python:  http://hello-python.localhost:8080
  hello-ruby:    http://hello-ruby.localhost:8080
  hello-go:      http://hello-go.localhost:8080
  hello-worker:  http://hello-worker.localhost:8080

Follow logs:  ${COMPOSE[*]} -f "$SCRIPT_DIR/compose.yml" logs -f
Run stop_demo.sh (or "${COMPOSE[*]} -f "$SCRIPT_DIR/compose.yml" down") when you're done.
=====================================================================
EOF
