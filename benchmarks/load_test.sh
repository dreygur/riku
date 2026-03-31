#!/bin/bash
# Riku Load Test Script
# Tests the health endpoint under concurrent load and measures CLI startup time
# over many iterations to get a stable baseline.

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

HEALTH_PORT="${RIKU_HEALTH_PORT:-9091}"
HEALTH_HOST="${RIKU_HEALTH_HOST:-127.0.0.1}"
CONCURRENCY="${LOAD_CONCURRENCY:-10}"
REQUESTS="${LOAD_REQUESTS:-100}"
STARTUP_ITERS="${STARTUP_ITERS:-20}"

echo "=== Riku Load Test ==="
echo ""

# ── Helper: require a command ──────────────────────────────────────────────────
require() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}Error: '$1' not found. Install it to run this test.${NC}"
        exit 1
    fi
}

# ── 1. CLI startup time (stable average over N iterations) ────────────────────
echo -e "${BLUE}=== 1. CLI Startup Time ===${NC}"

if ! command -v riku &>/dev/null; then
    echo -e "${YELLOW}Warning: riku not in PATH — skipping startup benchmark.${NC}"
    echo "  Build and install with: cargo build --release && sudo cp target/release/riku /usr/local/bin/"
    echo ""
else
    total_ms=0
    min_ms=999999
    max_ms=0

    for i in $(seq 1 "$STARTUP_ITERS"); do
        start=$(date +%s%N)
        riku --help >/dev/null 2>&1
        end=$(date +%s%N)
        elapsed_ms=$(( (end - start) / 1000000 ))

        total_ms=$(( total_ms + elapsed_ms ))
        [ "$elapsed_ms" -lt "$min_ms" ] && min_ms=$elapsed_ms
        [ "$elapsed_ms" -gt "$max_ms" ] && max_ms=$elapsed_ms
    done

    avg_ms=$(( total_ms / STARTUP_ITERS ))
    echo "  Iterations : $STARTUP_ITERS"
    echo "  Average    : ${avg_ms} ms"
    echo "  Min        : ${min_ms} ms"
    echo "  Max        : ${max_ms} ms"

    if [ "$avg_ms" -lt 50 ]; then
        echo -e "  ${GREEN}✓ Excellent startup time (< 50 ms)${NC}"
    elif [ "$avg_ms" -lt 150 ]; then
        echo -e "  ${GREEN}✓ Good startup time (< 150 ms)${NC}"
    else
        echo -e "  ${YELLOW}⚠ Slow startup time (>= 150 ms)${NC}"
    fi
    echo ""
fi

# ── 2. Health endpoint load test ──────────────────────────────────────────────
echo -e "${BLUE}=== 2. Health Endpoint Load Test ===${NC}"

# Check if health server is running
if ! curl -sf "http://${HEALTH_HOST}:${HEALTH_PORT}/health" >/dev/null 2>&1; then
    echo -e "${YELLOW}Warning: Health server not reachable at ${HEALTH_HOST}:${HEALTH_PORT}${NC}"
    echo "  Start the supervisor first: riku supervisor"
    echo "  Or set RIKU_HEALTH_PORT / RIKU_HEALTH_HOST env vars."
    echo ""
else
    require curl

    echo "  Target     : http://${HEALTH_HOST}:${HEALTH_PORT}/health"
    echo "  Concurrency: $CONCURRENCY"
    echo "  Requests   : $REQUESTS"
    echo ""

    # Use xargs for parallel curl requests (portable, no extra deps)
    tmp_times=$(mktemp)
    trap 'rm -f "$tmp_times"' EXIT

    seq 1 "$REQUESTS" | xargs -P "$CONCURRENCY" -I{} sh -c \
        'start=$(date +%s%N); \
         code=$(curl -sf -o /dev/null -w "%{http_code}" "http://'"${HEALTH_HOST}:${HEALTH_PORT}"'/health" 2>/dev/null || echo "000"); \
         end=$(date +%s%N); \
         echo "$code $(( (end - start) / 1000000 ))"' \
        >> "$tmp_times" 2>&1

    total_req=$(wc -l < "$tmp_times")
    success=$(grep -c "^200 " "$tmp_times" || true)
    failed=$(( total_req - success ))

    # Calculate latency stats from successful requests
    if [ "$success" -gt 0 ]; then
        latencies=$(grep "^200 " "$tmp_times" | awk '{print $2}' | sort -n)
        total_lat=$(echo "$latencies" | awk '{s+=$1} END {print s}')
        avg_lat=$(( total_lat / success ))
        min_lat=$(echo "$latencies" | head -1)
        max_lat=$(echo "$latencies" | tail -1)
        p95_idx=$(( success * 95 / 100 ))
        p95_lat=$(echo "$latencies" | sed -n "${p95_idx}p")
    else
        avg_lat="-"
        min_lat="-"
        max_lat="-"
        p95_lat="-"
    fi

    echo "  Results:"
    echo "    Total requests : $total_req"
    echo "    Successful     : $success"
    echo "    Failed         : $failed"
    echo ""
    echo "  Latency (ms):"
    echo "    Average : ${avg_lat}"
    echo "    Min     : ${min_lat}"
    echo "    Max     : ${max_lat}"
    echo "    p95     : ${p95_lat}"

    success_pct=$(( success * 100 / total_req ))
    if [ "$success_pct" -eq 100 ]; then
        echo -e "  ${GREEN}✓ 100% success rate${NC}"
    elif [ "$success_pct" -ge 99 ]; then
        echo -e "  ${YELLOW}⚠ ${success_pct}% success rate${NC}"
    else
        echo -e "  ${RED}✗ ${success_pct}% success rate — investigate failures${NC}"
    fi
    echo ""

    # ── 3. Per-app metrics endpoint ───────────────────────────────────────────
    echo -e "${BLUE}=== 3. Per-App Metrics Endpoint ===${NC}"
    apps_json=$(curl -sf "http://${HEALTH_HOST}:${HEALTH_PORT}/metrics/apps" 2>/dev/null || echo "[]")
    app_count=$(echo "$apps_json" | grep -o '"app":' | wc -l || true)
    echo "  /metrics/apps returned $app_count app(s)"

    if echo "$apps_json" | grep -q '"app":'; then
        # Test per-app endpoint for first app found
        first_app=$(echo "$apps_json" | grep -o '"app":"[^"]*"' | head -1 | cut -d'"' -f4)
        if [ -n "$first_app" ]; then
            code=$(curl -sf -o /dev/null -w "%{http_code}" \
                "http://${HEALTH_HOST}:${HEALTH_PORT}/metrics/apps/${first_app}" 2>/dev/null || echo "000")
            if [ "$code" = "200" ]; then
                echo -e "  ${GREEN}✓ /metrics/apps/${first_app} → HTTP 200${NC}"
            else
                echo -e "  ${RED}✗ /metrics/apps/${first_app} → HTTP ${code}${NC}"
            fi
        fi
    fi

    # Non-existent app should 404
    code=$(curl -sf -o /dev/null -w "%{http_code}" \
        "http://${HEALTH_HOST}:${HEALTH_PORT}/metrics/apps/__nonexistent__" 2>/dev/null || echo "000")
    if [ "$code" = "404" ]; then
        echo -e "  ${GREEN}✓ /metrics/apps/__nonexistent__ → HTTP 404 (correct)${NC}"
    else
        echo -e "  ${YELLOW}⚠ /metrics/apps/__nonexistent__ → HTTP ${code} (expected 404)${NC}"
    fi
    echo ""
fi

# ── 4. Binary size ─────────────────────────────────────────────────────────────
echo -e "${BLUE}=== 4. Binary Information ===${NC}"
if command -v riku &>/dev/null; then
    binary=$(which riku)
    size_bytes=$(stat -c%s "$binary" 2>/dev/null || stat -f%z "$binary" 2>/dev/null || echo "0")
    size_mb=$(echo "scale=1; $size_bytes / 1048576" | bc 2>/dev/null || echo "?")
    version=$(riku --version 2>&1 | head -1)
    echo "  Binary  : $binary"
    echo "  Size    : ${size_mb} MB"
    echo "  Version : $version"
fi
echo ""

echo -e "${GREEN}Load test complete.${NC}"
echo "Tip: Share your results in benchmarks/results/ — see benchmarks/README.md for the format."
