# riku-deploy (src)

## Overview
Component package located at `crates/riku-deploy/src`.

## Interface
- `BackupService` [backup.rs:BackupService] (struct)
- `new` [backup.rs:new] (fn)
- `backup` [backup.rs:backup] (fn)
- `backup.rs` [backup.rs] (and 1 additional symbols)
- `load_deploy_env` [deploy_env.rs:load_deploy_env] (fn)
- `finish_deploy` [deploy_finish.rs:finish_deploy] (fn)
- `run_preflight` [env_setup.rs:run_preflight] (fn)
- `run_release` [env_setup.rs:run_release] (fn)
- `write_live_env` [env_setup.rs:write_live_env] (fn)
- `env_setup.rs` [env_setup.rs] (and 1 additional symbols)
- `head_sha` [git_ops.rs:head_sha] (fn)
- `git_fetch` [git_ops.rs:git_fetch] (fn)
- `git_reset` [git_ops.rs:git_reset] (fn)
- `git_ops.rs` [git_ops.rs] (and 2 additional symbols)
- `run_pre_deploy` [hooks.rs:run_pre_deploy] (fn)
- `run_pre_build` [hooks.rs:run_pre_build] (fn)
- `run_post_build` [hooks.rs:run_post_build] (fn)
- `hooks.rs` [hooks.rs] (and 1 additional symbols)
- `do_deploy` [lib.rs:do_deploy] (fn)
- `rollback` [lib.rs:rollback] (fn)
- `backup` [lib.rs:backup] (module)
- `lib.rs` [lib.rs] (and 17 additional symbols)
- `acquire` [lock.rs:acquire] (fn)
- `is_locked` [lock.rs:is_locked] (fn)
- `mock_engine` [mock_engine.rs:mock_engine] (fn)
- `logged` [mock_engine.rs:logged] (fn)
- `project_in` [mock_engine.rs:project_in] (fn)
- `deploy_procfile_app` [procfile_deploy.rs:deploy_procfile_app] (fn)
- `Release` [releases.rs:Release] (struct)
- `ReleaseLog` [releases.rs:ReleaseLog] (struct)
- `new` [releases.rs:new] (fn)
- `releases.rs` [releases.rs] (and 3 additional symbols)
- `configure` [router.rs:configure] (fn)
- `apply` [router.rs:apply] (fn)
- `remove` [router.rs:remove] (re-export of `router/remove.rs`)
- `ensure_host_ready` [router.rs:ensure_host_ready] (re-export of `router/host_guard.rs`)
- `apply_scaling_deltas` [scaling.rs:apply_scaling_deltas] (fn)
- `pull_service` [service_update.rs:pull_service] (fn)
- `resolve_riku_bin` [supervisor_ctl.rs:resolve_riku_bin] (fn)
- `ensure_supervisor_running` [supervisor_ctl.rs:ensure_supervisor_running] (fn)
- `notify_supervisor_reload` [supervisor_ctl.rs:notify_supervisor_reload] (fn)
- `supervisor_ctl.rs` [supervisor_ctl.rs] (and 1 additional symbols)
- `plugin_destroy` [teardown.rs:plugin_destroy] (fn)
- `stop_procfile_app` [app_stop.rs:stop_procfile_app] (fn)
- `restart_worker` [worker_control.rs:restart_worker] (fn)
- `delete_worker` [worker_control.rs:delete_worker] (fn)
- `read_scaling_count` [workers.rs:read_scaling_count] (fn)
- `create_workers_generic` [workers.rs:create_workers_generic] (fn)
- [containers](./containers/README.md) - Child contract for `containers`
- [rolling](./rolling/README.md) - Child contract for `rolling`
- [router](./router/README.md) - Child contract for `router`
- [workers](./workers/README.md) - Child contract for `workers`

## Invariants
- [backup.rs] Validates the app name, and every archive member before a restore (no absolute paths, no `..`)
- [deploy_env.rs] Warns about invalid env vars through `validate_env_vars`
- [router.rs] Refuses a malformed `RIKU_DOMAINS` before it reaches the router
- [supervisor_ctl.rs] Never resolves the riku binary by bare name off `$PATH`
- [worker_control.rs] Validates the app name and worker kind before building paths from them

## Side Effects
- [app_stop.rs] Removes an app's worker configs and the uWSGI sockets riku made for it
- [deploy_env.rs] Sets the app's env dir to `0700` and its ENV file to `0600`
- [backup.rs] Spawns external child processes via `std::process::Command`
- [backup.rs] Mutates local filesystem (creates/modifies/removes files)
- [env_setup.rs] Spawns external child processes via `std::process::Command`
- [env_setup.rs] Mutates local filesystem (creates/modifies/removes files)
- [env_setup.rs] Reads or updates process environment variables
- [env_setup.rs] Applies resource limits to child processes in a `pre_exec` hook
- [git_ops.rs] Spawns external child processes via `std::process::Command`
- [git_ops.rs] Mutates local filesystem (creates/modifies/removes files)
- [hooks.rs] Mutates local filesystem (creates/modifies/removes files)
- [lib.rs] Mutates local filesystem (creates/modifies/removes files)
- [lock.rs] Mutates local filesystem (creates/modifies/removes files)
- [lock.rs] Takes `flock` locks through `libc`
- [mock_engine.rs] Mutates local filesystem (creates/modifies/removes files)
- [releases.rs] Mutates local filesystem (creates/modifies/removes files)
- [scaling.rs] Mutates local filesystem (creates/modifies/removes files)
- [supervisor_ctl.rs] Spawns external child processes via `std::process::Command`
- [supervisor_ctl.rs] Mutates local filesystem (creates/modifies/removes files)
- [supervisor_ctl.rs] Reads or updates process environment variables
- [supervisor_ctl.rs] Signals the supervisor process through `nix` (`SIGHUP`)
- [worker_control.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-deploy`
