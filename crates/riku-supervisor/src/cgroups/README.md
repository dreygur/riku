# cgroups

## Overview
Component package located at `crates/riku-supervisor/src/cgroups`.

## Interface
- `CgroupLimits` [mod.rs:CgroupLimits] (struct)
- `WorkerCgroup` [mod.rs:WorkerCgroup] (struct)
- `startup_diagnostic` [mod.rs:startup_diagnostic] (fn)
- `CgroupTree` [tree.rs:CgroupTree] (struct)
- `take_over_own` [tree.rs:take_over_own] (fn)

## Invariants
- The supervisor's own cgroup comes from the `0::` line of `/proc/self/cgroup`; the riku unit must say `Delegate=yes`
- The supervisor sits in `<own>/supervisor`; isolated workers in `<own>/apps/<app>@<kind>.<n>`
- A worker cgroup name must parse back as a `WorkerName`

## Side Effects
- [tree.rs] Creates `supervisor/` and `apps/`, writes its pid to `supervisor/cgroup.procs`, writes `cgroup.subtree_control`
- [mod.rs] Creates worker cgroups and writes `memory.max`, `cpu.max`, `pids.max` and `cgroup.procs`

## Verification
- Run: `cargo test -p riku-supervisor cgroups` (fake tree in a tempdir; kernel behavior only on a real host)
