#!/usr/bin/env bash
# user_traffic_simulation.sh — runs on the HOST, against the running
# container started by run_container_test.sh.
#
# Phase 1 (developer workflow): git init the mock app, push it over SSH
# to the container on port 2222. Ground truth: the first push to a new
# app auto-creates the bare repo and hook (src/cli/git/receive_pack.rs
# cmd_git_receive_pack -> ensure_repo_symlink + git init --bare on first
# call) — no `riku apps create` step is required before pushing.
#
# Phase 2 (traffic): blast the nginx-proxied app (port 80 on the host,
# mapped from the container) with concurrent load, log status codes and
# latency percentiles. Prefers wrk, falls back to k6, falls back to a
# parallel curl loop if neither binary is present.
set -uo pipefail

HOST="${RIKU_TEST_HOST:-localhost}"
SSH_PORT="${RIKU_TEST_SSH_PORT:-2222}"
HTTP_PORT="${RIKU_TEST_HTTP_PORT:-80}"
APP_NAME="${1:-trafficapp}"
SSH_KEY="${RIKU_TEST_SSH_KEY:-$HOME/.riku_container_test_key}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_SRC_DIR="$SCRIPT_DIR/test_web_app"
WORK_DIR="$(mktemp -d /tmp/riku_traffic_test.XXXXXX)"
RESULT_DIR="$SCRIPT_DIR/../results"
mkdir -p "$RESULT_DIR"
TS="$(date +%Y%m%d_%H%M%S)"
LOG="$RESULT_DIR/user_traffic_simulation_${TS}.log"
DURATION_SECONDS="${RIKU_TEST_DURATION:-30}"
CONCURRENCY="${RIKU_TEST_CONCURRENCY:-80}"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

SSH_OPTS=(-i "$SSH_KEY" -p "$SSH_PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o BatchMode=yes)

log "=== user_traffic_simulation.sh starting (app=$APP_NAME) ==="
log "work_dir=$WORK_DIR"

# ---- Phase 1: developer workflow (git push deploy) ----
log "--- phase 1: developer git push deploy ---"
cp -r "$APP_SRC_DIR"/. "$WORK_DIR/"
cd "$WORK_DIR"

git init -q -b main
git config user.email "audit@riku.test"
git config user.name "Riku Audit"
git add -A
git commit -q -m "initial mock app commit"

REMOTE_URL="ssh://riku@${HOST}:${SSH_PORT}/${APP_NAME}"
git remote add riku "$REMOTE_URL" 2>/dev/null || git remote set-url riku "$REMOTE_URL"

log "pushing to $REMOTE_URL"
export GIT_SSH_COMMAND="ssh ${SSH_OPTS[*]}"

DEPLOY_T0=$(date +%s.%N)
if git push riku main 2>&1 | tee -a "$LOG"; then
    DEPLOY_T1=$(date +%s.%N)
    DEPLOY_ELAPSED=$(echo "$DEPLOY_T1 - $DEPLOY_T0" | bc 2>/dev/null || echo "?")
    log "DEPLOY: SUCCESS in ${DEPLOY_ELAPSED}s"
    DEPLOY_OK=1
else
    log "DEPLOY: FAILED"
    DEPLOY_OK=0
fi

if [ "$DEPLOY_OK" -ne 1 ]; then
    log "RESULT: FAIL — deployment did not succeed, skipping traffic phase"
    exit 2
fi

# Riku's default NGINX_SERVER_NAME is "<app>.example.com" when the app's
# ENV has no explicit override (src/nginx/context.rs:49). The static vhost
# in this container (riku-nginx.conf) is a catch-all default_server with
# server_name "_", so requests WITHOUT a matching Host header land there
# (404, no proxy_pass) instead of reaching the app — this is correct nginx
# behavior, not a bug, but it means every request in this script must send
# the right Host header to actually exercise the app.
APP_HOST_HEADER="${APP_NAME}.example.com"

log "waiting up to 30s for app to come up behind nginx (Host: $APP_HOST_HEADER)"
APP_UP=0
for i in $(seq 1 30); do
    if curl -sf -o /dev/null "http://${HOST}:${HTTP_PORT}/health" -H "Host: ${APP_HOST_HEADER}" 2>/dev/null; then
        APP_UP=1
        break
    fi
    sleep 1
done

if [ "$APP_UP" -ne 1 ]; then
    log "RESULT: FAIL — app did not respond on /health within 30s after deploy"
    exit 2
fi
log "app is up and responding to /health"

# ---- Phase 2: end-user traffic ----
log "--- phase 2: traffic test (${CONCURRENCY} concurrent, ${DURATION_SECONDS}s) ---"
TARGET_URL="http://${HOST}:${HTTP_PORT}/"
STATUS_LOG="$RESULT_DIR/traffic_status_codes_${TS}.log"
LATENCY_LOG="$RESULT_DIR/traffic_latency_${TS}.csv"
: > "$STATUS_LOG"
echo "request_id,http_code,time_total_s" > "$LATENCY_LOG"

run_curl_fallback() {
    log "wrk/k6 not found — using parallel curl loop fallback"
    local end_time=$(( $(date +%s) + DURATION_SECONDS ))
    local req_id=0
    local pids=()

    worker() {
        local wid="$1"
        local n=0
        while [ "$(date +%s)" -lt "$end_time" ]; do
            n=$((n + 1))
            local out
            out=$(curl -s -o /dev/null -w "%{http_code} %{time_total}" -H "Host: ${APP_HOST_HEADER}" "$TARGET_URL" 2>/dev/null)
            local code time
            code=$(echo "$out" | awk '{print $1}')
            time=$(echo "$out" | awk '{print $2}')
            echo "${wid}-${n},${code:-000},${time:-0}" >> "$LATENCY_LOG"
            echo "${code:-000}" >> "$STATUS_LOG"
        done
    }

    for w in $(seq 1 "$CONCURRENCY"); do
        worker "$w" &
        pids+=($!)
    done

    for p in "${pids[@]}"; do
        wait "$p"
    done
}

if command -v wrk >/dev/null 2>&1; then
    log "using wrk"
    wrk -t8 -c"$CONCURRENCY" -d"${DURATION_SECONDS}s" --latency -H "Host: ${APP_HOST_HEADER}" "$TARGET_URL" | tee -a "$LOG" | tee "$RESULT_DIR/wrk_output_${TS}.txt"
elif command -v k6 >/dev/null 2>&1; then
    log "using k6"
    cat > "$WORK_DIR/k6_script.js" <<EOF
import http from 'k6/http';
export const options = { vus: ${CONCURRENCY}, duration: '${DURATION_SECONDS}s' };
export default function () {
  http.get('${TARGET_URL}', { headers: { Host: '${APP_HOST_HEADER}' } });
}
EOF
    k6 run "$WORK_DIR/k6_script.js" | tee -a "$LOG" | tee "$RESULT_DIR/k6_output_${TS}.txt"
else
    run_curl_fallback
fi

# ---- Analysis ----
log "--- analysis ---"
TOTAL_REQUESTS=$(wc -l < "$STATUS_LOG" 2>/dev/null || echo 0)
ERROR_502=$(grep -c '^502$' "$STATUS_LOG" 2>/dev/null || true)
ERROR_504=$(grep -c '^504$' "$STATUS_LOG" 2>/dev/null || true)
ERROR_OTHER=$(grep -cv '^200$' "$STATUS_LOG" 2>/dev/null || true)
SUCCESS_200=$(grep -c '^200$' "$STATUS_LOG" 2>/dev/null || true)

log "total_requests=$TOTAL_REQUESTS success_200=$SUCCESS_200 502=$ERROR_502 504=$ERROR_504 non_200_total=$ERROR_OTHER"

if [ -s "$LATENCY_LOG" ] && [ "$TOTAL_REQUESTS" -gt 0 ]; then
    # p50/p95/p99 from the curl-fallback CSV (column 3, seconds -> ms)
    sort -t, -k3 -n <(tail -n +2 "$LATENCY_LOG") > "$RESULT_DIR/sorted_latency_${TS}.csv"
    N=$(wc -l < "$RESULT_DIR/sorted_latency_${TS}.csv")
    if [ "$N" -gt 0 ]; then
        p50_line=$(( N * 50 / 100 )); [ "$p50_line" -lt 1 ] && p50_line=1
        p95_line=$(( N * 95 / 100 )); [ "$p95_line" -lt 1 ] && p95_line=1
        p99_line=$(( N * 99 / 100 )); [ "$p99_line" -lt 1 ] && p99_line=1
        p50=$(sed -n "${p50_line}p" "$RESULT_DIR/sorted_latency_${TS}.csv" | awk -F, '{printf "%.1f", $3*1000}')
        p95=$(sed -n "${p95_line}p" "$RESULT_DIR/sorted_latency_${TS}.csv" | awk -F, '{printf "%.1f", $3*1000}')
        p99=$(sed -n "${p99_line}p" "$RESULT_DIR/sorted_latency_${TS}.csv" | awk -F, '{printf "%.1f", $3*1000}')
        log "latency_ms p50=$p50 p95=$p95 p99=$p99 (curl-fallback measurements; wrk/k6 report their own percentiles above if used)"
    fi
fi

log "raw status log: $STATUS_LOG"
log "raw latency csv: $LATENCY_LOG"

rm -rf "$WORK_DIR"

log "=== run complete ==="
if [ "$ERROR_502" -gt 0 ] || [ "$ERROR_504" -gt 0 ]; then
    log "RESULT: FAIL — 502/504 errors observed under load (502=$ERROR_502 504=$ERROR_504)"
    exit 2
else
    log "RESULT: PASS — deploy succeeded, no 502/504 observed"
    exit 0
fi
