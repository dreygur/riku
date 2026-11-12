# stats

## Overview
Component package located at `crates/riku-supervisor/src/stats`.

## Interface
- `StatsManager` [manager.rs:StatsManager] (struct)
- `new` [manager.rs:new] (fn)
- `register_process` [manager.rs:register_process] (fn)
- `manager.rs` [manager.rs] (and 8 additional symbols)
- `manager` [mod.rs:manager] (module)
- `queries` [mod.rs:queries] (module)
- `repository` [mod.rs:repository] (module)
- `mod.rs` [mod.rs] (and 5 additional symbols)
- `get_all_stats` [queries.rs:get_all_stats] (fn)
- `remove_process` [queries.rs:remove_process] (fn)
- `remove_app` [queries.rs:remove_app] (fn)
- `read_stats_file` [repository.rs:read_stats_file] (fn)
- `ProcessTable` [resources.rs:ProcessTable] (struct)
- `ResourceUsage` [resources.rs:ResourceUsage] (struct)
- `ProcessStats` [types.rs:ProcessStats] (struct)
- `ProcessStatus` [types.rs:ProcessStatus] (enum)
- `HealthStatus` [types.rs:HealthStatus] (enum)
- `types.rs` [types.rs] (and 1 additional symbols)

## Invariants
- None

## Side Effects
- [manager.rs] Writes the stats file with `write_atomic`

## Verification
- Run: `cargo test -p riku-supervisor`
