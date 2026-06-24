#!/bin/bash
# Riku Benchmarking Script
# This script helps measure Riku's resource usage and performance

set -e

echo "=== Riku Benchmarking Suite ==="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if riku is installed
if ! command -v riku &> /dev/null; then
    echo -e "${RED}Error: riku command not found${NC}"
    echo "Please install riku first:"
    echo "  cargo build --release"
    echo "  sudo cp target/release/riku /usr/local/bin/"
    exit 1
fi

echo -e "${GREEN}✓ Riku is installed${NC}"
echo ""

# Function to measure memory usage
measure_memory() {
    local process=$1
    local pid=$(pgrep -f "$process" | head -1)
    
    if [ -n "$pid" ]; then
        local mem=$(ps -o rss= -p $pid 2>/dev/null || echo "0")
        echo "$((mem / 1024)) MB"
    else
        echo "Not running"
    fi
}

# Function to measure binary size
measure_binary_size() {
    local binary=$(which riku)
    if [ -f "$binary" ]; then
        local size=$(stat -c%s "$binary" 2>/dev/null || stat -f%z "$binary" 2>/dev/null || echo "0")
        echo "$((size / 1024 / 1024)) MB"
    else
        echo "Not found"
    fi
}

# Function to measure startup time
measure_startup_time() {
    local start=$(date +%s%N)
    riku --help > /dev/null 2>&1
    local end=$(date +%s%N)
    local diff=$(( (end - start) / 1000000 ))
    echo "${diff} ms"
}

echo "=== Binary Information ==="
echo -n "Binary location: "
which riku
echo -n "Binary size: "
measure_binary_size
echo -n "Version: "
riku --version 2>&1 | head -1
echo ""

echo "=== Resource Usage ==="
echo -n "Supervisor memory: "
measure_memory "riku supervisor"
echo ""

echo "=== Performance Tests ==="
echo -n "CLI startup time: "
measure_startup_time
echo ""

echo "=== System Information ==="
echo "OS: $(uname -s) $(uname -r)"
echo "Architecture: $(uname -m)"
echo "CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 'unknown')"
echo "Total RAM: $(free -h 2>/dev/null | grep Mem | awk '{print $2}' || sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.1f GB", $1/1024/1024/1024}' || echo 'unknown')"
echo ""

echo "=== Benchmarking Tips ==="
echo "1. Run 'riku supervisor' in background to measure supervisor memory"
echo "2. Deploy test apps to measure per-app resource usage"
echo "3. Use 'ps aux | grep riku' to see all Riku processes"
echo "4. Use 'htop' or 'top' for real-time monitoring"
echo ""

echo "=== Share Your Results ==="
echo "We encourage you to share your benchmark results!"
echo "Create a file in benches/results/ with your findings."
echo "See benches/README.md for the format."
echo ""

echo -e "${GREEN}Benchmarking complete!${NC}"
echo ""
echo "Note: Results vary based on system load, number of apps, and workload."
echo "For accurate measurements, run multiple times and average the results."
