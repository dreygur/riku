# addon

## Overview
Component package located at `crates/riku-plugins/src/addon`.

## Interface
- `VerbCall` [dispatch.rs:VerbCall] (struct)
- `run_verb` [dispatch.rs:run_verb] (fn)
- `AddonService` [mod.rs:AddonService] (re-export)
- `AddonService` [service.rs:AddonService] (struct)
- `new` [service.rs:new] (fn)
- `list` [service.rs:list] (fn)
- `service.rs` [service.rs] (and 5 additional symbols)
- `InstanceRecord` [state.rs:InstanceRecord] (struct)
- `new` [state.rs:new] (fn)
- `InstanceStore` [state.rs:InstanceStore] (struct)
- `state.rs` [state.rs] (and 6 additional symbols)

## Invariants
- None

## Side Effects
- [dispatch.rs] Spawns external child processes via `std::process::Command`
- [service.rs] Mutates local filesystem (creates/modifies/removes files)
- [state.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-plugins`
