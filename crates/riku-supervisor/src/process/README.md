# process

## Overview
Component package located at `crates/riku-supervisor/src/process`.

## Interface
- `GenerationStatus` [generation.rs:GenerationStatus] (enum)
- `AppGeneration` [generation.rs:AppGeneration] (struct)
- `ProbeOutcome` [generation.rs:ProbeOutcome] (struct)
- `check_processes` [health_check.rs:check_processes] (fn)
- `run_health_checks` [health_check.rs:run_health_checks] (fn)
- `perform_health_check` [health_check.rs:perform_health_check] (fn)
- `health_check.rs` [health_check.rs] (and 2 additional symbols)
- `claim_health_probe` [health_interval.rs:claim_health_probe] (fn)
- `ProcessManager` [mod.rs:ProcessManager] (struct)
- `new` [mod.rs:new] (fn)
- `get_process_count` [mod.rs:get_process_count] (fn)
- `mod.rs` [mod.rs] (and 12 additional symbols)
- `deploy_generation` [orchestration.rs:deploy_generation] (fn)
- `reconcile_generations` [orchestration.rs:reconcile_generations] (fn)
- `drain_deployment_events` [orchestration.rs:drain_deployment_events] (fn)
- `orchestration.rs` [orchestration.rs] (and 2 additional symbols)
- `worker_pid_dir` [pid_file.rs:worker_pid_dir] (fn)
- `WorkerPidFile` [pid_file.rs:WorkerPidFile] (struct)
- `app_has_live_worker` [pid_file.rs:app_has_live_worker] (fn)
- `process_has_live_worker` [pid_file.rs:process_has_live_worker] (fn)
- `clear_markers` [pid_file.rs:clear_markers] (fn)
- `spawn_process` [spawn.rs:spawn_process] (fn)
- `open_log_files` [spawn.rs:open_log_files] (fn)
- `SpawnedProcess` [spawned.rs:SpawnedProcess] (struct)
- `new_with_cgroup` [spawned.rs:new_with_cgroup] (fn)
- `oom_kill_count` [spawned.rs:oom_kill_count] (fn)
- `spawned.rs` [spawned.rs] (and 4 additional symbols)
- `stop_process_by_id` [stop.rs:stop_process_by_id] (fn)
- `stop_app_processes` [stop.rs:stop_app_processes] (fn)
- `stop_all_processes` [stop.rs:stop_all_processes] (fn)
- `minimal_config` [test_support.rs:minimal_config] (fn, test builds only)
- `ProcessInfo` [test_support.rs:ProcessInfo] (struct, test builds only)
- [isolation](./isolation/README.md) - Child contract for `isolation`

## Invariants
- None

## Side Effects
- [health_check.rs] Performs network I/O or binds socket
- [orchestration.rs] Performs network I/O or binds socket
- [spawn.rs] Spawns external child processes via `std::process::Command`
- [spawn.rs] Mutates local filesystem (creates/modifies/removes files)
- [spawn.rs] Invokes Linux isolation or cgroup syscalls
- [spawned.rs] Invokes Linux isolation or cgroup syscalls

## Verification
- Run: `cargo test -p riku-supervisor`
