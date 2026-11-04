# commands

## Overview
Component package located at `crates/riku-cli/src/agent/commands`.

## Interface
- `cmd_agent_apps` [cmd_app.rs:cmd_agent_apps] (fn)
- `cmd_agent_deploy` [cmd_app.rs:cmd_agent_deploy] (fn)
- `cmd_agent_destroy_request` [cmd_app.rs:cmd_agent_destroy_request] (fn)
- `cmd_app.rs` [cmd_app.rs] (and 1 additional symbols)
- `cmd_agent_config_get` [cmd_config.rs:cmd_agent_config_get] (fn)
- `cmd_agent_config_set` [cmd_config.rs:cmd_agent_config_set] (fn)
- `cmd_agent_config_show` [cmd_config.rs:cmd_agent_config_show] (fn)
- `cmd_ops.rs` [cmd_ops.rs] (internal module definitions)
- `cmd_agent_logs` [cmd_runtime.rs:cmd_agent_logs] (fn)
- `cmd_agent_ps` [cmd_runtime.rs:cmd_agent_ps] (fn)
- `cmd_agent_restart` [cmd_runtime.rs:cmd_agent_restart] (fn)
- `cmd_runtime.rs` [cmd_runtime.rs] (and 4 additional symbols)
- `mod.rs` [mod.rs] (internal module definitions)

## Invariants
- None

## Side Effects
- [cmd_app.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-cli`
