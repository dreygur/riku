# Resource Limits Configuration

**Status:** ✅ Production Ready  
**Version:** 3.0.0-rc.1

---

## Overview

Riku implements comprehensive resource limits (ulimit) for all spawned application processes. This prevents runaway processes, enables safe multi-tenant deployments, and protects the host system from resource exhaustion.

---

## Default Limits

By default, Riku enforces the following limits on all application processes:

| Resource | Limit | Environment Variable | Description |
|----------|-------|---------------------|-------------|
| **Memory** | 512 MB | `RIKU_MAX_MEMORY_MB` | Maximum virtual memory (address space); accepts `unlimited` |
| **CPU Time** | 3600 seconds | `RIKU_MAX_CPU_SECONDS` | Maximum CPU time before SIGKILL |
| **Open Files** | 1024 | `RIKU_MAX_OPEN_FILES` | Maximum file descriptors |
| **Processes** | 64 | `RIKU_MAX_PROCESSES` | Maximum child processes (fork limit) |
| **File Size** | 1 GB | `RIKU_MAX_FILE_SIZE_MB` | Maximum size of created files |
| **Core Dumps** | 0 (disabled) | `RIKU_ENABLE_CORE_DUMPS` | Core dump files (disabled for security) |

---

## Configuration

### Environment Variables

Configure resource limits via environment variables before starting the supervisor:

```bash
# Set memory limit to 256 MB
export RIKU_MAX_MEMORY_MB=256

# Set CPU time limit to 1 hour (3600 seconds)
export RIKU_MAX_CPU_SECONDS=3600

# Set max open files to 2048
export RIKU_MAX_OPEN_FILES=2048

# Set max processes to 32
export RIKU_MAX_PROCESSES=32

# Set max file size to 512 MB
export RIKU_MAX_FILE_SIZE_MB=512

# Enable core dumps (NOT recommended for production)
export RIKU_ENABLE_CORE_DUMPS=1

# Start supervisor with configured limits
riku supervisor
```

### Systemd Service

For systemd deployments, add environment variables to the service file:

```ini
[Service]
Type=simple
User=riku
WorkingDirectory=/home/riku
ExecStart=/usr/local/bin/riku supervisor

# Resource limits
Environment="RIKU_MAX_MEMORY_MB=512"
Environment="RIKU_MAX_CPU_SECONDS=7200"
Environment="RIKU_MAX_OPEN_FILES=1024"
Environment="RIKU_MAX_PROCESSES=64"
Environment="RIKU_MAX_FILE_SIZE_MB=1024"

Restart=always
RestartSec=10s
```

**This `Environment=` block only affects already-running *workers*** —
the supervisor daemon `systemd` spawns. It does **not** affect the `build`
step of a deploy, which runs inside `riku git-hook` triggered by `git
push`'s post-receive hook: a separate, short-lived process spawned by the
SSH session of whoever pushed, not by systemd. See the unlimited-memory
section below if your build step needs different limits than your workers.

---

## Unlimited Memory for Go-Based Build Tools

`RIKU_MAX_MEMORY_MB` also accepts the literal value `unlimited`, which skips
applying `RLIMIT_AS` entirely instead of setting some large finite cap.

This exists specifically for plugin `build` steps that shell out to a Go
binary — `docker`, `podman`, or the `go` toolchain itself (used by the
bundled `container`, `ghcr`, and `go` runtime plugins). The Go runtime
reserves a large virtual address space for its heap arena at process
*startup*, regardless of how much memory the program will actually use, and
`RLIMIT_AS` caps address space, not resident memory — so no finite limit
compatible with normal host RAM avoids this. It was confirmed still
failing (`fatal error: failed to reserve page summary memory`) at 32 GB in
testing; only `unlimited` reliably works.

```bash
export RIKU_MAX_MEMORY_MB=unlimited
```

Set this **in the environment of whatever runs `riku git-hook`/`riku
deploy`** for the build step to pick it up — typically the `deploy` user's
SSH session on the riku host (e.g. system-wide via `/etc/environment`, read
by `pam_env` for SSH logins on most distros), not the `riku supervisor`
systemd unit, which is a different process tree and only governs already-
running workers (see the note above). Confirm it took effect by checking
the deploy log for `Resource limit: max_memory = unlimited` (visible with
`RUST_LOG=info`).

This only disables the *build*-step memory cap; CPU/file-descriptor/process
limits and all *worker* limits are unaffected and still enforced normally.

---

## Multi-Tenant Recommendations

For multi-tenant deployments, use stricter limits to prevent resource abuse:

### Conservative (High Security)
```bash
export RIKU_MAX_MEMORY_MB=256      # 256 MB per app
export RIKU_MAX_CPU_SECONDS=1800   # 30 minutes
export RIKU_MAX_OPEN_FILES=512     # 512 files
export RIKU_MAX_PROCESSES=32       # 32 processes
export RIKU_MAX_FILE_SIZE_MB=512   # 512 MB files
```

### Moderate (Balanced)
```bash
export RIKU_MAX_MEMORY_MB=512      # 512 MB per app
export RIKU_MAX_CPU_SECONDS=3600   # 1 hour
export RIKU_MAX_OPEN_FILES=1024    # 1024 files
export RIKU_MAX_PROCESSES=64       # 64 processes
export RIKU_MAX_FILE_SIZE_MB=1024  # 1 GB files
```

### Generous (Trusted Users)
```bash
export RIKU_MAX_MEMORY_MB=1024     # 1 GB per app
export RIKU_MAX_CPU_SECONDS=7200   # 2 hours
export RIKU_MAX_OPEN_FILES=2048    # 2048 files
export RIKU_MAX_PROCESSES=128      # 128 processes
export RIKU_MAX_FILE_SIZE_MB=2048  # 2 GB files
```

---

## How It Works

### Implementation

Resource limits are enforced using the POSIX `setrlimit()` system call through the `nix` crate. Limits are set in the child process immediately after `fork()` but before `exec()`:

1. **Fork** - Create child process
2. **Set Limits** - Apply resource limits via `setrlimit()`
3. **Drop Privileges** - Change to configured UID/GID if specified
4. **Exec** - Replace process image with application

This ensures limits are in place before the application code runs.

### What Happens When Limits Are Exceeded

| Resource | Behavior |
|----------|----------|
| **Memory** | Process receives `SIGSEGV` or allocation fails with `ENOMEM` |
| **CPU Time** | Process receives `SIGKILL` after exceeding CPU seconds |
| **Open Files** | `open()` fails with `EMFILE` (too many open files) |
| **Processes** | `fork()` fails with `EAGAIN` (resource temporarily unavailable) |
| **File Size** | `write()` fails with `EFBIG` + `SIGXFSZ` sent to process |

### Monitoring

The supervisor logs resource limit configuration on startup:

```
ProcessManager initialized with resource limits: mem=512MB, cpu=3600s, files=1024, procs=64
```

---

## Testing

### Verify Limits Are Applied

Deploy a test app and check its limits:

```bash
# Deploy test app
git push riku main

# Find the app's PID
riku ps myapp

# Check resource limits for the process
cat /proc/<PID>/limits
```

Expected output:
```
Limit                     Soft Limit           Hard Limit           Units
Max cpu time              3600                 3600                 seconds
Max file size             1073741824           1073741824           bytes
Max data size             536870912            536870912            bytes
Max stack size            8388608              8388608              bytes
Max core file size        0                    0                    bytes
Max resident set          unlimited            unlimited            bytes
Max processes             64                   64                   processes
Max open files            1024                 1024                 files
Max locked memory         unlimited            unlimited            bytes
Max address space         536870912            536870912            bytes
```

### Test Memory Limit

Create an app that tries to allocate excessive memory:

```python
# memory_hog.py
import sys

def allocate_memory(mb):
    data = []
    try:
        for i in range(mb):
            data.append(' ' * 1024 * 1024)  # 1 MB
            print(f"Allocated {i+1} MB")
    except MemoryError:
        print(f"Memory limit reached at {i} MB", file=sys.stderr)
        sys.exit(1)

allocate_memory(1000)  # Try to allocate 1 GB
```

**Expected:** Process will be killed when exceeding configured memory limit.

### Test CPU Limit

Create an app that runs indefinitely:

```python
# cpu_hog.py
import time

start = time.time()
while True:
    pass  # Infinite CPU loop
```

**Expected:** Process receives `SIGKILL` after configured CPU time limit.

### Test File Descriptor Limit

```python
# fd_hog.py
import sys

files = []
try:
    for i in range(2000):
        files.append(open('/dev/null', 'r'))
        print(f"Opened {i+1} files")
except OSError as e:
    print(f"File limit reached: {e}", file=sys.stderr)
    sys.exit(1)
```

**Expected:** `open()` fails when reaching file descriptor limit.

---

## Troubleshooting

### Application Crashes with SIGKILL

**Symptom:** App terminates unexpectedly with signal 9 (SIGKILL)

**Possible Causes:**
1. **CPU limit exceeded** - App used too much CPU time
2. **Memory limit exceeded** - App tried to allocate too much memory

**Solution:**
- Check app logs in `~/.riku/logs/<app>/`
- Increase limits if legitimate usage:
  ```bash
  export RIKU_MAX_CPU_SECONDS=7200  # 2 hours
  export RIKU_MAX_MEMORY_MB=1024    # 1 GB
  systemctl restart riku-supervisor
  ```

### File Operations Failing

**Symptom:** App logs show "Too many open files" errors

**Solution:**
```bash
export RIKU_MAX_OPEN_FILES=2048
systemctl restart riku-supervisor
```

### Cannot Fork Worker Processes

**Symptom:** App logs show "Resource temporarily unavailable" on `fork()`

**Solution:**
```bash
export RIKU_MAX_PROCESSES=128
systemctl restart riku-supervisor
```

---

## Security Considerations

### Why These Limits Matter

1. **Prevents DoS** - Malicious or buggy apps cannot exhaust host resources
2. **Fair Sharing** - Resources are distributed fairly among apps
3. **Predictable Behavior** - Apps have defined resource guarantees
4. **Early Detection** - Resource issues surface quickly during testing

### Best Practices

1. ✅ **Start Conservative** - Begin with strict limits, relax as needed
2. ✅ **Monitor Usage** - Check `/metrics` endpoint for resource usage
3. ✅ **Test Limits** - Deploy to staging first with production limits
4. ✅ **Log Violations** - Review logs for apps hitting limits
5. ❌ **Don't Disable** - Always enforce limits in multi-tenant environments
6. ❌ **Don't Enable Core Dumps** - Security risk in production

---

## Integration with Monitoring

Resource limits work seamlessly with Riku's observability features:

### Health Checks
```bash
curl http://localhost:9091/health
# Returns supervisor status with resource limit config
```

### Metrics
```bash
curl http://localhost:9091/metrics
# Returns per-process resource usage
# Compare against configured limits to detect issues
```

### Structured Logs
```bash
RUST_LOG=info riku supervisor
# Logs resource limit configuration on startup
# Logs when limits are exceeded (via app stderr)
```

---

## Migration from Older Versions

If upgrading from Riku 2.x:

1. **No Breaking Changes** - Resource limits are enabled by default with safe values
2. **Review App Behavior** - Some apps may need higher limits
3. **Test First** - Deploy to staging before production
4. **Monitor Logs** - Check for unexpected terminations

### Rollback

If resource limits cause issues, temporarily disable by setting very high
values (or, for memory specifically when the offending process is a Go
binary — see [above](#unlimited-memory-for-go-based-build-tools) — use the
literal `unlimited`, since no finite value actually works for that case):

```bash
export RIKU_MAX_MEMORY_MB=16384      # 16 GB — not truly unlimited; Go
                                      # binaries still fail at this value
export RIKU_MAX_CPU_SECONDS=86400    # 24 hours
export RIKU_MAX_OPEN_FILES=65536     # 64K files
export RIKU_MAX_PROCESSES=1024       # 1K processes
systemctl restart riku-supervisor
```

---

## Future Enhancements

Planned improvements:

- [ ] **Per-App Limits** - Configure limits per application in `ENV` file
- [ ] **cgroups v2 Support** - More granular control (CPU shares, memory.max)
- [ ] **Automatic Scaling** - Increase limits based on metrics
- [ ] **Limit Notifications** - Alert when apps approach limits

---

## References

- [setrlimit(2) man page](https://man7.org/linux/man-pages/man2/setrlimit.2.html)
- [Resource limit kernel documentation](https://www.kernel.org/doc/Documentation/scheduler/sched-bwc.txt)
- [nix crate documentation](https://docs.rs/nix/latest/nix/sys/resource/)

---

**Status:** ✅ Production Ready  
**Last Updated:** March 11, 2026  
**Version:** 3.0.0-rc.1
