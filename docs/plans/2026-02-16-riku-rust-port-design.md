# Riku: Rust Port of Piku — Design Document

**Date:** 2026-02-16
**Status:** Approved

## Goals

- Eliminate Python runtime dependency on the server
- Improve performance
- Drop-in replacement for piku (same directory structure, file formats, git push workflow)
- Replace uWSGI entirely with a native Rust process supervisor

## Decisions

| Decision | Choice |
|----------|--------|
| Language | Rust |
| Scope | Full port (all runtimes, plugins, SSL) |
| Process management | Native Rust supervisor daemon (replaces uWSGI) |
| Binary | Single binary, two modes: CLI + supervisor |
| CLI framework | clap (derive macros) |
| Templates | tera (Jinja2-like, embedded in binary) |
| Error handling | anyhow |
| Git operations | Shell out to git commands |
| Internal worker config | TOML format |
| Plugin system | Shell-based (executable scripts/binaries) |
| Cron jobs | Built-in scheduler in supervisor daemon |
| Compatibility | Backward-compatible with existing ~/.piku installations |
| Location | `riku/` directory in the piku repo |

## Architecture

Single binary with two modes:

```
riku (single binary)
├── CLI commands (clap subcommands)
├── Deployer (runtime detection, build steps)
├── Supervisor daemon (process lifecycle, cron scheduler)
├── Nginx config generator (tera templates)
└── Git integration (shell-out to git)
```

## Project Structure

```
riku/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, clap CLI definition
│   ├── cli/
│   │   ├── mod.rs           # CLI subcommand routing
│   │   ├── apps.rs          # apps, config, deploy, destroy, logs, ps, run, etc.
│   │   ├── git.rs           # git-hook, git-receive-pack, git-upload-pack
│   │   ├── setup.rs         # setup, setup:ssh
│   │   └── scp.rs           # scp wrapper
│   ├── config.rs            # ENV/Procfile/SCALING parsing, path constants
│   ├── deploy/
│   │   ├── mod.rs           # do_deploy() orchestration + runtime detection
│   │   ├── python.rs        # deploy_python, poetry, uv variants
│   │   ├── node.rs          # deploy_node
│   │   ├── ruby.rs          # deploy_ruby
│   │   ├── go.rs            # deploy_go
│   │   ├── rust.rs          # deploy_rust
│   │   ├── java.rs          # deploy_java_maven, deploy_java_gradle
│   │   ├── clojure.rs       # deploy_clojure_cli, deploy_clojure_leiningen
│   │   └── identity.rs      # deploy_identity (pass-through)
│   ├── nginx.rs             # Nginx config generation (tera templates)
│   ├── supervisor/
│   │   ├── mod.rs           # Supervisor daemon main loop
│   │   ├── process.rs       # Process spawning, monitoring, restart logic
│   │   ├── cron.rs          # Cron expression parser + scheduler
│   │   └── config.rs        # Worker TOML config read/write
│   ├── plugins.rs           # Shell-based plugin discovery + execution
│   └── util.rs              # sanitize_app_name, get_free_port, expandvars, etc.
├── templates/
│   ├── nginx.conf.tera
│   ├── nginx_https_only.conf.tera
│   ├── nginx_common.conf.tera
│   ├── nginx_portmap.conf.tera
│   ├── nginx_acme_firstrun.conf.tera
│   ├── nginx_static.conf.tera
│   ├── nginx_cache.conf.tera
│   └── nginx_uwsgi.conf.tera
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI framework |
| `anyhow` | Error handling |
| `tera` | Nginx template rendering |
| `serde` + `toml` | Worker config serialization |
| `nix` | Unix process management (fork, signals, setuid) |
| `regex` | Cron expression parsing, Procfile validation |
| `colored` | Terminal colored output |
| `log` + `env_logger` | Logging |
| `notify` | Filesystem watching (supervisor watches config dir) |

## Supervisor Daemon

Replaces uWSGI Emperor. Started via `riku supervisor`.

### Process Lifecycle

- **Start:** New `.toml` config appears in enabled dir → spawn process
- **Stop:** Config removed → SIGTERM, grace period, SIGKILL
- **Restart:** Config modified → stop old, start new
- **Crash recovery:** Exponential backoff (1s, 2s, 4s... up to 60s), reset after 60s stable

### Directory Watching

Uses `notify` crate (inotify on Linux) to monitor `~/.piku/uwsgi-enabled/` for changes. Same symlink-based activation as uWSGI Emperor — configs are written to `uwsgi-available/` and symlinked to `uwsgi-enabled/` to activate.

### Cron Scheduler

- Parses cron expressions at config load time
- Timer thread checks every second for jobs to fire
- Spawns one-shot processes, logs output

### Worker TOML Config Format

```toml
[worker]
app = "myapp"
kind = "web"
command = "gunicorn app:app --bind 127.0.0.1:5000"
ordinal = 1

[env]
PORT = "5000"
APP = "myapp"

[options]
working_dir = "/home/piku/.piku/apps/myapp"
log_file = "/home/piku/.piku/logs/myapp/web.1.log"
uid = "piku"
gid = "piku"
```

### Signal Handling

- `SIGTERM` / `SIGINT` → graceful shutdown (stop all children, exit)
- `SIGHUP` → reload configs (re-scan enabled directory)

## Nginx Config Generation

Tera templates embedded in binary via `include_str!()`. Variable context built from app environment. Same templates as Python version:

- Base HTTP+HTTPS with ACME challenge
- HTTPS-only redirect variant
- Common fragment (gzip, headers, SSL, security)
- Reverse proxy upstream blocks
- ACME first-run minimal config
- Static file alias mapping
- Cache path + location blocks

SSL provisioning unchanged: try `acme.sh` → fall back to self-signed via `openssl`. Config validated with `nginx -t` before deploying.

## Deploy Pipeline

Identical flow to Python version:

1. `git fetch` + `git reset` + submodule init
2. Parse Procfile
3. Run `preflight` worker (if defined)
4. Detect runtime by marker files → call runtime-specific deployer
5. Run `release` worker (if defined)
6. Call `spawn_app()` → generate worker TOML configs + nginx config

### Runtime Detection

| Marker File | Runtime |
|-------------|---------|
| `requirements.txt` | Python (pip) |
| `pyproject.toml` + `poetry` | Python (Poetry) |
| `pyproject.toml` + `uv` | Python (uv) |
| `Gemfile` | Ruby |
| `package.json` | Node.js |
| `pom.xml` | Java Maven |
| `build.gradle` | Java Gradle |
| `go.mod` / `Godeps` / `*.go` | Go |
| `deps.edn` | Clojure CLI |
| `project.clj` | Clojure Leiningen |
| `Cargo.toml` | Rust |

All deployers shell out to their respective toolchains.

## CLI Commands

```
riku apps                          # List deployed apps
riku config <app>                  # Show app config
riku config get <app> <key>        # Get single setting (alias: config:get)
riku config set <app> KEY=VAL...   # Set env vars, triggers redeploy
riku config unset <app> KEY...     # Remove env vars, triggers redeploy
riku config live <app>             # Show live running config
riku deploy <app>                  # Force redeploy
riku destroy <app>                 # Remove app (preserves data dir)
riku logs <app> [process]          # Tail logs
riku ps <app>                      # Show process count
riku ps scale <app> web=N...       # Scale workers (alias: ps:scale)
riku run <app> <cmd...>            # Run command in app context
riku restart <app>                 # Restart app
riku stop <app>                    # Stop app
riku setup                         # Initialize ~/.piku directory structure
riku setup ssh <pubkey>            # Add SSH key (alias: setup:ssh)
riku update                        # Self-update binary
riku supervisor                    # Start process supervisor daemon
riku help                          # Display help

# Internal (git hooks)
riku git-hook <app>
riku git-receive-pack <app>
riku git-upload-pack <app>
riku scp <args...>
```

Colon-form aliases (`config:set`, `ps:scale`, `setup:ssh`) maintained for backward compatibility.

## Plugin System

Shell-based plugins: riku scans `~/.piku/plugins/` for executable files/scripts. Each becomes a subcommand via clap's `allow_external_subcommands`. Language-agnostic — plugins can be bash scripts, Python scripts, compiled binaries, etc.

## Backward Compatibility

- Same `~/.piku` directory structure
- Same `ENV` file format (`KEY=VALUE` with `$VAR` expansion)
- Same `Procfile` format
- Same `SCALING` file format (`kind:count`)
- Same SSH `authorized_keys` format (points to `riku` binary)
- Same git bare repo structure with post-receive hooks

### Breaking Change

Apps using `wsgi:`, `jwsgi:`, or `rwsgi:` Procfile entries must switch to `web:` with an explicit server command:

```
# Before (piku + uWSGI)
wsgi: myapp:application

# After (riku)
web: gunicorn myapp:application --bind unix:///path/to/sock
```

Riku prints a helpful migration message when it detects old-style entries.

## Migration Path

1. Install `riku` binary on the server
2. Run `riku setup` (idempotent)
3. Update `~/.ssh/authorized_keys` to point to `riku`
4. Start `riku supervisor` instead of uWSGI emperor
5. Update any `wsgi:`/`jwsgi:`/`rwsgi:` Procfile entries to `web:` commands
