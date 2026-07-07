#!/usr/bin/env bash
# stop_demo.sh — tears down the container started by run_demo.sh / compose.yml.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if command -v docker >/dev/null 2>&1; then
    COMPOSE=(docker compose)
elif command -v podman >/dev/null 2>&1; then
    COMPOSE=(podman compose)
else
    echo "FATAL: neither 'docker' nor 'podman' found on PATH" >&2
    exit 1
fi

"${COMPOSE[@]}" -f "$SCRIPT_DIR/compose.yml" down
