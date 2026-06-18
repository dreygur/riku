#!/usr/bin/env bash
# stress_lifecycle.sh
#
# Rapid worker scale-up/scale-down stress test against a real Riku
# supervisor instance. Drives the actual CLI (`riku ps <app> --scale ...`,
# `src/cli/cli.rs` Ps command) 1 -> 20 -> 1 workers, 100 cycles, while a
# background watcher polls for zombie (Z state) descendants of the
# supervisor process.
#
# Riku's reaping is poll-based: src/supervisor/process/health_check.rs
# calls try_wait() inside check_processes(), which only runs from the
# Err(RecvTimeoutError::Timeout) branch of the 1s daemon tick in
# src/supervisor/daemon/mod.rs. There is no SIGCHLD handler anywhere in
# src/supervisor/ (verified: grep -rn SIGCHLD src/ returns nothing).
# This script exists to find out whether that poll-based reaping keeps up
# under rapid scale churn, or whether zombies accumulate.
set -uo pipefail

APP="${1:-stresslifecycle}"
CYCLES="${2:-100}"
WORKER_KIND="${3:-web}"
RESULT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/results"
mkdir -p "$RESULT_DIR"
LOG="$RESULT_DIR/stress_lifecycle_$(date +%Y%m%d_%H%M%S).log"
ZOMBIE_LOG="$RESULT_DIR/zombie_events.log"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

find_supervisor_pid() {
    pgrep -f "riku[[:space:]]+supervisor" | head -1
}

zombie_descendants_of() {
    local root_pid="$1"
    # state,pid,ppid columns; filter zombie state and ppid == root_pid
    ps -eo state,pid,ppid 2>/dev/null | awk -v root="$root_pid" '$1 ~ /^Z/ && $3 == root {print $2}'
}

log "=== stress_lifecycle.sh starting ==="
log "app=$APP cycles=$CYCLES worker_kind=$WORKER_KIND"

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

BASELINE_Z="$(zombie_descendants_of "$SUP_PID" | wc -l)"
log "baseline_zombie_count=$BASELINE_Z"

# Background zombie watcher: samples every 200ms for the whole run,
# appends any pid seen in Z state to ZOMBIE_LOG (deduplicated per run).
(
    : > "$ZOMBIE_LOG"
    while kill -0 "$SUP_PID" 2>/dev/null; do
        zs="$(zombie_descendants_of "$SUP_PID")"
        if [ -n "$zs" ]; then
            for z in $zs; do
                echo "$(date '+%H:%M:%S.%N') zombie_pid=$z ppid=$SUP_PID" >> "$ZOMBIE_LOG"
            done
        fi
        sleep 0.2
    done
) &
WATCHER_PID=$!
log "zombie_watcher_pid=$WATCHER_PID"

FAIL_COUNT=0
for i in $(seq 1 "$CYCLES"); do
    T0=$(date +%s.%N)

    if ! riku ps "$APP" --scale "${WORKER_KIND}=20" >>"$LOG" 2>&1; then
        log "cycle=$i scale-up FAILED"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    sleep 0.3

    if ! riku ps "$APP" --scale "${WORKER_KIND}=1" >>"$LOG" 2>&1; then
        log "cycle=$i scale-down FAILED"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    sleep 0.3

    T1=$(date +%s.%N)
    ELAPSED=$(echo "$T1 - $T0" | bc 2>/dev/null || echo "?")
    CURRENT_Z="$(zombie_descendants_of "$SUP_PID" | wc -l)"

    log "cycle=$i elapsed=${ELAPSED}s zombies_now=$CURRENT_Z"

    if [ "$CURRENT_Z" -gt "$BASELINE_Z" ]; then
        log "!! ZOMBIE ACCUMULATION DETECTED at cycle $i (count=$CURRENT_Z baseline=$BASELINE_Z)"
    fi
done

kill "$WATCHER_PID" 2>/dev/null
wait "$WATCHER_PID" 2>/dev/null

FINAL_Z="$(zombie_descendants_of "$SUP_PID" | wc -l)"
ZOMBIE_EVENTS="$(wc -l < "$ZOMBIE_LOG" 2>/dev/null || echo 0)"

log "=== run complete ==="
log "scale_command_failures=$FAIL_COUNT"
log "baseline_zombies=$BASELINE_Z final_zombies=$FINAL_Z"
log "total_zombie_sample_events=$ZOMBIE_EVENTS (see $ZOMBIE_LOG)"

if [ "$FINAL_Z" -gt "$BASELINE_Z" ] || [ "$ZOMBIE_EVENTS" -gt 0 ]; then
    log "RESULT: FAIL — zombies observed during or after the run"
    exit 2
else
    log "RESULT: PASS — no zombie accumulation observed"
    exit 0
fi
