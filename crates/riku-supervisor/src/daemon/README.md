# daemon

## Overview
Component package located at `crates/riku-supervisor/src/daemon`.

## Interface
- `reload_all_configs` [config_watcher.rs:reload_all_configs] (fn)
- `load_initial_configs` [config_watcher.rs:load_initial_configs] (fn)
- `handle_file_event` [config_watcher.rs:handle_file_event] (fn)
- `config_watcher.rs` [config_watcher.rs] (and 3 additional symbols)
- `check_cron_jobs` [cron_tasks.rs:check_cron_jobs] (fn)
- `get_app_context` [cron_tasks.rs:get_app_context] (fn)
- `load_cron_jobs` [cron_tasks.rs:load_cron_jobs] (fn)
- `containers_if_due` [container_tick.rs:containers_if_due] (fn)
- `run_event_loop` [event_loop.rs:run_event_loop] (fn)
- `shutdown` [event_loop.rs:shutdown] (fn)
- `check_watched_images` [image_watch.rs:check_watched_images] (fn)
- `new` [init.rs:new] (fn)
- `with_actions` [init.rs:with_actions] (fn)
- `create_pid_file_with_lock` [init.rs:create_pid_file_with_lock] (fn)
- `rotate_logs` [maintenance.rs:rotate_logs] (fn)
- `write_stats` [maintenance.rs:write_stats] (fn)
- `Supervisor` [mod.rs:Supervisor] (struct)
- `run` [mod.rs:run] (fn)
- `config_watcher` [mod.rs:config_watcher] (module)
- `mod.rs` [mod.rs] (and 8 additional symbols)
- `is_due` [periodic.rs:is_due] (fn)
- `run_periodic_maintenance` [periodic.rs:run_periodic_maintenance] (fn)
- `acquire_pid_lock` [startup.rs:acquire_pid_lock] (fn)
- `check_cgroup_isolation` [startup.rs:check_cgroup_isolation] (fn)
- `start_health_endpoint` [startup.rs:start_health_endpoint] (fn)
- `watch_config_dir` [startup.rs:watch_config_dir] (fn)

## Invariants
- [cron_tasks.rs] Skips a cron Procfile line whose schedule fails `validate_cron_expression`

## Side Effects
- [cron_tasks.rs] Spawns external child processes via `std::process::Command`
- [cron_tasks.rs] Applies resource limits in the child through `pre_exec`
- [image_watch.rs] Reads app `ENV` files
- [init.rs] Takes an exclusive `flock` on the PID file
- [event_loop.rs] Removes the PID file on shutdown
- [startup.rs] Reads `RIKU_ENV` and `RIKU_HEALTH_PORT` from the process environment
- [startup.rs] Checks that the cgroup root is writable

## Verification
- Run: `cargo test -p riku-supervisor`
