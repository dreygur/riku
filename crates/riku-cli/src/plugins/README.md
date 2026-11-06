# plugins

## Overview
Component package located at `crates/riku-cli/src/plugins`.

## Interface
- `cmd_plugins_install` [mod.rs:cmd_plugins_install] (fn)
- `cmd_plugins_list` [mod.rs:cmd_plugins_list] (fn)
- `cmd_plugins_remove` [mod.rs:cmd_plugins_remove] (fn)
- `mod.rs` [mod.rs] (and 9 additional symbols)

## Invariants
- [mod.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-cli`
