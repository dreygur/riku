# Resource Limits Configuration

## Overview

Riku implements comprehensive resource limits (ulimit) for all spawned application processes. This prevents runaway processes, enables safe multi-tenant deployments, and protects the host system from resource exhaustion.

---

## Default Limits

By default, Riku enforces the following limits on all application processes:

| Resource | Limit | Environment Variable | Description |
|----------|-------|---------------------|-------------|
| **Memory** | *not applied* | `RIKU_MAX_MEMORY_MB` | Maximum virtual memory (address space) |
| **CPU Time** | 3600 seconds | `RIKU_MAX_CPU_SECONDS` | Maximum CPU time before SIGKILL |
| **Open Files** | 1024 | `RIKU_MAX_OPEN_FILES` | Maximum file descriptors |
| **Processes** | *not applied* | `RIKU_MAX_PROCESSES` | Maximum child processes (fork limit) |
| **File Size** | 1 GB | `RIKU_MAX_FILE_SIZE_MB` | Maximum size of created files |
| **Core Dumps** | 0 (disabled) | `RIKU_ENABLE_CORE_DUMPS` | Core dump files (disabled for security) |

Two of these are opt-in and are left unset unless you configure them:

- **Memory** caps *virtual* address space (`RLIMIT_AS`). Node/v8 and the JVM
  reserve multiple GB of virtual memory at startup and abort under a tight cap,
  so applying one by default would make those builds and workers unrunnable.
  Resident memory is bounded by the cgroup `memory.max` instead, set with
  `limits.isolation_max_memory_mb` (see [riku.toml](riku-toml.md)).
- **Processes** (`RLIMIT_NPROC`) is UID-wide, not per-worker: every process the
  deploy user owns counts against it, including the supervisor and your shell.
  A per-app-looking number set here can lock the whole user out of forking.

Set either one explicitly when you want it enforced.

---

## Configuration

### Where the limits are read

The server wide limits come from the environment of the process that starts
the command. The supervisor reads these variables once, at startup, for
every worker and cron job of every app. A build and the Procfile
`preflight` and `release` commands read them from the riku process that
runs the deploy. `riku run` reads them from the riku process that runs the
command.

An app can set its own value for any of them with the `limits.max_*` keys
of `riku settings` (see [riku.toml](riku-toml.md)):

```bash
riku settings set myapp limits.max_open_files=4096 limits.max_cpu_seconds=7200
```

| Key | Variable |
|-----|----------|
| `limits.max_memory_mb` | `RIKU_MAX_MEMORY_MB` |
| `limits.max_cpu_seconds` | `RIKU_MAX_CPU_SECONDS` |
| `limits.max_open_files` | `RIKU_MAX_OPEN_FILES` |
| `limits.max_processes` | `RIKU_MAX_PROCESSES` |
| `limits.max_file_size_mb` | `RIKU_MAX_FILE_SIZE_MB` |

A value the app sets replaces the server wide one for that app's workers,
cron jobs, builds, `preflight` and `release` commands and `riku run`, in either
direction. A limit the app does not set keeps the server wide value. `set`
redeploys the app, so the new limits apply from that deploy. Core dumps
have no per app key. `limits.max_processes` is still UID-wide, as described
above, even when one app sets it.

### Systemd Service

The supervisor runs as `riku.service` (a root install) or as the deploy
user's user unit `riku`. `riku init` rewrites the unit file itself, so put
the variables in a drop-in, which init leaves alone:

```bash
sudo systemctl edit riku        # or, for a user install: systemctl --user edit riku
```

```ini
[Service]
Environment="RIKU_MAX_MEMORY_MB=512"
Environment="RIKU_MAX_CPU_SECONDS=7200"
Environment="RIKU_MAX_OPEN_FILES=1024"
Environment="RIKU_MAX_PROCESSES=64"
Environment="RIKU_MAX_FILE_SIZE_MB=1024"
# Enable core dumps (not recommended for production)
# Environment="RIKU_ENABLE_CORE_DUMPS=1"
```

Then restart the supervisor so it reads them, which restarts every app's
workers:

```bash
sudo systemctl restart riku     # or: systemctl --user restart riku
```

### Running the supervisor by hand

Without systemd, set the variables in the shell that starts it:

```bash
RIKU_MAX_CPU_SECONDS=3600 RIKU_MAX_OPEN_FILES=2048 riku supervisor
```

---

## Multi-Tenant Recommendations

For multi-tenant deployments, use stricter limits to prevent resource abuse.
Each set below goes in the `riku` unit's drop-in (see above); the values
apply to every worker of every app.

### Conservative (High Security)
```ini
Environment="RIKU_MAX_MEMORY_MB=256"
Environment="RIKU_MAX_CPU_SECONDS=1800"
Environment="RIKU_MAX_OPEN_FILES=512"
Environment="RIKU_MAX_PROCESSES=32"
Environment="RIKU_MAX_FILE_SIZE_MB=512"
```

### Moderate (Balanced)
```ini
Environment="RIKU_MAX_MEMORY_MB=512"
Environment="RIKU_MAX_CPU_SECONDS=3600"
Environment="RIKU_MAX_OPEN_FILES=1024"
Environment="RIKU_MAX_PROCESSES=64"
Environment="RIKU_MAX_FILE_SIZE_MB=1024"
```

### Generous (Trusted Users)
```ini
Environment="RIKU_MAX_MEMORY_MB=1024"
Environment="RIKU_MAX_CPU_SECONDS=7200"
Environment="RIKU_MAX_OPEN_FILES=2048"
Environment="RIKU_MAX_PROCESSES=128"
Environment="RIKU_MAX_FILE_SIZE_MB=2048"
```

To give one app different values, use its `limits.max_*` keys (see
"Where the limits are read" above). For per worker resident memory, CPU
share and process count, use the cgroup settings `limits.isolation_*` (see
[riku.toml](riku-toml.md)).

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
ProcessManager initialized with resource limits: cpu=3600s, files=1024
```

Only the limits actually in effect are listed, so `mem=` and `procs=` appear
here once you set `RIKU_MAX_MEMORY_MB` or `RIKU_MAX_PROCESSES`.

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

Expected output with the defaults (memory and processes left unset):
```
Limit                     Soft Limit           Hard Limit           Units
Max cpu time              3600                 3600                 seconds
Max file size             1073741824           1073741824           bytes
Max core file size        0                    0                    bytes
Max resident set          unlimited            unlimited            bytes
Max processes             unlimited            unlimited            processes
Max open files            1024                 1024                 files
Max address space         unlimited            unlimited            bytes
```

With `RIKU_MAX_MEMORY_MB=512` set, `Max address space` reads `536870912`
instead; with `RIKU_MAX_PROCESSES=64`, `Max processes` reads `64`.

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
- Increase limits if legitimate usage, in the unit's drop-in
  (`sudo systemctl edit riku`):
  ```ini
  Environment="RIKU_MAX_CPU_SECONDS=7200"
  Environment="RIKU_MAX_MEMORY_MB=1024"
  ```
  then `sudo systemctl restart riku`.

### File Operations Failing

**Symptom:** App logs show "Too many open files" errors

**Solution:** add `Environment="RIKU_MAX_OPEN_FILES=2048"` to the drop-in,
then `sudo systemctl restart riku`.

### Cannot Fork Worker Processes

**Symptom:** App logs show "Resource temporarily unavailable" on `fork()`

**Solution:** add `Environment="RIKU_MAX_PROCESSES=128"` to the drop-in,
then `sudo systemctl restart riku`.

---

## Security Considerations

### Why These Limits Matter

1. **Prevents DoS** - Malicious or buggy apps cannot exhaust host resources
2. **Fair Sharing** - Resources are distributed fairly among apps
3. **Predictable Behavior** - Apps have defined resource guarantees
4. **Early Detection** - Resource issues surface quickly during testing

### Best Practices

Do:

1. **Start conservative**: begin with strict limits, relax as needed.
2. **Monitor usage**: check the `/metrics` endpoint for resource usage.
3. **Test limits**: deploy to staging first with production limits.
4. **Log violations**: review logs for apps hitting limits.

Don't:

1. **Disable limits**: always enforce them in multi-tenant environments.
2. **Enable core dumps**: they are a security risk in production.

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

If resource limits cause issues, temporarily relax them with very high
values in the drop-in, then `sudo systemctl restart riku`:

```ini
Environment="RIKU_MAX_MEMORY_MB=16384"
Environment="RIKU_MAX_CPU_SECONDS=86400"
Environment="RIKU_MAX_OPEN_FILES=65536"
Environment="RIKU_MAX_PROCESSES=1024"
```

---

## Future Enhancements

Planned improvements:

- [ ] **Automatic Scaling** - Increase limits based on metrics
- [ ] **Limit Notifications** - Alert when apps approach limits

---

## References

- [setrlimit(2) man page](https://man7.org/linux/man-pages/man2/setrlimit.2.html)
- [Resource limit kernel documentation](https://www.kernel.org/doc/Documentation/scheduler/sched-bwc.txt)
- [nix crate documentation](https://docs.rs/nix/latest/nix/sys/resource/)
