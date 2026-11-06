# rolling

## Overview
Component package located at `crates/riku-deploy/src/rolling`.

## Interface
- `HealthTarget` [health.rs:HealthTarget] (enum)
- `health_target` [health.rs:health_target] (fn)
- `procfile_target` [health.rs:procfile_target] (fn)
- `health.rs` [health.rs] (and 1 additional symbols)
- `CallLog` [mock_copies.rs:CallLog] (type)
- `NewCopy` [mock_copies.rs:NewCopy] (enum)
- `MockRuntime` [mock_copies.rs:MockRuntime] (struct)
- `mock_copies.rs` [mock_copies.rs] (and 5 additional symbols)
- `health` [mod.rs:health] (module)
- `replace` [mod.rs:replace] (module)
- `probe_url` [probe.rs:probe_url] (fn)
- `probe_port` [probe.rs:probe_port] (fn)
- `probe_socket` [probe.rs:probe_socket] (fn)
- `poll_probe` [probe.rs:poll_probe] (fn)
- `scale` [mod.rs:scale] (module)
- `mod.rs` [mod.rs] (and 1 additional symbols)
- `CopyHandle` [replace.rs:CopyHandle] (enum)
- `LiveCopy` [replace.rs:LiveCopy] (struct)
- `CopyRuntime` [replace.rs:CopyRuntime] (trait)
- `replace.rs` [replace.rs] (and 3 additional symbols)
- `start_healthy` [scale.rs:start_healthy] (fn)
- `publish_added` [scale.rs:publish_added] (fn)
- `add_copy` [scale.rs:add_copy] (fn)
- `scale.rs` [scale.rs] (and 2 additional symbols)

## Invariants
- [probe.rs] HTTP and TCP probes only connect to `127.0.0.1`, and the HTTP client follows no redirect and ignores system proxy settings

## Side Effects
- [probe.rs] Performs network I/O (HTTP, TCP and unix socket probes); the HTTP client comes from `riku-http`

## Verification
- Run: `cargo test -p riku-deploy`
