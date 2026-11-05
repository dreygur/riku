# riku-nginx (src)

## Overview
Component package located at `crates/riku-nginx/src`. It renders a site config from a checked route and holds what the deploy user still does for nginx. The router helper (`riku-route`) writes the config and reloads nginx.

## Modules
- `backends.rs`: the default backend of an app from its ENV, and the upstream name
- `default_server.rs`: decides from `nginx -T` whether root init adds riku's `return 444` catch-all for port 80; never when any other file already has a port 80 `default_server`
- `host.rs`: the one root-owned include file, `nginx_include_file`, holding `include <nginx_root>/*.conf;` and, when `default_server.rs` says so, the catch-all
- `remove.rs`: removes an app's certificate files and sockets from the deploy user's tree
- `route_site.rs`: renders a site config from a route, with every path passed in by the caller
- `ssl.rs`: issues a certificate with acme.sh or a self signed fallback into `nginx_root`
- `static_site.rs`, `static_copy.rs`: publishes an app's static files into the static root

## Invariants
- [sanitize.rs] Refuses `;`, `{`, `}` and newlines in values bound for nginx templates, plus backtick, `$`, quotes and `#` as defense in depth
- [host.rs] The path written into the root-owned include file must be absolute and free of directive-breaking characters

## Side Effects
- [host.rs] Writes the include file atomically
- [remove.rs] Removes an app's files from `nginx_root` and `socket_root`
- [ssl.rs] Spawns acme.sh or openssl and writes certificate files in `nginx_root`
- [static_site.rs] Copies static files into the static root

## Verification
- Run: `cargo test -p riku-nginx`
