# addon

## Overview
Component package located at `crates/riku-cli/src/addon`.

## Interface
- `cmd_addon_list` [mod.rs:cmd_addon_list] (fn)
- `cmd_addon_create` [mod.rs:cmd_addon_create] (fn)
- `cmd_addon_bind` [mod.rs:cmd_addon_bind] (fn)
- `mod.rs` [mod.rs] (and 3 additional symbols)

## Invariants
- [mod.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- [mod.rs] Changes addon instances and app bindings through `AddonService`

## Verification
- Run: `cargo test -p riku-cli`
