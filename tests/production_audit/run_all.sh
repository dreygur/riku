#!/usr/bin/env bash
# run_all.sh — orchestrates the full production audit suite in order.
# Requires a running `riku supervisor` instance reachable via the `riku`
# CLI on PATH. Resource-limit tests additionally require the supervisor
# to have been started with bad_tenant_app/start-supervisor.env sourced.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULT_DIR="$SCRIPT_DIR/results"
mkdir -p "$RESULT_DIR"
SUMMARY="$RESULT_DIR/run_all_summary_$(date +%Y%m%d_%H%M%S).log"

run_step() {
    local name="$1"; shift
    echo "=== running: $name ===" | tee -a "$SUMMARY"
    if "$@"; then
        echo "[$name] PASS" | tee -a "$SUMMARY"
    else
        echo "[$name] FAIL (exit $?)" | tee -a "$SUMMARY"
    fi
}

if ! pgrep -f "riku[[:space:]]+supervisor" >/dev/null; then
    echo "no 'riku supervisor' process detected. Start one before running this suite:" | tee -a "$SUMMARY"
    echo "  riku supervisor &" | tee -a "$SUMMARY"
    echo "for the resource-limit step, start it instead with:" | tee -a "$SUMMARY"
    echo "  set -a; source $SCRIPT_DIR/bad_tenant_app/start-supervisor.env; set +a; riku supervisor &" | tee -a "$SUMMARY"
    exit 1
fi

run_step "stress_lifecycle"     "$SCRIPT_DIR/stress_lifecycle.sh" stresslifecycle 100
run_step "leak_monitor"         "$SCRIPT_DIR/leak_monitor.sh" leakmonitor 50
run_step "chaos_signals"        "$SCRIPT_DIR/chaos_signals.sh" chaossignals web 30
run_step "resource_limit_mem"   "$SCRIPT_DIR/resource_limit_audit.sh" badtenant mem
run_step "resource_limit_cpu"   "$SCRIPT_DIR/resource_limit_audit.sh" badtenant cpu

echo "=== full suite complete, summary at $SUMMARY ===" | tee -a "$SUMMARY"
cat "$SUMMARY"
