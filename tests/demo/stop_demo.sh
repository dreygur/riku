#!/usr/bin/env bash
# stop_demo.sh — tears down the container started by run_demo.sh.
# The SSH keypair under .keys/ is left in place so the next run_demo.sh
# doesn't need to re-provision it.
set -uo pipefail

CONTAINER_NAME="riku-demo-env-instance"

if command -v docker >/dev/null 2>&1; then
    DOCKER_BIN="docker"
elif command -v podman >/dev/null 2>&1; then
    DOCKER_BIN="podman"
else
    echo "FATAL: neither 'docker' nor 'podman' found on PATH" >&2
    exit 1
fi

if "$DOCKER_BIN" rm -f "$CONTAINER_NAME" >/dev/null 2>&1; then
    echo "stopped and removed $CONTAINER_NAME"
else
    echo "$CONTAINER_NAME was not running"
fi
