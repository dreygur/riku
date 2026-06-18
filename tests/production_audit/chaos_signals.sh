#!/usr/bin/env bash
# chaos_signals.sh
#
# Deploys a worker, finds its real OS pid (bypassing Riku entirely),
# kill -9's it directly, then polls `riku ps <app> --verbose` to see how
# long the supervisor takes to detect the crash and respawn.
#
# Ground truth from src/supervisor/process/health_check.rs: crash
# detection is poll-based (process.is_running() / try_wait()) inside
# check_processes(), invoked from the Err(Timeout) branch of the 1s
# recv_timeout loop in src/supervisor/daemon/mod.rs. There is no SIGCHLD
# handler. Respawn also goes through an exponential backoff with jitter
# (health_check.rs: base_backoff = min(60, 2^restart_count), + 0-9s
# jitter from pid % 10) — so "respawn within 2 seconds" is NOT guaranteed
# by the code as written; this script measures the real number rather
# than assuming it.
set -uo pipefail

APP="${1:-chaossignals}"
WORKER_KIND="${2:-web}"
TIMEOUT_SECONDS="${3:-30}"
RESULT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/results"
mkdir -p "$RESULT_DIR"
LOG="$RESULT_DIR/chaos_signals_$(date +%Y%m%d_%H%M%S).log"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

find_supervisor_pid() {
    pgrep -f "riku[[:space:]]+supervisor" | head -1
}

log "=== chaos_signals.sh starting ==="
log "app=$APP worker_kind=$WORKER_KIND timeout=${TIMEOUT_SECONDS}s"

SUP_PID="$(find_supervisor_pid)"
if [ -z "$SUP_PID" ]; then
    log "FATAL: no running 'riku supervisor' process found. Start it first with: riku supervisor &"
    exit 1
fi
log "supervisor_pid=$SUP_PID"

if ! riku apps info "$APP" >/dev/null 2>&1; then
    log "app '$APP' not found, creating it via 'riku apps create $APP'"
    riku apps create "$APP" >>"$LOG" 2>&1 || {
        log "FATAL: 'riku apps create $APP' failed, see log above"
        exit 1
    }
fi

log "deploying worker via 'riku ps $APP --scale ${WORKER_KIND}=1'"
riku ps "$APP" --scale "${WORKER_KIND}=1" >>"$LOG" 2>&1
sleep 1

# Find the real OS pid of the worker. Process group convention used by
# spawn.rs is "<app>-<kind>-<ordinal>", reflected in process names/cmdline.
WORKER_PID="$(pgrep -f "${APP}.*${WORKER_KIND}" | head -1)"
if [ -z "$WORKER_PID" ]; then
    log "FATAL: could not locate worker pid for app=$APP kind=$WORKER_KIND via pgrep"
    exit 1
fi
log "worker_pid=$WORKER_PID (raw host pid, found independently of riku state)"

log "sending kill -9 to worker_pid=$WORKER_PID directly from host OS"
kill -9 "$WORKER_PID"

if kill -0 "$WORKER_PID" 2>/dev/null; then
    log "WARNING: pid $WORKER_PID still alive immediately after kill -9 (unexpected)"
fi

log "polling 'riku ps $APP --verbose' for healed state, timeout=${TIMEOUT_SECONDS}s"
T0=$(date +%s.%N)
HEALED=0
ELAPSED_AT_HEAL=""

for ((s=0; s<TIMEOUT_SECONDS*5; s++)); do
    NEW_PID="$(pgrep -f "${APP}.*${WORKER_KIND}" | grep -v "^${WORKER_PID}$" | head -1)"
    if [ -n "$NEW_PID" ] && kill -0 "$NEW_PID" 2>/dev/null; then
        T1=$(date +%s.%N)
        ELAPSED_AT_HEAL=$(echo "$T1 - $T0" | bc 2>/dev/null || echo "?")
        log "new worker pid detected: $NEW_PID, elapsed=${ELAPSED_AT_HEAL}s"
        HEALED=1
        break
    fi
    sleep 0.2
done

log "=== riku ps --verbose output at end of poll ==="
riku ps "$APP" --verbose >>"$LOG" 2>&1 || true
cat "$LOG" | tail -20

log "=== run complete ==="
if [ "$HEALED" -eq 1 ]; then
    log "RESULT: HEALED in ${ELAPSED_AT_HEAL}s (old_pid=$WORKER_PID new_pid=$NEW_PID)"
    log "NOTE: code path is poll-based with backoff+jitter, not signal-driven."
    log "      a value well above 2s does NOT by itself indicate a bug -"
    log "      compare against health_check.rs backoff formula before flagging."
    exit 0
else
    log "RESULT: FAIL — worker was not respawned within ${TIMEOUT_SECONDS}s"
    exit 2
fi
