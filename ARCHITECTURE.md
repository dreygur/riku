# Riku Architecture Design Document

## Overview

Riku is a complete Rust port of the Piku micro-PaaS, designed to provide Heroku-like git push deployments to small servers without Docker. This document outlines the architecture, design decisions, and implementation details of the Riku system.

## Goals

1. **Performance**: Replace Python runtime with efficient Rust implementation
2. **Compatibility**: Maintain full compatibility with existing Piku workflows
3. **Reliability**: Improve stability and error handling
4. **Maintainability**: Provide cleaner, more efficient codebase
5. **Extensibility**: Support plugin system and new runtimes

## High-Level Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Git Client    │───▶│   Riku Server    │───▶│  Applications   │
│                 │    │                  │    │                 │
│  git push       │    │  ┌─────────────┐ │    │  Managed by     │
│  (deploys)      │    │  │ Supervisor  │ │    │  Supervisor     │
└─────────────────┘    │  │ (daemon)    │ │    │                 │
                       │  └─────────────┘ │    └─────────────────┘
                       │                  │
                       │  ┌─────────────┐ │
                       │  │ Nginx       │ │
                       │  │ (reverse    │ │
                       │  │  proxy)     │ │
                       │  └─────────────┘ │
                       └──────────────────┘
```

## Component Architecture

### 1. CLI Layer (`main.rs`, `cli/`)

The CLI layer handles user commands and orchestrates operations:

- **Command Parsing**: Uses `clap` for robust command-line argument parsing
- **Command Routing**: Routes commands to appropriate handlers
- **Input Validation**: Validates user inputs before processing
- **Error Handling**: Provides user-friendly error messages

### 2. Configuration System (`config.rs`)

Manages all path configurations and system settings:

- **Path Resolution**: Resolves all directory paths based on environment
- **Environment Variables**: Honors `$RIKU_ROOT` and `$HOME`
- **Directory Structure**: Maintains compatibility with Piku's directory layout
- **Default Values**: Provides sensible defaults for all paths

### 3. Deployment Engine (`deploy/`)

Handles application deployment by orchestrating the plugin-based runtime system:

- **Plugin Discovery**: Scans `~/.riku/plugins/` for runtime plugin executables
- **Runtime Detection**: Delegates detection to plugins via `detect` subcommand (exit 0 = match)
- **Build Dispatch**: Calls `build` on the matched plugin; streams stdout/stderr to deploy log
- **Environment Merging**: Calls `env` on plugin; merges `KEY=VALUE` output into worker env
- **Worker Configuration**: Creates TOML-based worker configurations for supervisor
- **Start Command Fallback**: Uses plugin `start` output if Procfile has no command for a process type

#### Plugin-Based Runtime Dispatch

Runtime detection resolution:
1. If `RUNTIME=<name>` is in the app ENV → use that plugin directly
2. Otherwise → run `detect` on all non-`riku-*` plugins sorted alphabetically; first exit 0 wins
3. If no plugin matches → deploy fails with a clear error

Plugins receive context via environment variables: `RIKU_APP`, `RIKU_APP_PATH`,
`RIKU_ENV_PATH`, `RIKU_ROOT`.

#### Bundled Runtime Plugins
- `node` — Node.js (npm/yarn/pnpm), detects `package.json`
- `python` — Python (pip/Poetry/uv), detects `requirements.txt` / `pyproject.toml`
- `ruby` — Ruby (Bundler), detects `Gemfile`
- `go` — Go (modules/godeps), detects `go.mod` / `Godeps` / `.go` files
- `rust-lang` — Rust (Cargo), detects `Cargo.toml` + `rust-toolchain.toml`
- `riku-plugin-java` — Java (Maven/Gradle), detects `pom.xml` / `build.gradle`
- `riku-plugin-clojure` — Clojure (Lein/deps.edn), detects `project.clj` / `deps.edn`
- `riku-plugin-container` — Docker/Podman, detects `Dockerfile` / `Containerfile` / `docker-compose.yml`

### 4. Process Supervisor (`supervisor/`)

Manages application processes and provides process lifecycle management:

- **Process Management**: Spawns, monitors, and manages application processes
- **File Watching**: Watches TOML configuration files for changes
- **Automatic Restart**: Restarts processes when configurations change
- **Health Monitoring**: Checks process health and restarts failed processes
- **Log Rotation**: Automatically rotates logs based on size and retention
- **Cron Scheduling**: Manages scheduled cron jobs from Procfile

#### Supervisor Modules
- `config.rs` - Worker configuration and health check settings
- `process.rs` - Process spawning and lifecycle management
- `cron.rs` - Cron expression parsing and job scheduling
- `log_rotation.rs` - Log file rotation and cleanup
- **Graceful Shutdown**: Handles process termination signals properly

#### Supervisor Components
- **Process Manager**: Core process management functionality
- **Configuration Watcher**: Monitors configuration changes
- **Cron Scheduler**: Manages scheduled tasks
- **Signal Handlers**: Handles OS signals (SIGTERM, SIGINT, SIGHUP)

### 5. Nginx Integration (`nginx.rs`)

Generates nginx configurations for applications:

- **Template System**: Uses Tera templating engine for flexible configs
- **Multiple Config Types**: Supports HTTP, HTTPS, static, port mapping, etc.
- **ACME Integration**: Handles Let's Encrypt certificate challenges
- **Validation**: Validates generated configurations before applying

### 6. Plugin System (`plugins/`)

Provides extensibility through external executables in `~/.riku/plugins/`:

- **Runtime plugins** (`plugins/runtime.rs`): discover, detect, build, get_env, get_start_cmd — full runtime dispatch used by the deployment engine
- **Lifecycle hook plugins** (`plugins/manager.rs`): `riku-pre-deploy`, `riku-pre-build`, `riku-post-build`, `riku-post-deploy` — run at deploy lifecycle stages
- **Execution** (`plugins/executor.rs`): timeout-aware process spawning with environment injection
- **Naming convention**: runtime plugins are non-`riku-*` executables; lifecycle hooks are `riku-*` executables; both live in the same `plugins/` directory

### 7. Utility Functions (`util.rs`)

Common utility functions used throughout the system:

- **String Processing**: Name sanitization, environment variable expansion
- **File Operations**: Configuration parsing, settings management
- **Network Utilities**: Free port detection
- **System Utilities**: Process execution, requirement checking

## Data Flow

### Application Deployment Flow

1. **Git Hook Trigger**: Git post-receive hook receives new commits. First
   push for a new app is detected by checking for a `HEAD` file inside the
   target bare repo (`riku_repo.join("HEAD").exists()` in
   `src/cli/git/receive_pack.rs`), not by checking whether the repo
   directory itself exists — a plain directory-existence check is
   unreliable here because the hooks/ subdirectory created later in the
   same code path would otherwise make the repo appear to "exist" before
   `git init --bare` has actually run, silently skipping initialization.
2. **Code Checkout**: Code is checked out to application directory
3. **Procfile Parsing**: Procfile is parsed; empty/missing Procfile aborts deploy
4. **ENV Loading**: App environment variables loaded from `~/.riku/envs/<app>/ENV`
5. **pre-deploy hook**: `riku-pre-deploy` plugin runs (failure aborts deploy)
6. **Plugin Discovery**: Scan `~/.riku/plugins/` for non-`riku-*` executables
7. **Runtime Detection**: `RUNTIME=` override or first `detect`-exit-0 plugin wins; no match = error
8. **pre-build hook**: `riku-pre-build` plugin runs
9. **Build**: Plugin `build` subcommand runs; output streamed to deploy log
10. **Env merge**: Plugin `env` output merged into app environment
11. **post-build hook**: `riku-post-build` plugin runs
12. **Worker Config Creation**: TOML configurations generated using Procfile + plugin start command
13. **Supervisor Activation**: Configurations symlinked to `workers-enabled/`
14. **Process Start**: Supervisor detects new configs and starts processes
15. **Nginx Update**: Nginx configuration regenerated and reloaded
16. **post-deploy hook**: `riku-post-deploy` plugin runs

### Process Management Flow

1. **Config File Creation**: Deployment creates TOML worker config
2. **Symlink Creation**: Config is symlinked to `workers-enabled/`
3. **File Watcher Notification**: Supervisor detects file system change
4. **Process Spawn**: Supervisor spawns new process based on config
5. **Health Monitoring**: Supervisor continuously monitors process health
6. **Automatic Recovery**: Failed processes are automatically restarted
7. **Config Updates**: Modified configs trigger process restarts
8. **Cleanup**: Removed configs cause processes to be stopped

## Configuration Formats

### Worker Configuration (TOML)

```toml
[worker]
app = "myapp"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "5000"
DATABASE_URL = "sqlite:///db.sqlite3"

[options]
working_dir = "/home/piku/.piku/apps/myapp"
log_file = "/home/piku/.piku/logs/myapp/web.1.log"
```

### Scaling Configuration

```
web=2
worker=4
```

### Environment Configuration

```
KEY1=VALUE1
KEY2=VALUE2
```

## Security Model

### SSH Access Control

- SSH key restrictions prevent shell access
- Commands are restricted to Riku operations only
- Public keys are added with command restrictions

### Process Isolation

- Applications run as the deploy user, never as root
- Limited system access for application processes
- Resource limits where possible

### Unprivileged Worker / Nginx Interaction Model

- The `riku supervisor` daemon and all spawned application workers run
  entirely as the unprivileged deploy user (e.g. `riku`) — the daemon is
  never started as root in a correctly configured deployment.
- Nginx's master process runs as root by OS package default, which the
  deploy user cannot signal directly. Riku does not solve this by running
  itself as root; instead, the deploy user is granted a narrowly-scoped
  passwordless sudo rule limited to the `nginx` binary itself (config test
  and reload only — no shell, no other commands), reached through a
  `nginx` wrapper placed ahead of `/usr/sbin/nginx` in `PATH`. This keeps
  the supervisor and every application process fully unprivileged while
  still allowing config reloads to take effect.
- The per-app nginx vhost symlink directory (`/etc/nginx/sites-enabled/`)
  is made group-writable by the deploy user rather than granting broader
  filesystem privileges.
- See `tests/stress/container/sudoers-riku-nginx` and
  `tests/stress/container/nginx-wrapper.sh` for the reference
  implementation, verified end-to-end in the containerized integration
  suite (see Testing Strategy below).

### Input Validation

- All app names are sanitized
- File paths are validated to prevent directory traversal
- Command injection prevention

## Performance Characteristics

### Memory Usage

- Low memory footprint compared to Python implementation
- Efficient data structures minimize allocations
- Supervisor only loads active configurations

### Startup Time

- Fast startup due to compiled binary
- Supervisor initializes quickly
- Process spawning is efficient

### Concurrency

- Supervisor uses file watching instead of polling
- Process monitoring is event-driven
- Minimal system resource usage

## Error Handling Strategy

### Graceful Degradation

- System continues operating when individual apps fail
- Configuration errors don't affect other applications
- Partial deployments are handled gracefully

### Logging

- Colored output for different log levels
- Error messages provide actionable information
- Process logs are stored in application-specific directories

### Recovery

- Automatic process restart with exponential backoff
- Configuration rollback capabilities
- Supervisor restart on crashes

## Testing Strategy

### Unit Tests

- Comprehensive coverage of utility functions
- Runtime detection logic testing
- Configuration parsing validation
- Edge case handling

### Integration Tests

- Full deployment workflows
- Process lifecycle management
- Configuration updates
- Error condition handling

### Test Coverage

- Target 80%+ code coverage
- Property-based testing where appropriate
- Mock external dependencies for isolation

### Containerized Production Integration Suite

`tests/stress/container/run_container_test.sh` builds a real
target server image (Ubuntu 24.04, sshd, nginx, the compiled `riku`
binary, bundled runtime plugins), provisions a throwaway SSH keypair,
boots the container, performs an actual `git push` deploy of a mock app
over SSH, then drives concurrent HTTP load against the nginx-proxied
app and collects a structured pass/fail verdict (502/504 count, zombie
process check, supervisor liveness). It runs with either Docker or
Podman — the script detects whichever is on `PATH` and uses it
transparently, no flags needed:

```bash
./tests/stress/container/run_container_test.sh
```

Latest verified run: 14,530/14,530 requests succeeded (zero 502/504s)
under 80 concurrent workers for 30s, supervisor remained alive, zero
zombie processes. See `tests/stress/README.md` for the full
suite (lifecycle stress, fd/leak monitor, chaos signal tests, resource
limit audit) and `tests/stress/container/` for this
containerized suite specifically.

## Deployment Compatibility

### Directory Structure

```
~/.riku/
├── apps/               # Application code (checked-out source)
├── data/               # Persistent data
├── envs/               # Environment variables (<app>/ENV, <app>/LIVE_ENV)
├── repos/              # Git bare repositories
├── logs/               # App logs (<app>/deploy.log, <app>/web.1.log, …)
├── nginx/              # Nginx configurations
├── cache/              # Nginx cache files
├── workers/            # Worker process configurations
├── workers-available/  # Available worker TOML configs
├── workers-enabled/    # Enabled worker configs (symlinks)
├── acme/               # ACME/Let's Encrypt certificates
└── plugins/            # Plugin executables (runtime + lifecycle hooks)
```

### File Formats

- Procfile support for process definitions
- SCALING file for process counts
- ENV files for environment variables
- Standard git hooks for deployment

## Future Extensions

### Planned Features

- Green/blue deployment support
- Advanced monitoring and metrics
- Container runtime support
- Multi-server clustering
- Advanced plugin API

### Extensibility Points

- Plugin system for custom functionality
- Runtime system for new languages
- Configuration templates for nginx
- Custom process supervisors

## Implementation Notes

### Rust-Specific Decisions

- **Error Handling**: Using `anyhow` for application errors
- **CLI Framework**: Using `clap` for robust argument parsing
- **Templating**: Using `tera` for flexible configuration generation
- **File Watching**: Using `notify` for efficient file system monitoring
- **Process Management**: Using `nix` for Unix process operations

### Performance Optimizations

- Minimal allocations in hot paths
- Efficient string operations
- Lazy configuration loading
- Event-driven architecture

This architecture provides a solid foundation for a high-performance, reliable micro-PaaS implementation while maintaining full compatibility with the existing Piku ecosystem.