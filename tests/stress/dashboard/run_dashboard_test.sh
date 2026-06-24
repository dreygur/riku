#!/usr/bin/env bash
# run_dashboard_test.sh — orchestrates a production-fidelity audit of the
# Next.js/Hono dashboard: builds the real riku binary, runs a real
# `riku supervisor` against a sandboxed RIKU_ROOT (not the host's real
# ~/.riku — unlike the other scripts in this suite, the dashboard doesn't
# need to touch host state to be tested honestly, so it doesn't), deploys
# the suite's existing `container/test_web_app` as a real worker process,
# builds and starts the dashboard in production mode (`next build && next
# start`), then runs `dashboard/scripts/audit-dashboard.ts` against the
# live stack end to end (REST handlers, the metrics SSE stream, the
# deploy-log SSE tail).
#
# Ground truth baked in (verified against source before writing):
# - The dashboard's Hono routes read RIKU_API_URL from process.env at
#   request time (src/...dashboard/server/routers/supervisor.ts), not at
#   build time — so one `next build` can be reused against any sandboxed
#   supervisor port without rebuilding.
# - Worker config filenames are <app>-<kind>-<ordinal>.toml under
#   workers-enabled/ (src/supervisor/daemon/config_watcher.rs) — this
#   script writes one directly, same mechanism `riku ps --scale` uses
#   under the hood, to avoid depending on a full git-push deploy.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DASHBOARD_DIR="$REPO_ROOT/dashboard"
RESULT_DIR="$SCRIPT_DIR/../results"
mkdir -p "$RESULT_DIR"
TS="$(date +%Y%m%d_%H%M%S)"
LOG="$RESULT_DIR/run_dashboard_test_${TS}.log"

APP_NAME="${RIKU_TEST_APP_NAME:-dashboardaudit}"
HEALTH_PORT="${RIKU_TEST_HEALTH_PORT:-19091}"
WORKER_PORT="${RIKU_TEST_WORKER_PORT:-19080}"
DASHBOARD_PORT="${RIKU_TEST_DASHBOARD_PORT:-3100}"
MAX_PROCESSES="${RIKU_TEST_MAX_PROCESSES:-64}"
SANDBOX_ROOT="$(mktemp -d /tmp/riku_dashboard_audit.XXXXXX)"

SUPERVISOR_PID=""
DASHBOARD_PID=""

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

cleanup() {
    log "=== cleanup ==="
    [ -n "$DASHBOARD_PID" ] && kill "$DASHBOARD_PID" 2>/dev/null
    [ -n "$SUPERVISOR_PID" ] && kill "$SUPERVISOR_PID" 2>/dev/null
    sleep 1
    [ -n "$DASHBOARD_PID" ] && kill -9 "$DASHBOARD_PID" 2>/dev/null
    [ -n "$SUPERVISOR_PID" ] && kill -9 "$SUPERVISOR_PID" 2>/dev/null
    log "sandbox root left for postmortem: $SANDBOX_ROOT"
}
trap cleanup EXIT

log "=== run_dashboard_test.sh starting ==="
log "repo_root=$REPO_ROOT sandbox_root=$SANDBOX_ROOT health_port=$HEALTH_PORT worker_port=$WORKER_PORT dashboard_port=$DASHBOARD_PORT"

# ---- Step 1: build the real riku release binary ----
log "--- step 1: building riku release binary ---"
(cd "$REPO_ROOT" && cargo build --release) 2>&1 | tee -a "$LOG"
RIKU_BIN="$REPO_ROOT/target/release/riku"
if [ ! -x "$RIKU_BIN" ]; then
    log "FATAL: $RIKU_BIN not found after build"
    exit 1
fi
log "binary ready: $RIKU_BIN"

# ---- Step 2: lay out the sandbox RIKU_ROOT and a real worker config ----
log "--- step 2: provisioning sandbox RIKU_ROOT ---"
mkdir -p "$SANDBOX_ROOT/logs/$APP_NAME" "$SANDBOX_ROOT/envs/$APP_NAME" "$SANDBOX_ROOT/workers-enabled"

WORKER_APP_DIR="$SCRIPT_DIR/../container/test_web_app"
if [ ! -f "$WORKER_APP_DIR/app.py" ]; then
    log "FATAL: expected worker app at $WORKER_APP_DIR/app.py (reusing container/test_web_app)"
    exit 1
fi

cat > "$SANDBOX_ROOT/workers-enabled/${APP_NAME}-web-1.toml" <<EOF
[worker]
app = "$APP_NAME"
kind = "web"
command = "python3 $WORKER_APP_DIR/app.py"
ordinal = 1

[env]
PORT = "$WORKER_PORT"

[options]
working_dir = "$WORKER_APP_DIR"
log_file = "$SANDBOX_ROOT/logs/$APP_NAME/web.log"
timeout = 30
grace_period = 2
max_restarts = 3

[options.health_check]
url = "/health"
interval = 30
timeout = 2
retries = 3
EOF
log "worker config written: $SANDBOX_ROOT/workers-enabled/${APP_NAME}-web-1.toml"

# ---- Step 3: start the real supervisor against the sandbox ----
log "--- step 3: starting riku supervisor (sandboxed RIKU_ROOT) ---"
RIKU_ROOT="$SANDBOX_ROOT" \
RIKU_HEALTH_PORT="$HEALTH_PORT" \
RIKU_MAX_PROCESSES="$MAX_PROCESSES" \
RUST_LOG=info \
    "$RIKU_BIN" supervisor > "$SANDBOX_ROOT/supervisor.log" 2>&1 &
SUPERVISOR_PID=$!
log "supervisor_pid=$SUPERVISOR_PID"

log "waiting for health server on 127.0.0.1:$HEALTH_PORT"
SUPERVISOR_READY=0
for i in $(seq 1 30); do
    if curl -fsS "http://127.0.0.1:${HEALTH_PORT}/health" >/dev/null 2>&1; then
        SUPERVISOR_READY=1
        break
    fi
    sleep 1
done
if [ "$SUPERVISOR_READY" -ne 1 ]; then
    log "FATAL: supervisor health endpoint never came up"
    cat "$SANDBOX_ROOT/supervisor.log" | tee -a "$LOG"
    exit 1
fi
log "supervisor is up"

log "waiting for the worker process to report healthy via /metrics"
WORKER_HEALTHY=0
for i in $(seq 1 30); do
    if curl -fsS "http://127.0.0.1:${HEALTH_PORT}/metrics" 2>/dev/null | grep -q '"health_check_status":"Healthy"'; then
        WORKER_HEALTHY=1
        break
    fi
    sleep 1
done
log "worker_healthy=$WORKER_HEALTHY"

# ---- Step 4: build and start the dashboard in production mode ----
log "--- step 4: building dashboard (next build) ---"
(cd "$DASHBOARD_DIR" && npx --yes next build) 2>&1 | tee -a "$LOG"

log "--- step 4b: starting dashboard (next start -p $DASHBOARD_PORT) ---"
# Invoke the local `next` binary directly (not via `npx`, which forks an
# indirection process of its own) so $! below is the actual server PID —
# needed for the alive/zombie checks in step 6 to mean anything.
RIKU_API_URL="http://127.0.0.1:${HEALTH_PORT}" \
    "$DASHBOARD_DIR/node_modules/.bin/next" start "$DASHBOARD_DIR" -p "$DASHBOARD_PORT" \
    > "$SANDBOX_ROOT/dashboard.log" 2>&1 &
DASHBOARD_PID=$!
log "dashboard_pid=$DASHBOARD_PID"

log "waiting for dashboard on 127.0.0.1:$DASHBOARD_PORT"
DASHBOARD_READY=0
for i in $(seq 1 30); do
    if curl -fsS "http://127.0.0.1:${DASHBOARD_PORT}/api/health" >/dev/null 2>&1; then
        DASHBOARD_READY=1
        break
    fi
    sleep 1
done
if [ "$DASHBOARD_READY" -ne 1 ]; then
    log "FATAL: dashboard never came up"
    cat "$SANDBOX_ROOT/dashboard.log" | tee -a "$LOG"
    exit 1
fi
log "dashboard is up"

# ---- Step 5: run the real audit script against the live stack ----
log "--- step 5: running scripts/audit-dashboard.ts ---"
AUDIT_LOG="$RESULT_DIR/audit_dashboard_${TS}.log"
DASHBOARD_URL="http://127.0.0.1:${DASHBOARD_PORT}" \
RIKU_API_URL="http://127.0.0.1:${HEALTH_PORT}" \
AUDIT_APP="$APP_NAME" \
    node "$DASHBOARD_DIR/scripts/audit-dashboard.ts" > "$AUDIT_LOG" 2>&1
AUDIT_EXIT=$?
cat "$AUDIT_LOG" | tee -a "$LOG"
log "audit-dashboard.ts exited with code $AUDIT_EXIT (full output: $AUDIT_LOG)"

# ---- Step 6: collect logs + zombie check before teardown ----
log "--- step 6: collecting logs ---"
cp "$SANDBOX_ROOT/supervisor.log" "$RESULT_DIR/supervisor_${TS}.log" 2>/dev/null
cp "$SANDBOX_ROOT/dashboard.log" "$RESULT_DIR/dashboard_${TS}.log" 2>/dev/null
cp "$SANDBOX_ROOT/logs/$APP_NAME/web.log" "$RESULT_DIR/worker_${TS}.log" 2>/dev/null

# Scoped to direct descendants of the supervisor we started (same
# convention as stress_lifecycle.sh) — a system-wide zombie grep would
# pick up unrelated noise from whatever else is running on this host.
ZOMBIES_FOUND="$(ps -eo state,pid,ppid 2>/dev/null | awk -v root="$SUPERVISOR_PID" '$1 ~ /^Z/ && $3 == root {print $2}' | wc -l)"
SUPERVISOR_ALIVE="no"
kill -0 "$SUPERVISOR_PID" 2>/dev/null && SUPERVISOR_ALIVE="yes"
DASHBOARD_ALIVE="no"
kill -0 "$DASHBOARD_PID" 2>/dev/null && DASHBOARD_ALIVE="yes"

# ---- Step 7: structured verdict ----
log "--- step 7: verdict ---"
VERDICT_FILE="$RESULT_DIR/verdict_${TS}.txt"
{
    echo "==================================================================="
    echo "RIKU DASHBOARD AUDIT — VERDICT"
    echo "==================================================================="
    echo "timestamp:               $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "app:                     $APP_NAME"
    echo "health_port:             $HEALTH_PORT"
    echo "dashboard_port:          $DASHBOARD_PORT"
    echo
    echo "worker_reported_healthy: $WORKER_HEALTHY  (1 = saw Healthy in /metrics before audit ran)"
    echo "audit_dashboard_exit:    $AUDIT_EXIT  (0 = pass, see $AUDIT_LOG for the full terminal render)"
    echo "supervisor_alive_at_end: $SUPERVISOR_ALIVE"
    echo "dashboard_alive_at_end:  $DASHBOARD_ALIVE"
    echo "zombie_processes_found:  $ZOMBIES_FOUND"
    echo
    if [ "$AUDIT_EXIT" -eq 0 ] && [ "$SUPERVISOR_ALIVE" = "yes" ] \
        && [ "$DASHBOARD_ALIVE" = "yes" ] && [ "$ZOMBIES_FOUND" -eq 0 ]; then
        echo "OVERALL: PASS — REST handlers, metrics SSE, and log-tail SSE all verified live; no zombies."
    else
        echo "OVERALL: FAIL — see component results above for which check failed."
    fi
    echo "==================================================================="
} | tee "$VERDICT_FILE" | tee -a "$LOG"

log "verdict written to $VERDICT_FILE"
log "full log at $LOG"
log "=== run_dashboard_test.sh complete ==="

[ "$AUDIT_EXIT" -eq 0 ] && [ "$SUPERVISOR_ALIVE" = "yes" ] && [ "$DASHBOARD_ALIVE" = "yes" ] && [ "$ZOMBIES_FOUND" -eq 0 ]
exit $?
