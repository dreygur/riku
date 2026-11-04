# riku-cli (src)

## Overview
Component package located at `crates/riku-cli/src`.

## Interface
- `cmd_backup` [backup.rs:cmd_backup] (fn)
- `cmd_restore` [backup.rs:cmd_restore] (fn)
- `ContainerCmd` [container.rs:ContainerCmd] (struct)
- `cmd_container` [container.rs:cmd_container] (fn)
- `CliControlActions` [control_actions.rs:CliControlActions] (struct)
- `cmd_hook_list` [hooks.rs:cmd_hook_list] (fn)
- `cmd_hook_check` [hooks.rs:cmd_hook_check] (fn)
- `addon` [lib.rs:addon] (module)
- `agent` [lib.rs:agent] (module)
- `apps` [lib.rs:apps] (module)
- `lib.rs` [lib.rs] (and 11 additional symbols)
- `cmd_scp` [scp.rs:cmd_scp] (fn)
- `split_original` [ssh_command.rs:split_original] (fn)
- `process_argv` [ssh_command.rs:process_argv] (fn)
- [addon](./addon/README.md) - Child contract for `addon`
- [agent](./agent/README.md) - Child contract for `agent`
- [apps](./apps/README.md) - Child contract for `apps`
- [dispatch](./dispatch/README.md) - Child contract for `dispatch`
- [doctor](./doctor/README.md) - Child contract for `doctor`
- [git](./git/README.md) - Child contract for `git`
- [plugins](./plugins/README.md) - Child contract for `plugins`
- [setup](./setup/README.md) - Child contract for `setup`

## Invariants
- [hooks.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [container.rs] Spawns external child processes via `std::process::Command`
- [scp.rs] Spawns external child processes via `std::process::Command`
- [ssh_command.rs] Reads process environment variables (`SSH_ORIGINAL_COMMAND`)

## Verification
- Run: `cargo test -p riku-cli`
