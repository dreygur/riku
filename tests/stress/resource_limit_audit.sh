#!/usr/bin/env bash
# resource_limit_audit.sh
#
# Deploys bad_tenant_app under the supervisor-wide RIKU_MAX_* limits
# (see bad_tenant_app/start-supervisor.env) and confirms whether
# setrlimit enforcement in src/supervisor/resource_limits/mod.rs actually
# bounds the runaway process, or whether it survives / takes the host
# down with it. No cgroups are used anywhere in src/ (confirmed by grep)
# — this script does not pretend otherwise.
set -uo pipefail

APP="${1:-badtenant}"
MODE="${2:-mem}"   # mem | cpu
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULT_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULT_DIR"
LOG="$RESULT_DIR/resource_limit_audit_${MODE}_$(date +%Y%m%d_%H%M%S).log"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

find_supervisor_pid() {
    pgrep -f "riku[[:space:]]+supervisor" | head -1
}

log "=== resource_limit_audit.sh starting (mode=$MODE) ==="

SUP_PID="$(find_supervisor_pid)"
if [ -z "$SUP_PID" ]; then
    log "FATAL: no running 'riku supervisor' process found."
    log "Start it with the documented limits first:"
    log "  set -a; source $SCRIPT_DIR/bad_tenant_app/start-supervisor.env; set +a; riku supervisor &"
    exit 1
fi
log "supervisor_pid=$SUP_PID"

if ! riku apps info "$APP" >/dev/null 2>&1; then
    log "app '$APP' not found, creating it via 'riku apps create $APP'"
    riku apps create "$APP" >>"$LOG" 2>&1 || {
        log "FATAL: 'riku apps create $APP' failed"
        exit 1
    }
fi

WORKER_KIND="web"
[ "$MODE" = "cpu" ] && WORKER_KIND="worker"

log "scaling ${WORKER_KIND}=1 for app=$APP (mode=$MODE)"
riku ps "$APP" --scale "${WORKER_KIND}=1" >>"$LOG" 2>&1
sleep 1

WPID="$(pgrep -f "${APP}.*${WORKER_KIND}" | head -1)"
if [ -z "$WPID" ]; then
    log "FATAL: could not find worker pid for app=$APP kind=$WORKER_KIND"
    exit 1
fi
log "tenant_worker_pid=$WPID"

log "=== monitoring for up to 30s ==="
START=$(date +%s)
KILLED_BY_LIMIT=0
while [ $(( $(date +%s) - START )) -lt 30 ]; do
    if ! kill -0 "$WPID" 2>/dev/null; then
        log "tenant process $WPID terminated at t+$(( $(date +%s) - START ))s"
        KILLED_BY_LIMIT=1
        break
    fi
    VMSIZE="$(awk '/VmSize/{print $2}' "/proc/$WPID/status" 2>/dev/null || echo '?')"
    CPUTIME="$(awk '{print $14, $15}' "/proc/$WPID/stat" 2>/dev/null || echo '?')"
    log "t+$(( $(date +%s) - START ))s vmsize_kb=$VMSIZE utime_stime_ticks=$CPUTIME"
    sleep 2
done

log "=== dmesg tail (look for oom-killer activity) ==="
dmesg 2>/dev/null | tail -10 | tee -a "$LOG" || log "(dmesg not accessible without sudo)"

log "=== sibling-starvation check ==="
log "if a known-good app is also running, compare its scheduling latency now:"
log "  mpstat -P ALL 1 5"
log "RLIMIT_CPU is a TOTAL CONSUMPTION cap, not a fair-share scheduler —"
log "a cpu-spin tenant CAN peg one core for the full duration before SIGXCPU fires."

log "=== cleanup: killing tenant worker if still alive ==="
kill -9 "$WPID" 2>/dev/null || true
riku ps "$APP" --scale "${WORKER_KIND}=0" >>"$LOG" 2>&1 || true

log "=== run complete ==="
if [ "$KILLED_BY_LIMIT" -eq 1 ]; then
    log "RESULT: limit enforcement terminated the tenant (verify cause in dmesg/log above"
    log "        before crediting 'graceful' enforcement vs OOM-killer vs SIGXCPU)"
    exit 0
else
    log "RESULT: FAIL — tenant survived the full monitoring window unbounded"
    exit 2
fi
