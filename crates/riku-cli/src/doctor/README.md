# doctor

## Overview
Component package located at `crates/riku-cli/src/doctor`.

## Interface
- `renamed_keys` [app_env.rs:renamed_keys] (fn)
- `dependencies` [checks.rs:dependencies] (fn)
- `directories` [checks.rs:directories] (fn)
- `binary` [checks.rs:binary] (fn)
- `checks.rs` [checks.rs] (and 4 additional symbols)
- `container_engine` [container_check.rs:container_engine] (fn)
- `Status` [mod.rs:Status] (enum)
- `Check` [mod.rs:Check] (struct)
- `ok` [mod.rs:ok] (fn)
- `mod.rs` [mod.rs] (and 10 additional symbols)
- `router` [router_check.rs:router] (fn)
- `settings_file` [settings_check.rs:settings_file] (fn)

## Invariants
- [router_check.rs] Read-only: runs `nginx -t` or `caddy validate` and reads the router config, but changes nothing
- [container_check.rs] Read-only: opening the engine only runs its probe and `version` commands

## Side Effects
- [checks.rs] Spawns external child processes via `std::process::Command`
- [checks.rs] Reads process environment variables
- [checks.rs] Reads free disk space with `statvfs`
- [router_check.rs] Spawns external child processes via `std::process::Command`

## Verification
- Run: `cargo test -p riku-cli`
