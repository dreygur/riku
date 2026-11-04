# engine

## Overview
Component package located at `crates/riku-container/src/engine`.

## Interface
- `spawn_in_group` [bounded.rs:spawn_in_group] (fn)
- `wait_bounded` [bounded.rs:wait_bounded] (fn)
- `drain` [bounded.rs:drain] (fn)
- `capture` [bounded.rs:capture] (fn)
- `bounded.rs` [bounded.rs] (and 1 additional symbols)
- `BuildSpec` [build.rs:BuildSpec] (struct)
- `build` [build.rs:build] (fn)
- `run` [control.rs:run] (fn)
- `stop` [control.rs:stop] (fn)
- `kill` [control.rs:kill] (fn)
- `control.rs` [control.rs] (and 3 additional symbols)
- `KNOWN_ENGINES` [detect.rs:KNOWN_ENGINES] (const)
- `EngineBinary` [detect.rs:EngineBinary] (struct)
- `detect` [detect.rs:detect] (fn)
- `is_valid_name` [env_file.rs:is_valid_name] (fn)
- `write_env_file` [env_file.rs:write_env_file] (fn)
- `has_events` [events.rs:has_events] (fn)
- `events_command` [events.rs:events_command] (fn)
- `event_container` [events.rs:event_container] (fn)
- `events.rs` [events.rs] (and 1 additional symbols)
- `fake_engine` [fake.rs:fake_engine] (fn)
- `logging_engine` [fixture.rs:logging_engine] (fn)
- `sample_spec` [fixture.rs:sample_spec] (fn)
- `ID` [fixture.rs:ID] (const)
- `fixture.rs` [fixture.rs] (and 6 additional symbols)
- `EngineIdentity` [identity.rs:EngineIdentity] (struct)
- `parse_identity` [identity.rs:parse_identity] (fn)
- `Engine` [identity.rs:Engine] (struct)
- `identity.rs` [identity.rs] (and 7 additional symbols)
- `ContainerId` [ids.rs:ContainerId] (struct)
- `as_str` [ids.rs:as_str] (fn)
- `ContainerState` [inspect.rs:ContainerState] (enum)
- `ContainerInfo` [inspect.rs:ContainerInfo] (struct)
- `list_with_filter` [inspect.rs:list_with_filter] (fn)
- `inspect.rs` [inspect.rs] (and 3 additional symbols)
- `parse_inspect` [inspect_parse.rs:parse_inspect] (fn)
- `login` [login.rs:login] (fn)
- `print_logs` [logs.rs:print_logs] (fn)
- `logs_command` [logs.rs:logs_command] (fn)
- `owner_of` [lookup.rs:owner_of] (fn)
- `id_of` [lookup.rs:id_of] (fn)
- `list_project` [lookup.rs:list_project] (fn)
- `lookup.rs` [lookup.rs] (and 1 additional symbols)
- `BuildSpec` [mod.rs:BuildSpec] (re-export)
- `ContainerId` [mod.rs:ContainerId] (re-export)
- `REQUIRED_COMMANDS` [mod.rs:REQUIRED_COMMANDS] (re-export)
- `mod.rs` [mod.rs] (and 13 additional re-exports)
- `run_within` [output.rs:run_within] (fn)
- `run_quiet` [output.rs:run_quiet] (fn)
- `output_within` [output.rs:output_within] (fn)
- `REQUIRED_COMMANDS` [probe.rs:REQUIRED_COMMANDS] (const)
- `answers_version` [probe.rs:answers_version] (fn)
- `first_unanswered` [probe.rs:first_unanswered] (fn)
- `ensure_network` [resources.rs:ensure_network] (fn)
- `volume_exists` [resources.rs:volume_exists] (fn)
- `volume_labels` [resources.rs:volume_labels] (fn)
- `resources.rs` [resources.rs] (and 1 additional symbols)
- `whole_seconds_up` [run_args.rs:whole_seconds_up] (fn)
- `run_args` [run_args.rs:run_args] (fn)
- `PullPolicy` [run_spec.rs:PullPolicy] (enum)
- `RunSpec` [run_spec.rs:RunSpec] (struct)
- `fetch_present` [snapshot.rs:fetch_present] (fn)
- `check_positional` [spec_check.rs:check_positional] (fn)
- `check_spec` [spec_check.rs:check_spec] (fn)
- `exec_command_line` [spec_check.rs:exec_command_line] (fn)
- `ContainerStats` [stats.rs:ContainerStats] (struct)
- `parse_stats` [stats.rs:parse_stats] (fn)
- `stats` [stats.rs:stats] (fn)
- `run_streamed` [streamed.rs:run_streamed] (fn)

## Invariants
- [build.rs] Paths must be absolute and cannot contain parent traversal (`..`)
- [inspect_parse.rs] Validates and sanitizes identifiers against path traversal and injection
- [mod.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [bounded.rs] Spawns external child processes via `std::process::Command`, each in its own process group, and signals the group on timeout
- [control.rs] Runs the engine through the helpers in `output.rs` and `streamed.rs`
- [events.rs] Builds engine `std::process::Command`s for the caller to spawn
- [fake.rs] Mutates local filesystem (creates/modifies/removes files), test builds only
- [login.rs] Spawns external child processes via `std::process::Command`
- [logs.rs] Builds engine `std::process::Command`s for the caller to spawn
- [output.rs] Spawns the engine through `bounded.rs`, with a time limit on output capture
- [streamed.rs] Spawns external child processes via `std::process::Command`

## Verification
- Run: `cargo test -p riku-container`
