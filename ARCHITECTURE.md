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

Handles application deployment and runtime detection:

- **Runtime Detection**: Identifies application runtime from marker files
- **Build Process**: Executes build steps for each runtime
- **Worker Configuration**: Creates TOML-based worker configurations
- **Process Spawning**: Generates configurations for supervisor

#### Supported Runtimes
- Python (pip, Poetry, uv)
- Node.js (npm, yarn)
- Ruby (Bundler)
- Go (go modules, godeps)
- Java (Maven, Gradle)
- Clojure (tools.deps, Leiningen)
- Rust (Cargo)
- Generic (identity-style deployments)

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

### 6. Plugin System (`plugins.rs`)

Provides extensibility through external executables:

- **Discovery**: Scans `~/.piku/plugins/` for executable files
- **Execution**: Runs plugins as subcommands
- **Environment Access**: Provides access to Riku environment and app data

### 7. Utility Functions (`util.rs`)

Common utility functions used throughout the system:

- **String Processing**: Name sanitization, environment variable expansion
- **File Operations**: Configuration parsing, settings management
- **Network Utilities**: Free port detection
- **System Utilities**: Process execution, requirement checking

## Data Flow

### Application Deployment Flow

1. **Git Hook Trigger**: Git post-receive hook receives new commits
2. **Code Checkout**: Code is checked out to application directory
3. **Runtime Detection**: System detects application runtime from marker files
4. **Build Process**: Appropriate build process is executed
5. **Worker Config Creation**: TOML configurations are generated
6. **Supervisor Activation**: Configurations are symlinked to enable directory
7. **Process Start**: Supervisor detects new configs and starts processes
8. **Nginx Update**: Nginx configuration is regenerated and reloaded

### Process Management Flow

1. **Config File Creation**: Deployment creates TOML worker config
2. **Symlink Creation**: Config is symlinked to `uwsgi-enabled/`
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

- Applications run as the deploy user
- Limited system access for application processes
- Resource limits where possible

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

## Deployment Compatibility

### Directory Structure

Maintains full compatibility with Piku's directory structure:

```
~/.piku/
├── apps/               # Application code
├── data/               # Persistent data
├── envs/               # Environment variables
├── repos/              # Git repositories
├── logs/               # Application logs
├── nginx/              # Nginx configurations
├── uwsgi-available/    # Available worker configs
├── uwsgi-enabled/      # Active worker configs
└── plugins/            # Plugin executables
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