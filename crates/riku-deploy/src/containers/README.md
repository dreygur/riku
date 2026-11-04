# containers

## Overview
Component package located at `crates/riku-deploy/src/containers`.

## Interface
- `SINGLE_IMAGE_PORT` [app.rs:SINGLE_IMAGE_PORT] (const)
- `is_container_app` [app.rs:is_container_app] (fn)
- `app.rs` [app.rs] (and 5 additional symbols)
- `open_engine` [engine_open.rs:open_engine] (fn)
- `EngineUnreachable` [engine_open.rs:EngineUnreachable] (struct)
- `saved_env` [context.rs:saved_env] (fn)
- `ContainerApp` [context.rs:ContainerApp] (struct)
- `load` [context.rs:load] (fn)
- `context.rs` [context.rs] (and 5 additional symbols)
- `runs_containers` [control.rs:runs_containers] (fn)
- `restart_service` [control.rs:restart_service] (fn)
- `stop_service` [control.rs:stop_service] (fn)
- `control.rs` [control.rs] (and 1 additional symbols)
- `restart_copy` [copy_restart.rs:restart_copy] (fn)
- `RestartOutcome` [copy_restart.rs:RestartOutcome] (re-export)
- `CONTAINER_RUNTIME` [deploy.rs:CONTAINER_RUNTIME] (const)
- `deploy_containers` [deploy.rs:deploy_containers] (fn)
- `build_tag` [images.rs:build_tag] (fn)
- `prepare_images` [images.rs:prepare_images] (fn)
- `DB_AND_WEB` [mock_state.rs:DB_AND_WEB] (const)
- `MockServer` [mock_state.rs:MockServer] (struct)
- `calls` [mock_state.rs:calls] (fn)
- `mock_state.rs` [mock_state.rs] (and 6 additional symbols)
- `AppName` [mod.rs:AppName] (struct)
- `as_str` [mod.rs:as_str] (fn)
- `prepare_images` [mod.rs:prepare_images] (re-export)
- `mod.rs` [mod.rs] (and 5 additional symbols)
- `pick_free_port` [ports.rs:pick_free_port] (fn)
- `run_release` [release.rs:run_release] (fn)
- `app_containers` [removal.rs:app_containers] (fn)
- `remove_containers` [removal.rs:remove_containers] (fn)
- `remove_dropped` [removal.rs:remove_dropped] (fn)
- `removal.rs` [removal.rs] (and 1 additional symbols)
- `retire_procfile_workers` [retire.rs:retire_procfile_workers] (fn): stops and removes the Procfile workers of an app that has switched to containers
- `wait_restored_workers` [rollback_wait.rs:wait_restored_workers] (fn): during a migration rollback, waits for the restored old web workers to answer
- `roll_out` [rollout.rs:roll_out] (fn)
- `ServiceRoutes` [routes.rs:ServiceRoutes] (enum)
- `ContainerRuntime` [runtime.rs:ContainerRuntime] (struct)
- `copy_spec` [spec.rs:copy_spec] (fn)
- `start_copy` [start.rs:start_copy] (fn)

## Invariants
- [rollback_wait.rs] Health probes only connect to `127.0.0.1` on the port recorded in the restored worker's own config

## Side Effects
- [app.rs] Mutates local filesystem (reads and writes the app's `SCALING` file)
- [context.rs] Mutates local filesystem (creates/modifies/removes files)
- [mock_state.rs] Mutates local filesystem (creates/modifies/removes files)
- [ports.rs] Performs network I/O or binds socket

## Verification
- Run: `cargo test -p riku-deploy`
