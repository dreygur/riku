# resource_limits

## Overview
Component package located at `crates/riku-util/src/resource_limits`.

## Interface
- `ResourceLimits` [mod.rs:ResourceLimits] (struct)
- `from_env` [mod.rs:from_env] (fn)
- `apply` [mod.rs:apply] (fn)
- `mod.rs` [mod.rs] (and 1 additional symbols)

## Invariants
- None

## Side Effects
- [mod.rs] Reads or updates process environment variables
- [mod.rs] Calls `setrlimit` in the calling process (used from a child's `pre_exec`)

## Verification
- Run: `cargo test -p riku-util`
