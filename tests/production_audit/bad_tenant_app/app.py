#!/usr/bin/env python3
"""Hostile test tenant for resource-limit enforcement audits.

Two failure modes, selected by argv[1]:
  mem  (default) - unbounded heap growth, ~10MB/iteration, no cap.
                   Used to test RLIMIT_AS enforcement (or lack thereof)
                   applied via src/supervisor/resource_limits/mod.rs
                   ResourceLimits::apply() in the pre_exec() hook in
                   src/supervisor/process/spawn.rs.
  cpu            - tight spin loop, no syscalls, no sleep.
                   Used to test RLIMIT_CPU enforcement. Note: RLIMIT_CPU
                   caps total CPU *time consumed*, it does not prevent
                   this loop from pegging one core for the entire
                   duration before the limit is hit and SIGXCPU fires.

This script intentionally has no error handling, no cleanup, and no
safety valves - that is the point. It must be run only under a riku
supervisor instance configured with RIKU_MAX_MEMORY_MB / RIKU_MAX_CPU_SECONDS,
never on a host without those limits applied.
"""
import sys
import time


def leak_memory():
    chunks = []
    iteration = 0
    while True:
        chunks.append(bytearray(10 * 1024 * 1024))  # +10MB, never freed
        iteration += 1
        print(f"[bad_tenant] mem iteration={iteration} approx_mb={iteration * 10}", flush=True)
        time.sleep(0.05)


def spin_cpu():
    iteration = 0
    while True:
        iteration += 1
        if iteration % 50_000_000 == 0:
            print(f"[bad_tenant] cpu spin checkpoint={iteration}", flush=True)


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "mem"
    if mode == "mem":
        leak_memory()
    elif mode == "cpu":
        spin_cpu()
    else:
        print(f"unknown mode '{mode}', use 'mem' or 'cpu'", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
