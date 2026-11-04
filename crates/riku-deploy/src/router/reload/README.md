# reload

## Overview
Component package located at `crates/riku-deploy/src/router/reload`.

## Interface
- `Baseline` [nginx_unit.rs:Baseline] (enum)
- `capture` [nginx_unit.rs:capture] (fn)
- `await_unit` [nginx_unit.rs:await_unit] (fn)
- `UnitState` [unit_state.rs:UnitState] (struct)
- `read_unit` [unit_state.rs:read_unit] (fn): reads the state of `riku-nginx-reload.service` through `systemctl show`
- `mock_systemctl.rs` holds a mock `systemctl` for the reload tests

## Invariants
- None

## Side Effects
- [nginx_unit.rs] Reads the monotonic clock through `nix::time::clock_gettime`
- [unit_state.rs] Spawns external child processes via `std::process::Command`

## Verification
- Run: `cargo test -p riku-deploy`
