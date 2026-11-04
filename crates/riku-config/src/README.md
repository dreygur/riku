# riku-config (src)

## Overview
Component package located at `crates/riku-config/src`.

## Interface
- `ENGINE_FILE` [container_engine.rs:ENGINE_FILE] (const)
- `load` [container_engine.rs:load] (fn)
- `RIKU_WORKER_TIMEOUT` [lib.rs:RIKU_WORKER_TIMEOUT] (const)
- `RIKU_WORKER_GRACE_PERIOD` [lib.rs:RIKU_WORKER_GRACE_PERIOD] (const)
- `RikuPaths` [lib.rs:RikuPaths] (struct)
- `lib.rs` [lib.rs] (also `RIKU_MAX_RESTARTS`, the `NGINX_CACHE_*_DEFAULT` constants and the `RikuPaths` constructors)
- `NginxHostPaths` [nginx_host.rs:NginxHostPaths] (struct): the legacy `sites-enabled` directory and the include file the system nginx loads riku's sites through
- `DEFAULT_NGINX_SITES_ENABLED` [nginx_host.rs:DEFAULT_NGINX_SITES_ENABLED] (const)
- `DEFAULT_NGINX_INCLUDE_FILE` [nginx_host.rs:DEFAULT_NGINX_INCLUDE_FILE] (const)
- `find_program` [program.rs:find_program] (fn): locates a host executable on `$PATH` plus a fixed list of system directories
- `ReloadPrograms` [reload_programs.rs:ReloadPrograms] (struct)
- `root_dirs` [root_dirs.rs:root_dirs] (fn): validates `HOME` and `RIKU_ROOT`
- `ROUTER_FILE` [router.rs:ROUTER_FILE] (const)
- `Router` [router.rs:Router] (enum)
- `load` [router.rs:load] (fn)
- `router.rs` [router.rs] (and 3 additional symbols)
- `SETTINGS_FILE` [settings.rs:SETTINGS_FILE] (const)
- `EngineSettings` [settings.rs:EngineSettings] (struct)
- `ServerSettings` [settings.rs:ServerSettings] (struct)
- `settings.rs` [settings.rs] (and 1 additional symbols)

## Invariants
- [container_engine.rs] The engine setting is only read here; it is validated where it is turned into a program to run
- [nginx_host.rs] Root init uses `NginxHostPaths::system` and ignores the `RIKU_NGINX_SITES_ENABLED` and `RIKU_NGINX_HOST_INCLUDE` overrides, since `sudo -E` keeps the caller's environment
- [program.rs] Only compile-time program names such as `nginx` or `systemctl` are looked up, never app input
- [root_dirs.rs:checked_dir] Paths must be absolute and cannot contain parent traversal (`..`)

## Side Effects
- [lib.rs] Reads or updates process environment variables
- [nginx_host.rs] Reads process environment variables
- [program.rs] Reads `PATH` from the process environment
- [router.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-config`
