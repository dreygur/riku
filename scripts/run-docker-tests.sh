#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Use docker if available, fall back to podman
DOCKER=$(command -v docker 2>/dev/null || command -v podman 2>/dev/null || true)
if [ -z "$DOCKER" ]; then
    echo "Error: neither docker nor podman found in PATH"
    exit 1
fi
echo "Using container runtime: $DOCKER"

echo "Building E2E test image..."
"$DOCKER" build -f docker/Dockerfile.test -t riku-e2e-test .

echo "Running E2E tests..."
"$DOCKER" run --rm riku-e2e-test
