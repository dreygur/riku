# health

## Overview
Component package located at `crates/riku-supervisor/src/health`.

## Interface
- `RestartOutcome` [actions.rs:RestartOutcome] (enum)
- `ControlActions` [actions.rs:ControlActions] (trait)
- `NoopControlActions` [actions.rs:NoopControlActions] (struct)
- `actions.rs` [actions.rs] (and 1 additional symbols)
- `load_or_create_token` [auth.rs:load_or_create_token] (fn)
- `ControlState` [control.rs:ControlState] (struct)
- `control_router` [control.rs:control_router] (fn)
- `SharedSupervisorState` [mod.rs:SharedSupervisorState] (struct)
- `start_health_server` [mod.rs:start_health_server] (fn)
- `actions` [mod.rs:actions] (module)
- `mod.rs` [mod.rs] (and 1 additional symbols)
- `hooks_handler` [plugins.rs:hooks_handler] (fn)
- `build_health_json` [responses.rs:build_health_json] (fn)
- `build_metrics_json` [responses.rs:build_metrics_json] (fn)
- `build_app_metrics_json` [responses.rs:build_app_metrics_json] (fn)

## Invariants
- [control.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [auth.rs] Mutates local filesystem (creates/modifies/removes files)
- [control.rs] Mutates local filesystem (creates/modifies/removes files)
- [mod.rs] Performs network I/O or binds socket
- [mod.rs] Reads or updates process environment variables

## Verification
- Run: `cargo test -p riku-supervisor`
