# riku-nginx (src)

## Overview
Component package located at `crates/riku-nginx/src`. It renders a site config from a checked route and holds what the deploy user still does for nginx. The router helper (`riku-route`) writes the config and reloads nginx.

## Modules
- `app_snippet.rs`: the app's own snippet named by `RIKU_NGINX_INCLUDE_FILE`, checked for path, grammar and directive allowlist, and checked again after the `nginx.include_content` filter chain
- `backends.rs`: the default backend of an app from its ENV, and the upstream name
- `cloudflare.rs`: fetches the Cloudflare ranges and builds the `allow` lines; only root init calls it
- `default_server.rs`: decides from `nginx -T` whether root init adds riku's `return 444` catch-all for port 80; never when any other file already has a port 80 `default_server`
- `host.rs`: the one root-owned include file, `nginx_include_file`, holding `include <nginx_root>/*.conf;` and, when `default_server.rs` says so, the catch-all
- `remove.rs`: removes an app's certificate files and sockets from the deploy user's tree
- `route_site.rs`: renders a site config from a route, with every path passed in by the caller
- `snippet_grammar.rs`, `snippet_limits.rs`: the strict grammar an app snippet must fit, its split into statements and its size caps
- `ssl.rs`: issues a certificate with acme.sh or a self signed fallback into `nginx_root`
- `static_site.rs`, `static_copy.rs`: publishes an app's static files into the static root

## Invariants
- [sanitize.rs] Refuses `;`, `{`, `}` and newlines in values bound for nginx templates, plus backtick, `$`, quotes and `#` as defense in depth
- [route_site.rs] A CSP that holds `"`, `$`, `\` or a control character is refused, the same rule the route gate applies
- [app_snippet.rs] The snippet must resolve inside the app directory, fit the grammar in `snippet_grammar.rs`, and use only allowlisted directives, before and after filter plugins run
- [host.rs] The path written into the root-owned include file must be absolute and free of directive-breaking characters
- [cloudflare.rs] A response line becomes a directive only when it is a bare address or CIDR

## Side Effects
- [cloudflare.rs] Performs network I/O through `riku-http`
- [host.rs] Writes the include file atomically
- [remove.rs] Removes an app's files from `nginx_root` and `socket_root`
- [ssl.rs] Spawns acme.sh or openssl and writes certificate files in `nginx_root`
- [static_site.rs] Copies static files into the static root

## Verification
- Run: `cargo test -p riku-nginx`
