# events

## Overview
Component package located at `crates/riku-plugins/src/events`.

## Interface
- `EventBus` [bus.rs:EventBus] (struct)
- `new` [bus.rs:new] (fn)
- `publish` [bus.rs:publish] (fn)
- `bus.rs` [bus.rs] (and 2 additional symbols)
- `EventName` [mod.rs:EventName] (enum)
- `as_str` [mod.rs:as_str] (fn)
- `from_hook` [mod.rs:from_hook] (fn)
- `mod.rs` [mod.rs] (and 5 additional symbols)

## Invariants
- [bus.rs] `publish_custom` does not re-check a custom event: the caller validates the name's namespace and that the plugin declared `events.emit = true`
- [mod.rs] A plugin-emitted envelope's `name` must already be validated as `plugin.custom.*` by the caller

## Side Effects
- [bus.rs] Spawns external child processes via `std::process::Command`

## Verification
- Run: `cargo test -p riku-plugins`
