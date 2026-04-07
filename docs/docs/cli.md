# CLI Reference

Complete reference for all Riku commands.

## Usage

```bash
riku <command> [subcommand] [options]
```

## Application Management

### `riku apps`

List all deployed applications.

```bash
riku apps
```

**Output:**
```
myapp      web=2 worker=1    running
test-app   web=1             stopped
```

---

### `riku deploy <app>`

Force redeploy an application.

```bash
riku deploy myapp
```

This resets the working directory, detects the runtime, installs dependencies, and spawns workers.

---

### `riku destroy <app>`

Permanently remove an application and all its data.

```bash
riku destroy myapp
```

**Warning:** This deletes:
- Application code in `~/.riku/apps/<app>/`
- Environment variables in `~/.riku/envs/<app>/`
- Git repository in `~/.riku/repos/<app>/`
- Logs in `~/.riku/logs/<app>/`
- Nginx configuration
- Worker configurations

**Note:** `~/.riku/data/<app>/` and `~/.riku/cache/<app>/` are **preserved** by destroy.

---

### `riku logs <app> [process]`

Tail application logs.

```bash
# All logs
riku logs myapp

# Specific process
riku logs myapp web
riku logs myapp worker
```

Logs are stored in `~/.riku/logs/<app>/` and automatically rotated at 10MB.

---

### `riku restart <app>`

Restart all processes for an application.

```bash
riku restart myapp
```

Gracefully stops workers and restarts them. For zero-downtime rolling restarts, use `riku restart myapp --hot`.

---

### `riku stop <app>`

Stop all processes for an application.

```bash
riku stop myapp
```

To restart, use `riku restart <app>` or push a new commit.

---

### `riku run <app> <command>`

Execute a command in the application's environment.

```bash
riku run myapp python manage.py migrate
riku run myapp npm run build
riku run myapp bash
```

Environment variables from `~/.riku/envs/<app>/ENV` are loaded automatically.

---

## Configuration Management

### `riku config show <app>`

Show application configuration.

```bash
riku config show myapp
```

**Output:**
```
DATABASE_URL=postgres://localhost/mydb
NGINX_SERVER_NAME=example.com
RIKU_WORKER_TIMEOUT=3600
```

---

### `riku config get <app> <key>`

Get a specific configuration value.

```bash
riku config get myapp DATABASE_URL
```

---

### `riku config set <app> KEY=value...`

Set configuration values.

```bash
riku config set myapp DATABASE_URL=postgres://localhost/mydb
riku config set myapp KEY1=value1 KEY2=value2
```

Multiple key-value pairs can be set in one command.

---

### `riku config unset <app> KEY...`

Remove configuration values.

```bash
riku config unset myapp DATABASE_URL
riku config unset myapp KEY1 KEY2
```

---

### `riku config live <app>`

Show live running configuration (from worker TOML files).

```bash
riku config live myapp
```

Shows the actual configuration being used by the supervisor, including defaults.

---

---

## Process Management

### `riku ps <app>`

Show process counts and status.

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

### `riku ps <app> --scale web=N worker=N...`

Scale worker processes.

```bash
riku ps myapp --scale web=4 worker=2
```

This creates/updates the `SCALING` file and triggers a restart.

**Alternative:** Create a `SCALING` file directly:
```bash
echo "web=4" > SCALING
echo "worker=2" >> SCALING
git add SCALING && git commit -m "scale up"
git push riku main
```

---

## Setup Commands

### `riku init`

Initialize the Riku directory structure and install the binary.

```bash
riku init
```

Creates:
- `~/.riku/apps/` - Application code
- `~/.riku/repos/` - Git bare repositories
- `~/.riku/envs/` - Environment variables
- `~/.riku/logs/` - Application logs
- `~/.riku/nginx/` - Nginx configurations
- `~/.riku/cache/` - Nginx cache files
- `~/.riku/workers-available/` - Worker configs
- `~/.riku/workers-enabled/` - Enabled worker symlinks
- `~/.riku/plugins/` - Plugin executables
- `~/.riku/acme/` - SSL certificates

Also installs the riku binary to `~/.local/bin/riku` and generates a systemd user service.

---

## System Commands

### `riku supervisor`

Start the process supervisor daemon.

```bash
riku supervisor
```

The supervisor:
- Monitors worker TOML configurations
- Spawns and manages application processes
- Handles graceful shutdowns
- Performs health checks
- Auto-restarts failed processes

**Run as a systemd user service:**
```bash
systemctl --user enable riku
systemctl --user start riku
```

---

### `riku update`

Update the Riku binary to the latest version.

```bash
riku update
```

Downloads and replaces the current binary from the official release.

---

### `riku --help`

Show help information.

```bash
riku --help
riku <command> --help
```

---

### `riku --version`

Show version information.

```bash
riku --version
```

---

## Git Integration

### Post-receive Hook

When you `git push riku main`, the post-receive hook:
1. Checks out code to `~/.riku/apps/<app>/`
2. Detects the runtime
3. Installs dependencies
4. Generates nginx config
5. Creates worker configs
6. Notifies the supervisor

No manual intervention required.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | App not found |
| 4 | Configuration error |
| 5 | Permission denied |

---

## Environment Variables (CLI)

These affect CLI behavior:

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Set log level: `debug`, `info`, `warn`, `error` |
| `RIKU_ROOT` | Override default `~/.riku` location |
| `RIKU_DEBUG` | Enable debug output |

**Example:**
```bash
RUST_LOG=debug riku deploy myapp
```

---

## Examples

### Deploy and configure an app

```bash
# Create app directory locally
mkdir myapp && cd myapp
git init

# Add code and Procfile
echo 'web: python app.py' > Procfile
git add . && git commit -m "init"

# Add remote and deploy
git remote add riku deploy@server:myapp
git push riku main

# Configure
riku config set myapp DATABASE_URL=postgres://localhost/db
riku ps myapp --scale web=2

# Monitor
riku logs myapp
riku ps myapp
```

### Manage environment variables

```bash
# Set multiple vars
riku config set myapp \
  DATABASE_URL=postgres://localhost/db \
  SECRET_KEY=supersecret \
  DEBUG=false

# Get a value
riku config get myapp DATABASE_URL

# Remove a var
riku config unset myapp DEBUG

# View all
riku config show myapp
```

### Scale and restart

```bash
# Scale up
riku ps myapp --scale web=4 worker=2

# Restart after config change
riku restart myapp

# Stop for maintenance
riku stop myapp

# Start again
riku restart myapp
```
