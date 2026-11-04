# log_rotation

## Overview
Component package located at `crates/riku-supervisor/src/log_rotation`.

## Interface
- `LogRotationConfig` [mod.rs:LogRotationConfig] (struct)
- `LogRotator` [mod.rs:LogRotator] (struct)
- `new` [mod.rs:new] (fn)
- `mod.rs` [mod.rs] (and 3 additional symbols)

## Invariants
- None

## Side Effects
- [mod.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-supervisor`
