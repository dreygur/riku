# filters

## Overview
Component package located at `crates/riku-plugins/src/filters`.

## Interface
- `FilterBus` [bus.rs:FilterBus] (struct)
- `new` [bus.rs:new] (fn)
- `apply` [bus.rs:apply] (fn)
- `FilterBus` [mod.rs:FilterBus] (re-export)

## Invariants
- None

## Side Effects
- [bus.rs] Spawns external child processes via `std::process::Command`
## Verification
- Run: `cargo test -p riku-plugins`
