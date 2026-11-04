# dispatch

## Overview
Component package located at `crates/riku-cli/src/dispatch`.

## Interface
- `run_apps` [app_cmds.rs:run_apps] (fn)
- `run_config` [app_cmds.rs:run_config] (fn)
- `run_logs` [app_cmds.rs:run_logs] (fn)
- `app_cmds.rs` [app_cmds.rs] (and 4 additional symbols)
- `run_local` [mod.rs:run_local] (fn)
- `run_plugins` [plugins.rs:run_plugins] (fn)
- `run_hook` [plugins.rs:run_hook] (fn)

## Invariants
- None

## Side Effects
- [mod.rs] Reads process environment variables (`RIKU_NS_ROOT`, `RIKU_NS_CMD`)

## Verification
- Run: `cargo test -p riku-cli`
