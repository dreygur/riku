#!/usr/bin/env bash
# Runs the embedded dashboard (the API the browser dashboard talks to) on a
# riku server as the deploy user, tunneled to this machine, for as long as
# this terminal stays open. Ctrl-C stops both.
#
#   scripts/dashboard-tunnel.sh dreygur@solutech.local
#
# Log in as a user who can sudo: riku's own deploy keys allow no port
# forwarding. The dashboard gets the token as an argument, so other users
# on the server can see it in ps while it runs.
set -euo pipefail

source "$(dirname "$0")/lib/dashboard-token.sh"

TARGET="${1:?usage: $0 <user@host>}"
DEPLOY_USER="${RIKU_USER:-riku}"
PORT="${RIKU_DASHBOARD_PORT:-8088}"
TOKEN="$(dashboard_token)"

# -t keeps a terminal on the server, so closing this one stops the
# dashboard there instead of leaving it holding the port.
remote=$(printf '%q ' sudo -u "$DEPLOY_USER" -H sh -c \
    'exec "$HOME/.local/bin/riku" dashboard --bind "$1" --token "$2"' \
    sh "127.0.0.1:$PORT" "$TOKEN")

echo "Dashboard API on http://127.0.0.1:$PORT, start the web UI with scripts/dashboard-web.sh"
exec ssh -t -o ExitOnForwardFailure=yes -L "$PORT:127.0.0.1:$PORT" "$TARGET" "$remote"
