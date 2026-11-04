# apps

## Overview
Component package located at `crates/riku-cli/src/apps`.

## Interface
- `cmd_config_show` [config.rs:cmd_config_show] (fn)
- `cmd_config_get` [config.rs:cmd_config_get] (fn)
- `cmd_config_set` [config.rs:cmd_config_set] (fn)
- `config.rs` [config.rs] (and 3 additional symbols)
- `cmd_run` [control.rs:cmd_run] (fn)
- `cmd_restart` [control.rs:cmd_restart] (fn)
- `cmd_stop` [control.rs:cmd_stop] (fn)
- `control.rs` [control.rs] (and 2 additional symbols)
- `ControlTarget` [control_target.rs:ControlTarget] (enum)
- `control_target` [control_target.rs:control_target] (fn)
- `cmd_apps_create` [create.rs:cmd_apps_create] (fn)
- `cmd_deploy` [deploy.rs:cmd_deploy] (fn)
- `DestroyMode` [destroy.rs:DestroyMode] (enum)
- `cmd_destroy` [destroy.rs:cmd_destroy] (fn)
- `container_cleanup` [destroy_engine.rs:container_cleanup] (fn)
- `remove_app_files` [destroy_files.rs:remove_app_files] (fn) Deletes the files `riku destroy` removes once the route is gone. The app directory goes last so a failed destroy can be retried
- `cmd_dump_state` [dump_state.rs:cmd_dump_state] (fn)
- `state_json` [dump_state.rs:state_json] (fn)
- `cmd_hot_reload` [hot_reload.rs:cmd_hot_reload] (fn)
- `cmd_apps_info` [info.rs:cmd_apps_info] (fn)
- `cmd_install_plugins` [install_plugins.rs:cmd_install_plugins] (fn)
- `cmd_apps` [list.rs:cmd_apps] (fn)
- `cmd_deploy_logs` [logs.rs:cmd_deploy_logs] (fn)
- `cmd_logs` [logs.rs:cmd_logs] (fn)
- `known_services` [logs.rs:known_services] (fn)
- `tail_deploy_log` [logs/tail.rs:tail_deploy_log] (fn)
- `multi_tail` [logs/tail.rs:multi_tail] (fn)
- `logs/tail.rs` [logs/tail.rs] Log tailing for `riku logs`: follows several files at once and reopens a file after rotation
- `config` [mod.rs:config] (module)
- `control` [mod.rs:control] (module)
- `create` [mod.rs:create] (module)
- `mod.rs` [mod.rs] (the other module declarations and re-exports)
- `cmd_plugin_emit` [plugin_emit.rs:cmd_plugin_emit] (fn)
- `cmd_rollback` [rollback.rs:cmd_rollback] (fn)
- `cmd_stats_all` [stats.rs:cmd_stats_all] (fn)
- `cmd_stats_app` [stats.rs:cmd_stats_app] (fn)
- [dump_state](./dump_state/README.md) - Child contract for `dump_state`
- [process](./process/README.md) - Child contract for `process`

## Invariants
- [control_target.rs] Validates and sanitizes identifiers against path traversal and injection
- [create.rs] Validates and sanitizes identifiers against path traversal and injection
- [info.rs] Validates and sanitizes identifiers against path traversal and injection
- [logs.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [control.rs] Spawns external child processes via `std::process::Command`
- [control.rs] Mutates local filesystem (creates/modifies/removes files)
- [create.rs] Spawns external child processes via `std::process::Command`
- [create.rs] Mutates local filesystem (creates/modifies/removes files)
- [deploy.rs] Spawns external child processes via `std::process::Command`
- [deploy.rs] Mutates local filesystem (creates/modifies/removes files)
- [destroy_files.rs] Mutates local filesystem (creates/modifies/removes files)
- [hot_reload.rs] Mutates local filesystem (creates/modifies/removes files)
- [info.rs] Spawns external child processes via `std::process::Command`
- [install_plugins.rs] Mutates local filesystem (creates/modifies/removes files)
- [install_plugins.rs] Performs network I/O or binds socket
- [plugin_emit.rs] Reads process environment variables (`RIKU_PLUGIN_NAME`)
- [plugin_emit.rs] Publishes a `plugin.custom.*` event through `EventBus`

## Verification
- Run: `cargo test -p riku-cli`
