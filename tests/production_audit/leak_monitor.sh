#!/usr/bin/env bash
# leak_monitor.sh
#
# Triggers 50 consecutive `riku config set` calls (env var rewrite +
# redeploy, see src/cli/cmds.rs ConfigCmd::Set and src/deploy/env_setup.rs)
# against a target app, and tracks the supervisor's open file descriptor
# count and RSS over the run. Each `config set` rewrites the app's ENV
# file and triggers do_deploy, which restarts the app's workers — this
# exercises src/supervisor/daemon/config_watcher.rs's reload_all_configs()
# path (unload_config + load_config_file) on every iteration.
#
# Target leak surfaces (read before judging results):
#   - src/supervisor/log_rotation/mod.rs: opens File::open / OpenOptions
#     per rotation; if these aren't dropped promptly, fd count climbs.
#   - src/supervisor/daemon/config_watcher.rs: reload_all_configs() builds
#     a fresh HashMap<String, PathBuf> every call; check it isn't retained.
set -uo pipefail

APP="${1:-leakmonitor}"
ITERATIONS="${2:-50}"
RESULT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/results"
mkdir -p "$RESULT_DIR"
TS="$(date +%Y%m%d_%H%M%S)"
LOG="$RESULT_DIR/leak_monitor_${TS}.log"
CSV="$RESULT_DIR/leak_monitor_${TS}.csv"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"; }

find_supervisor_pid() {
    pgrep -f "riku[[:space:]]+supervisor" | head -1
}

sample() {
    local pid="$1" label="$2"
    local fd_count rss_kb vss_kb threads
    fd_count="$(ls "/proc/$pid/fd" 2>/dev/null | wc -l)"
    rss_kb="$(awk '/VmRSS/{print $2}' "/proc/$pid/status" 2>/dev/null)"
    vss_kb="$(awk '/VmSize/{print $2}' "/proc/$pid/status" 2>/dev/null)"
    threads="$(awk '/Threads/{print $2}' "/proc/$pid/status" 2>/dev/null)"
    echo "$(date +%s),$label,$fd_count,${rss_kb:-0},${vss_kb:-0},${threads:-0}" >> "$CSV"
    log "sample[$label] fd=$fd_count rss_kb=${rss_kb:-0} vss_kb=${vss_kb:-0} threads=${threads:-0}"
}

log "=== leak_monitor.sh starting ==="
log "app=$APP iterations=$ITERATIONS"

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

echo "epoch,label,fd_count,rss_kb,vss_kb,threads" > "$CSV"
sample "$SUP_PID" "baseline"

FAIL_COUNT=0
for i in $(seq 1 "$ITERATIONS"); do
    if ! riku config set "$APP" "LEAK_TEST_BUILD_ID=$i" >>"$LOG" 2>&1; then
        log "iteration=$i config set FAILED"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    # Force a config reload pass deterministically rather than waiting on
    # the daemon's own watcher (avoids racing the 1s recv_timeout tick).
    kill -HUP "$SUP_PID" 2>/dev/null || true

    sleep 0.3
    sample "$SUP_PID" "iter_$i"
done

sample "$SUP_PID" "final"

log "=== fd listing at end of run (top 20 by target) ==="
ls -la "/proc/$SUP_PID/fd" 2>/dev/null | sort -k11 | uniq -c -f10 | sort -rn | head -20 | tee -a "$LOG"

BASELINE_FD="$(awk -F, 'NR==2{print $3}' "$CSV")"
FINAL_FD="$(tail -1 "$CSV" | awk -F, '{print $3}')"
BASELINE_RSS="$(awk -F, 'NR==2{print $4}' "$CSV")"
FINAL_RSS="$(tail -1 "$CSV" | awk -F, '{print $4}')"

log "=== run complete ==="
log "config_set_failures=$FAIL_COUNT"
log "fd_baseline=$BASELINE_FD fd_final=$FINAL_FD"
log "rss_baseline_kb=$BASELINE_RSS rss_final_kb=$FINAL_RSS"
log "raw samples: $CSV"

FD_GROWTH=$((FINAL_FD - BASELINE_FD))
log "fd_growth=$FD_GROWTH (threshold: fail if > 5, i.e. growing roughly 1:1 with reload count)"

if [ "$FD_GROWTH" -gt 5 ]; then
    log "RESULT: FAIL — fd count grew by $FD_GROWTH over $ITERATIONS reloads, possible fd leak"
    exit 2
else
    log "RESULT: PASS — fd count stable within threshold"
    exit 0
fi
