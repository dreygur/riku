# workers

## Overview
Component package located at `crates/riku-deploy/src/workers`.

## Interface
- `remove_stale_configs` [files.rs:remove_stale_configs] (fn)
- `write_worker_config` [files.rs:write_worker_config] (fn)
- `check_pinned_port` [pinned.rs:check_pinned_port] (fn)
- `check_pinned_port_with` [pinned.rs:check_pinned_port_with] (fn)
- `pinned_port` [ports.rs:pinned_port] (fn)
- `free_loopback_port` [ports.rs:free_loopback_port] (fn)
- `unused_port` [ports.rs:unused_port] (fn)
- `check_custom_socket` [socket.rs:check_custom_socket] (fn)
- `is_wsgi_kind` [web_env.rs:is_wsgi_kind] (fn)
- `is_web_facing` [web_env.rs:is_web_facing] (fn)
- `build_worker_env` [web_env.rs:build_worker_env] (fn)

## Invariants
- None

## Side Effects
- [files.rs] Mutates local filesystem (creates/modifies/removes files)
- [pinned.rs] Performs network I/O or binds socket
- [web_env.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-deploy`
