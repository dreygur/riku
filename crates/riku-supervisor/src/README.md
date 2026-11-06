# riku-supervisor (src)

## Overview
Component package located at `crates/riku-supervisor/src`.

## Interface
- `RUNNING` [lib.rs:RUNNING] (static)
- `RELOAD_COUNTER` [lib.rs:RELOAD_COUNTER] (static)
- `CONFIG_RELOAD_LOCK` [lib.rs:CONFIG_RELOAD_LOCK] (static)
- `lib.rs` [lib.rs] (and 13 additional symbols)
- [cgroups](./cgroups/README.md) - Child contract for `cgroups`
- [config](./config/README.md) - Child contract for `config`
- `containers` [lib.rs:containers] (module) - Container supervision: adoption, event watching, log copying and restarts
- [daemon](./daemon/README.md) - Child contract for `daemon`
- [health](./health/README.md) - Child contract for `health`
- [log_rotation](./log_rotation/README.md) - Child contract for `log_rotation`
- [process](./process/README.md) - Child contract for `process`
- [stats](./stats/README.md) - Child contract for `stats`

## Invariants
- None

## Side Effects
- [lib.rs] Installs process signal handlers (`setup_signal_handlers`, `spawn_sighup_listener`)

## Verification
- Run: `cargo test -p riku-supervisor`
