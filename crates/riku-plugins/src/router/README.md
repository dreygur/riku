# router

## Overview
Component package located at `crates/riku-plugins/src/router`.

## Interface
- `run_verb` [dispatch.rs:run_verb] (fn)
- `run_verb` [mod.rs:run_verb] (re-export)

## Invariants
- None

## Side Effects
- [dispatch.rs] Spawns external child processes via `std::process::Command`
## Verification
- Run: `cargo test -p riku-plugins`
