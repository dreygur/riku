# types

## Overview
Component package located at `crates/riku-container/src/types`.

## Interface
- `DEFAULT_STOP_GRACE` [lifecycle.rs:DEFAULT_STOP_GRACE] (const)
- `StopSignal` [lifecycle.rs:StopSignal] (struct)
- `as_str` [lifecycle.rs:as_str] (fn)
- `lifecycle.rs` [lifecycle.rs] (and 3 additional symbols)
- `mod.rs` [mod.rs] (internal module definitions)
- `MountSource` [mounts.rs:MountSource] (enum)
- `Mount` [mounts.rs:Mount] (struct)
- `is_writable` [mounts.rs:is_writable] (fn)
- `APP` [names.rs:APP] (const)
- `SERVICE` [names.rs:SERVICE] (const)
- `COPY` [names.rs:COPY] (const)
- `names.rs` [names.rs] (and 22 additional symbols)
- `HostIp` [ports.rs:HostIp] (enum)
- `Protocol` [ports.rs:Protocol] (enum)
- `PortBinding` [ports.rs:PortBinding] (struct)

## Invariants
- [names.rs] Validates and sanitizes identifiers against path traversal and injection

## Side Effects
- None

## Verification
- Run: `cargo test -p riku-container`
