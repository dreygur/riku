# isolation

## Overview
Component package located at `crates/riku-supervisor/src/process/isolation`.

## Interface
- `exec_isolated` [linux.rs:exec_isolated] (fn)
- `NamespaceConfig` [mod.rs:NamespaceConfig] (struct)
- `exec_isolated` [mod.rs:exec_isolated] (fn)

## Invariants
- None

## Side Effects
- [linux.rs] Spawns external child processes via `std::process::Command`
- [linux.rs] Mutates local filesystem (creates/modifies/removes files)
- [linux.rs] Invokes Linux isolation or cgroup syscalls
- [mod.rs] Invokes Linux isolation or cgroup syscalls

## Verification
- Run: `cargo test -p riku-supervisor`
