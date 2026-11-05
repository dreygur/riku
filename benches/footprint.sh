#!/usr/bin/env bash
# Prints the numbers the rootless plan's footprint ledger holds: binary bytes
# and idle RSS in KiB. Starts its own supervisor on an empty temp root and the
# distribution's nginx with its stock config, so run it as root in a throwaway
# container or on a host where nginx is not running.
#
# Usage: benches/footprint.sh <riku binary>

set -euo pipefail

SETTLE_SECONDS=60
HEALTH_PORT=19091
NGINX_PID_FILE=/run/nginx.pid

riku=${1:?usage: footprint.sh <riku binary>}

rss_kib() {
  ps -o rss= -p "$1" | tr -d ' '
}

nginx_rss_kib() {
  local master=$1
  { ps -o rss= -p "$master"; ps -o rss= --ppid "$master"; } |
    awk '{ total += $1 } END { print total }'
}

cleanup() {
  kill "${supervisor:-}" 2>/dev/null || true
  wait "${supervisor:-}" 2>/dev/null || true
  nginx -s quit 2>/dev/null || true
  rm -rf "$root"
}

if pgrep -x nginx >/dev/null; then
  echo "nginx is already running, its RSS would be counted" >&2
  exit 1
fi

root=$(mktemp -d)
trap cleanup EXIT

mkdir "$root/workers-enabled"
RIKU_ROOT=$root RIKU_HEALTH_PORT=$HEALTH_PORT "$riku" supervisor >"$root/supervisor.log" 2>&1 &
supervisor=$!
nginx

sleep "$SETTLE_SECONDS"

if ! kill -0 "$supervisor" 2>/dev/null; then
  echo "supervisor exited before the measurement:" >&2
  tail -n 5 "$root/supervisor.log" >&2
  exit 1
fi

master=$(<"$NGINX_PID_FILE")
supervisor_kib=$(rss_kib "$supervisor")
nginx_kib=$(nginx_rss_kib "$master")

echo "binary_bytes $(stat -c %s "$riku")"
echo "supervisor_kib $supervisor_kib"
echo "supervisor_nginx_kib $((supervisor_kib + nginx_kib))"
echo "nginx_workers $(ps -o pid= --ppid "$master" | wc -l | tr -d ' ')"
echo "nginx_version $(nginx -v 2>&1 | sed 's/^nginx version: //')"
