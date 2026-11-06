# router

## Overview
Component package located at `crates/riku-deploy/src/router`.

## Interface
- `copy_socket`, `web_copies`, `current_backends` [backends.rs]: the app's running backends
- `configure_caddy` [caddy.rs]: writes the Caddy site
- `fallback_backends` [fallback.rs]: the default backend of an app with no web copy
- `apply_nginx_site`, `check_nginx_site`, `remove_nginx_site` [nginx_site.rs]: the deploy user's part of an nginx site and the requests to the router helper
- `send` [helper_client.rs]: one request to the router helper socket
- `configure_plugin`, `reload_plugin`, `remove_plugin` [plugin.rs]: the router plugin verbs
- `build_route` [route_build.rs]: the checked route sent to the helper
- `ERROR_FILE`, `record_outcome`, `reload_caddy`, `reload_router_plugin` [reload.rs]: confirmed reloads and the failure file doctor reads
- `remove` [remove.rs]: removes an app from the active router

## Invariants
- [plugin.rs] The request carries already validated domains and goes to the plugin as JSON on stdin, with no shell interpolation; a plugin not declaring router protocol 2 is refused before `configure`
- [nginx_site.rs] The deploy user never writes an nginx config or runs nginx; a helper refusal is reported with its stage and message
- [remove.rs] For nginx, local files are deleted only after the helper removed the site

## Side Effects
- [reload.rs] Spawns external child processes via `std::process::Command`
- [reload.rs] Mutates local filesystem (creates/modifies/removes files)
- [helper_client.rs] Connects to `/run/riku-router.sock`

## Verification
- Run: `cargo test -p riku-deploy`
