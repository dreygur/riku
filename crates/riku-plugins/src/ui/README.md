# ui

## Overview
Component package located at `crates/riku-plugins/src/ui`.

## Interface
- `PanelField` [dispatch.rs:PanelField] (struct)
- `PanelSection` [dispatch.rs:PanelSection] (struct)
- `UiPanelResponse` [dispatch.rs:UiPanelResponse] (struct)
- `dispatch.rs` [dispatch.rs] (and 1 additional symbols)
- `mod.rs` [mod.rs] (internal module definitions)

## Invariants
- None

## Side Effects
- [dispatch.rs] Spawns external child processes via `std::process::Command`
## Verification
- Run: `cargo test -p riku-plugins`
