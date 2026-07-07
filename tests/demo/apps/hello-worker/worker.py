#!/usr/bin/env python3
"""Background worker process -- shows up alongside `web` in the dashboard's
process table, demonstrating riku's multi-process-per-app support."""
import time

while True:
    print(f"[worker] heartbeat", flush=True)
    time.sleep(10)
