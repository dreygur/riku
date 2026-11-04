# cgroups

## Overview
Component package located at `crates/riku-supervisor/src/cgroups`.

## Interface
- `CgroupLimits` [mod.rs:CgroupLimits] (struct)
- `WorkerCgroup` [mod.rs:WorkerCgroup] (struct)
- `verify_root_writable` [mod.rs:verify_root_writable] (fn)
- `mod.rs` [mod.rs] (and 5 additional symbols)

## Invariants
- None

## Side Effects
- [mod.rs] Mutates local filesystem (creates/modifies/removes files)
- [mod.rs] Creates cgroup v2 directories and writes `memory.max`, `cpu.max`, `pids.max` and `cgroup.procs`

## Verification
- Run: `cargo test -p riku-supervisor`
