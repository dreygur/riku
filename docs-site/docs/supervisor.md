# Process Supervisor

Riku includes a custom process supervisor written in Rust that manages application processes. It replaces uWSGI Emperor from the original Piku.

---

## Overview

The supervisor:

- Monitors worker TOML configurations in `~/.riku/workers-enabled/`
- Spawns and manages application processes
- Performs health checks
- Auto-restarts failed processes
- Handles graceful shutdowns
- Supports cron job scheduling

---

## Architecture

```
┌─────────────────┐
│   Supervisor    │
│    (Daemon)     │
└────────┬────────┘
         │
    ┌────┴────┐
    │  File   │
    │ Watcher │
    └────┬────┘
         │
    ┌────┴─────────────────────────────┐
    │  ~/.riku/workers-enabled/        │
    │  ├── myapp-web.toml              │
    │  ├── myapp-worker.toml           │
    │  └── myapp-cron.toml             │
    └──────────────────────────────────┘
         │
    ┌────┴────┐
    │Process  │
    │Manager  │
    └────┬────┘
         │
    ┌────┴─────────────────────────────┐
    │  Running Processes               │
    │  ├── web.1 (PID 1234)            │
    │  ├── web.2 (PID 1235)            │
    │  ├── worker.1 (PID 1236)         │
    │  └── cron (PID 1237)             │
    └──────────────────────────────────┘
```

---

## Starting the Supervisor

### Manual Start

```bash
riku supervisor
```

Runs in the foreground. Press `Ctrl+C` to stop.

### As a Systemd Service

Create `~/.config/systemd/user/riku.service`:

```ini
[Unit]
Description=Riku Process Supervisor
After=network.target

[Service]
Type=simple
ExecStart=%h/.riku/riku supervisor
Restart=always
Environment=PATH=/usr/local/bin:/usr/bin:/bin

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable riku
systemctl --user start riku
```

Check status:

```bash
systemctl --user status riku
```

---

## Worker Configuration

Worker configs are TOML files in `~/.riku/workers-enabled/`.

### Config Structure

```toml
[worker]
app = "myapp"
kind = "web"
command = "python app.py"
processes = 2
port = 5000

[env]
DATABASE_URL = "postgres://localhost/mydb"
SECRET_KEY = "supersecret"

[options]
timeout = 3600
grace_period = 60
```

### Config Fields

#### `[worker]` Section

| Field | Description | Required |
|-------|-------------|----------|
| `app` | Application name | Yes |
| `kind` | Process type (`web`, `worker`, `cron`) | Yes |
| `command` | Command to execute | Yes |
| `processes` | Number of processes to spawn | No (default: 1) |
| `port` | Port to bind to | No (auto-assigned) |

#### `[env]` Section

Environment variables for the process. Inherited from `~/.riku/envs/<app>/ENV`.

#### `[options]` Section

| Field | Description | Default |
|-------|-------------|---------|
| `timeout` | Health check timeout (seconds) | 3600 |
| `grace_period` | Graceful shutdown period (seconds) | 60 |
| `restart_on_failure` | Auto-restart on failure | true |
| `max_restarts` | Max restart attempts | 10 |

---

## Scaling

### Using SCALING File

Create a `SCALING` file in your app root:

```
web=2
worker=4
```

Commit and push:

```bash
git add SCALING && git commit -m "scale up"
git push riku master
```

### Using CLI

```bash
riku ps scale myapp web=4 worker=2
```

### Using Environment Variable

```bash
riku config:set myapp RIKU_WORKER_PROCESSES="web=4,worker=2"
```

---

## Health Checks

The supervisor performs periodic health checks on managed processes.

### HTTP Health Checks

For web processes, the supervisor checks if the port is listening.

### Process Health

- Monitors if process is running
- Checks for zombie processes
- Tracks memory usage (optional)

### Auto-Restart

When a process fails:

1. Supervisor attempts restart (up to `RIKU_MAX_RESTARTS`)
2. Logs restart attempt
3. Marks as failed if max restarts exceeded

Configure restart behavior:

```bash
riku config:set myapp RIKU_WORKER_TIMEOUT=3600
riku config:set myapp RIKU_MAX_RESTARTS=10
```

---

## Graceful Shutdown

When stopping or restarting:

1. Send `SIGTERM` to process
2. Wait for `RIKU_WORKER_GRACE_PERIOD` seconds
3. If still running, send `SIGKILL`

This ensures:
- In-flight requests complete
- Database connections close properly
- No data loss during shutdown

---

## Cron Jobs

Cron jobs are defined in the Procfile and managed by the supervisor.

### Procfile Syntax

```
cron: 0 2 * * * /path/to/script.sh
```

### Cron Expression Format

```
* * * * *
│ │ │ │ │
│ │ │ │ └─ Day of week (0-7, Sunday=0 or 7)
│ │ │ └─── Month (1-12)
│ │ └───── Day of month (1-31)
│ └─────── Hour (0-23)
└───────── Minute (0-59)
```

### Examples

```
# Every minute
cron: * * * * * /path/to/every-minute.sh

# Every hour at minute 0
cron: 0 * * * * /path/to/hourly.sh

# Every day at 2:30 AM
cron: 30 2 * * * /path/to/daily.sh

# Every Monday at 9:00 AM
cron: 0 9 * * 1 /path/to/weekly.sh

# Every 15 minutes
cron: */15 * * * * /path/to/every-15-min.sh
```

### Multiple Cron Jobs

```
cron: 0 * * * * /path/to/hourly.sh
cron-daily: 0 2 * * * /path/to/daily.sh
cron-weekly: 0 9 * * 1 /path/to/weekly.sh
```

---

## Log Rotation

The supervisor handles log rotation automatically.

### Rotation Settings

- **Max size:** 10MB (default)
- **Max files:** 5 rotated logs
- **Format:** `app-process.log`, `app-process.log.1`, etc.

### Configure Rotation

```bash
riku config:set myapp LOG_ROTATION_SIZE=5242880  # 5MB
```

### View Logs

```bash
# Tail all logs
riku logs myapp

# Tail specific process
riku logs myapp web

# View rotated logs
ls -la ~/.riku/logs/myapp/
```

---

## Process States

| State | Description |
|-------|-------------|
| `running` | Process is running and healthy |
| `starting` | Process is being spawned |
| `stopping` | Process is being stopped |
| `failed` | Process failed to start or crashed |
| `restarting` | Process is being restarted |

### Check Process Status

```bash
riku ps myapp
```

**Output:**
```
web:     2/2 running
worker:  1/1 running
cron:    1/1 running
```

---

## Manual Process Management

### Start Processes

```bash
riku restart myapp
```

### Stop Processes

```bash
riku stop myapp
```

### Restart Specific Process

```bash
riku ps restart myapp web
```

### View Running Processes

```bash
# Via Riku
riku ps myapp

# Via system
ps aux | grep myapp
```

---

## Troubleshooting

### Process Won't Start

1. **Check logs:**
   ```bash
   riku logs myapp
   ```

2. **Verify command:**
   ```bash
   riku config live myapp
   ```

3. **Check port:**
   ```bash
   netstat -tlnp | grep myapp
   ```

### Process Keeps Restarting

1. **Check crash reason:**
   ```bash
   riku logs myapp
   ```

2. **Increase timeout:**
   ```bash
   riku config:set myapp RIKU_WORKER_TIMEOUT=7200
   ```

3. **Check resource limits:**
   ```bash
   free -h
   df -h
   ```

### Cron Job Not Running

1. **Verify cron expression:**
   ```bash
   cat ~/.riku/workers-enabled/myapp-cron.toml
   ```

2. **Check supervisor logs:**
   ```bash
   journalctl --user -u riku
   ```

3. **Test command manually:**
   ```bash
   riku run myapp /path/to/script.sh
   ```

### High Memory Usage

1. **Check process memory:**
   ```bash
   ps aux | grep myapp
   ```

2. **Reduce worker count:**
   ```bash
   riku ps scale myapp web=1
   ```

3. **Set memory limits** (via cgroups or container)

---

## Advanced Configuration

### Custom Health Check

For non-HTTP health checks, create a health check script:

```bash
#!/bin/sh
# ~/.riku/apps/myapp/healthcheck.sh

# Check database connection
psql -h localhost -U myapp -c "SELECT 1" > /dev/null 2>&1
exit $?
```

### Process Priority

Set nice level for processes:

```bash
# In worker config TOML
[options]
nice = 10
```

### Resource Limits

```bash
# In worker config TOML
[options]
rlimit_nofile = 65536
rlimit_nproc = 4096
```

---

## See Also

- [CLI Reference](cli.md) - Process management commands
- [Environment Variables](env.md) - Supervisor configuration
- [Cron Jobs](cron.md) - Detailed cron documentation
