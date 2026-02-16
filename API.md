# Riku API Documentation

## Overview

Riku is a Rust port of the Piku micro-PaaS, providing Heroku-like git push deployments to small servers. This documentation describes the internal API and module structure.

## Module Structure

### `main.rs`
The entry point of the application that parses CLI arguments and routes commands to appropriate handlers.

### `cli/` Module
Handles command-line interface functionality:

#### `cli/mod.rs`
- Defines the CLI structure using clap
- Contains all command definitions
- Routes commands to appropriate handlers

#### `cli/apps.rs`
Application management commands:
- `cmd_apps()` - List deployed applications
- `cmd_config_show()` - Show app configuration
- `cmd_config_get()` - Get single config value
- `cmd_config_set()` - Set config values
- `cmd_config_unset()` - Unset config values
- `cmd_config_live()` - Show live running configuration
- `cmd_deploy()` - Deploy an app
- `cmd_destroy()` - Remove an app
- `cmd_logs()` - Tail app logs
- `cmd_ps_show()` - Show process scaling info
- `cmd_ps_scale()` - Scale workers
- `cmd_run()` - Run command in app context
- `cmd_restart()` - Restart an app
- `cmd_stop()` - Stop an app
- `cmd_update()` - Self-update binary
- `cmd_supervisor()` - Start supervisor daemon

#### `cli/git.rs`
Git integration commands:
- `cmd_git_hook()` - Post-receive git hook handler
- `cmd_git_receive_pack()` - Handle git pushes
- `cmd_git_upload_pack()` - Handle git uploads

#### `cli/scp.rs`
SCP handler:
- `cmd_scp()` - Simple wrapper to allow scp

#### `cli/setup.rs`
Setup and initialization commands:
- `cmd_setup_init()` - Initialize directory structure
- `cmd_setup_ssh()` - Add SSH public key

### `config.rs`
Configuration management:
- `RikuPaths` - Struct containing all resolved directory paths
- `PIKU_RAW_SOURCE_URL` - Raw source URL for fetching the latest script
- `UWSGI_LOG_MAXSIZE` - Maximum log size constant

### `deploy/` Module
Application deployment functionality:

#### `deploy/mod.rs`
Main deployment logic:
- `do_deploy()` - Deploy an app by resetting work directory, detecting runtime, and spawning workers
- `detect_runtime()` - Detect the application runtime by checking marker files
- `spawn_app()` - Spawn application processes based on Procfile and SCALING

#### `deploy/python.rs`
Python application deployment:
- `deploy_python()` - Deploy Python app using pip
- `deploy_python_poetry()` - Deploy Python app using Poetry
- `deploy_python_uv()` - Deploy Python app using uv
- `create_python_worker_config()` - Create worker config for Python process

#### `deploy/node.rs`
Node.js application deployment:
- `deploy_node()` - Deploy Node.js app using npm/yarn
- `create_node_worker_config()` - Create worker config for Node.js process

#### `deploy/ruby.rs`
Ruby application deployment:
- `deploy_ruby()` - Deploy Ruby app using Bundler
- `create_ruby_worker_config()` - Create worker config for Ruby process

#### `deploy/go.rs`
Go application deployment:
- `deploy_go()` - Deploy Go app
- `create_go_worker_config()` - Create worker config for Go process

#### `deploy/java.rs`
Java application deployment:
- `deploy_java_maven()` - Deploy Java app using Maven
- `deploy_java_gradle()` - Deploy Java app using Gradle
- `create_java_worker_config()` - Create worker config for Java process

#### `deploy/clojure.rs`
Clojure application deployment:
- `deploy_clojure_cli()` - Deploy Clojure app using tools.deps
- `deploy_clojure_lein()` - Deploy Clojure app using Leiningen
- `create_clojure_worker_config()` - Create worker config for Clojure process

#### `deploy/identity.rs`
Generic application deployment:
- `deploy_identity()` - Deploy identity-style applications
- `create_identity_workers()` - Create worker configs for identity-style deployments

### `nginx.rs`
Nginx configuration generation:
- `generate_nginx_config()` - Generate nginx configuration for an app
- `remove_nginx_config()` - Remove nginx configuration for an app
- `generate_acme_nginx_config()` - Generate minimal nginx config for ACME challenges
- `validate_nginx_config()` - Validate nginx configuration file

### `supervisor/` Module
Process supervision functionality:

#### `supervisor/mod.rs`
Main supervisor daemon:
- `Supervisor` - Main supervisor daemon struct
- `cmd_supervisor()` - Start the supervisor daemon
- `setup_signal_handlers()` - Setup signal handlers for graceful shutdown

#### `supervisor/process.rs`
Process management:
- `SpawnedProcess` - Represents a spawned application process
- `ProcessManager` - Manages the lifecycle of application processes
- `spawn_process()` - Spawn a new process based on worker configuration
- `stop_app_processes()` - Stop all processes for a specific app
- `check_processes()` - Check status of all managed processes

#### `supervisor/cron.rs`
Cron scheduling:
- `CronJob` - A scheduled cron job
- `CronScheduler` - Cron scheduler that manages and executes scheduled jobs
- `validate_cron_expression()` - Validate a cron expression
- `calculate_next_run()` - Parse cron expression and calculate next run time

#### `supervisor/config.rs`
Worker configuration:
- `WorkerConfig` - The main worker configuration structure
- `create_worker_config()` - Create a worker config from app name, kind, command, and environment

### `plugins.rs`
Plugin system:
- `run_plugin()` - Execute a plugin
- `list_plugins()` - Scan plugins directory and return available plugins
- `plugin_exists()` - Check if a plugin exists and is executable

### `util.rs`
Utility functions:
- `sanitize_app_name()` - Sanitize the app name
- `exit_if_invalid()` - Sanitize name, check app dir exists, exit(1) if not
- `get_free_port()` - Find a free TCP port
- `get_boolean()` - Convert boolean-ish string to boolean
- `write_config()` - Write key=value config file
- `setup_authorized_keys()` - Append to ~/.ssh/authorized_keys with SSH restrictions
- `parse_procfile()` - Parse a Heroku-style Procfile
- `expandvars()` - Expand shell-style environment variables
- `command_output()` - Run shell command, return stdout
- `parse_settings()` - Parse KEY=VALUE file with variable interpolation
- `check_requirements()` - Check all binaries exist via `which`
- `found_app()` - Print "-----> {kind} app detected." in green
- `echo()` - Print colored output

## Key Data Structures

### `Runtime`
Enum representing supported application runtimes:
- `Python`, `PythonPoetry`, `PythonUv`
- `Node`, `Ruby`, `Go`
- `JavaMaven`, `JavaGradle`
- `ClojureCli`, `ClojureLein`
- `Rust`, `Identity`

### `WorkerConfig`
Structure for TOML-based worker configurations:
- `worker` - Information about the worker process
- `env` - Environment variables
- `options` - Options for the worker process

### `RikuPaths`
Structure containing all resolved directory paths:
- `riku_root`, `riku_bin`, `riku_script`
- `plugin_root`, `app_root`, `data_root`
- `env_root`, `git_root`, `log_root`
- `nginx_root`, `cache_root`, `uwsgi_root`
- `uwsgi_available`, `uwsgi_enabled`, `acme_root`, `acme_www`

## Error Handling

The application uses the `anyhow` crate for error handling, returning `Result<T, anyhow::Error>` from most functions. Errors are propagated up the call stack and handled appropriately at the CLI level.

## Testing

Each module contains comprehensive unit tests covering:
- Runtime detection
- Worker configuration creation
- Process management
- Utility functions
- Configuration parsing
- Error conditions

Run tests with `cargo test`.

## Performance Considerations

- The supervisor uses file watching (notify crate) to monitor configuration changes
- Process management is optimized for low overhead
- Memory usage is minimized through efficient data structures
- The Rust implementation provides better performance than the original Python version