#!/usr/bin/env bash
# Builds and runs the browser dashboard (dashboard/) on this machine against
# the API that scripts/dashboard-tunnel.sh opens. Start the tunnel first.
#
#   scripts/dashboard-web.sh
#
# Listens on 127.0.0.1 only. It has no login unless
# RIKU_DASHBOARD_PASSWORD_HASH is set (see dashboard/README.md).
set -euo pipefail

source "$(dirname "$0")/lib/dashboard-token.sh"
cd "$(dirname "$0")/../dashboard"

API_PORT="${RIKU_DASHBOARD_PORT:-8088}"
WEB_PORT="${RIKU_DASHBOARD_WEB_PORT:-3100}"

if ! command -v nub >/dev/null; then
    echo "nub not found, install it with: npm install -g @nubjs/nub" >&2
    exit 1
fi

nub install --frozen-lockfile
nub run build

echo "Browser dashboard on http://127.0.0.1:$WEB_PORT"
RIKU_API_URL="http://127.0.0.1:$API_PORT" \
RIKU_DASHBOARD_TOKEN="$(dashboard_token)" \
    exec ./node_modules/.bin/next start -H 127.0.0.1 -p "$WEB_PORT"
