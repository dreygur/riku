# router

## Overview
Component package located at `crates/riku-deploy/src/router`.

## Interface
- `copy_socket` [backends.rs:copy_socket] (fn)
- `web_copies` [backends.rs:web_copies] (fn)
- `current_backends` [backends.rs:current_backends] (fn)
- `configure_caddy` [caddy.rs:configure_caddy] (fn)
- `fallback_backends` [fallback.rs:fallback_backends] (fn)
- `ready_for` [host_guard.rs:ready_for] (fn)
- `ensure_host_ready` [host_guard.rs:ensure_host_ready] (fn)
- `configure_plugin` [plugin.rs:configure_plugin] (fn)
- `reload_plugin` [plugin.rs:reload_plugin] (fn)
- `remove_plugin` [plugin.rs:remove_plugin] (fn)
- `build_route` [route_build.rs:build_route] (fn)
- `ERROR_FILE` [reload.rs:ERROR_FILE] (const)
- `record_outcome` [reload.rs:record_outcome] (fn)
- `reload` [reload.rs:reload] (fn)
- `reload.rs` [reload.rs] (and 1 additional symbols)
- `remove` [remove.rs:remove] (fn)
- [reload](./reload/README.md) - Child contract for `reload`

## Invariants
- [plugin.rs] The request carries already validated domains and goes to the plugin as JSON on stdin, with no shell interpolation; a plugin not declaring router protocol 2 is refused before `configure`
- [host_guard.rs] A router install is refused while `nginx_root` holds a `*.conf` that is no app's config; Caddy and router plugins have no such host step
- [remove.rs] For nginx, only paths built from the app name inside `nginx_root` are touched, never anything under `/etc/nginx`

## Side Effects
- [reload.rs] Spawns external child processes via `std::process::Command`
- [reload.rs] Mutates local filesystem (creates/modifies/removes files)

## Verification
- Run: `cargo test -p riku-deploy`
