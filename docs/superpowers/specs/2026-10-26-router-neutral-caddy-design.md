# Router neutral riku, Caddy first

## Intent

Run riku on a fresh server with Caddy and its automatic HTTPS, and no nginx
installed. nginx stays the default. Traefik follows later on the same pattern.

riku is used only on the owner's servers, so breaking changes are fine and
there are no backward compatible aliases.

## Goals

1. Every env var riku itself defines carries a `RIKU_` prefix, and the
   routing ones are router neutral.
2. `RIKU_ROUTER=caddy` works end to end on a host with no nginx: init,
   deploy, destroy, reload, doctor, dashboard checks, systemd units.
3. A compose app that publishes host port 80 or 443 fails its deploy with a
   clear message, under any router.

## Non goals

- Traefik (next project, same pattern as Caddy).
- Running two routers on one host. The router stays a host level singleton.
- riku installing or supervising the router daemon. The router is the OS
  service (`nginx`, `caddy`), riku only writes config and triggers reloads.
- The release reset to 0.1.2. Separate step, confirmed before any release is
  deleted.

## Anti goals

- An upgraded host silently loses an app's domain or HTTPS because its ENV
  still uses an old key name.
- Caddy config built by string splicing user input, so a crafted domain value
  injects config.
- riku driving Caddy through its admin API: it listens on localhost with no
  auth, so any local user could reconfigure Caddy.

## Part 1: env rename

Every name riku invented moves under `RIKU_`. Names another tool reads directly
keep their standard name: `PORT`, `PATH`, `HOME`, `COMPOSE_FILE`, `UWSGI_*`
(uWSGI reads `UWSGI_<option>` from its environment itself), and app owned
values such as `DATABASE_URL`.

| Old | New |
|---|---|
| `NGINX_SERVER_NAME` | `RIKU_DOMAINS` |
| `NGINX_HTTPS_ONLY` | `RIKU_HTTPS_ONLY` |
| `NGINX_INTERNAL_PORT` | `RIKU_UPSTREAM_PORT` |
| `BIND_ADDRESS` | `RIKU_BIND_ADDRESS` |
| `DISABLE_IPV6` | `RIKU_DISABLE_IPV6` |
| other `NGINX_*` (cache, static paths, Cloudflare ACL, catch all, include file, IPv4/IPv6 address, WSGI, portmap, socket, document root, external port) | `RIKU_NGINX_*` |
| `ACME_EMAIL`, `ACME_ROOT_CA`, `ACME_ROOT` | `RIKU_ACME_*` |
| `NODE_VERSION`, `PYTHON_VERSION`, `PYTHON_PACKAGE_MANAGER` | `RIKU_NODE_VERSION`, `RIKU_PYTHON_VERSION`, `RIKU_PYTHON_PACKAGE_MANAGER` |
| `RUNTIME` | `RIKU_RUNTIME` (already read as a second name; drop `RUNTIME`) |
| `GHCR_USERNAME`, `GHCR_TOKEN` | `RIKU_GHCR_USERNAME`, `RIKU_GHCR_TOKEN` |

Scope covers Rust sources, `templates/*.tera`, the bundled shell plugins in
`plugins/`, example apps, tests, and docs. Template variable names that are
purely internal (`APP`, `ACME_WWW`, `APP_UPSTREAM`) are renamed only if they
collide; they never come from a user ENV.

Legacy keys: deploy output and `riku doctor` list any old key found in an
app's ENV with its new name. Deploy proceeds; the warning is loud.

Mechanics: the rename is mechanical, done with `sed` over `git grep -l`
results, one old name per pass with word boundaries, followed by a review of
the diff.

## Part 2: Caddy router

Built into riku, next to nginx, behind the existing `RIKU_ROUTER` seam in
`crates/riku-deploy/src/router.rs`. The `examples/plugins/caddy-router` shell
plugin is removed; the plugin seam stays for third party routers.

Config:
- One file per app at `$RIKU_ROOT/caddy/<app>.caddy`, generated from a typed
  request (domains, upstream port, HTTPS only).
- Every domain is validated as a hostname before it is written. Anything else
  fails the deploy.
- No domain: no site file, and the deploy output says how to add one.
- `RIKU_HTTPS_ONLY`: Caddy already redirects HTTP to HTTPS for named sites;
  an app that is not HTTPS only gets an explicit `http://` site address too.

Host wiring, done by `riku init` when `RIKU_ROUTER=caddy`:
- Check `caddy` is installed; do not warn about nginx.
- Add `import <riku root>/caddy/*.caddy` to `/etc/caddy/Caddyfile` once.
- The `caddy` user must be able to read `<riku root>/caddy`. Grant that
  through group membership on that directory, never by opening the riku
  home directory to everyone. nginx faces the same need; match whatever init
  already does for it.
- Skip the nginx ACME first run config.
- systemd: `riku-caddy.path` watches `<riku root>/caddy` and starts
  `riku-caddy-reload.service`, which runs `systemctl reload caddy`. Same shape
  as the existing nginx path unit. riku's own units stop ordering on
  `nginx.service` when Caddy is active.

Everywhere else nginx is assumed today becomes router aware: supervisor reload
on SIGHUP, `riku doctor`, dashboard system checks, `riku apps info`.

## Part 3: compose port check

Before `compose up`, the container plugin runs `compose config --format json`
and fails the deploy if any service publishes host port 80 or 443, naming the
service and port and pointing at the fix: drop the `ports` mapping or the
profile that adds it, and point `RIKU_UPSTREAM_PORT` at the app's own proxy.

Docs gain a short "app ships its own proxy" section, using
a real compose app's `proxy` service as the example.

## Order

Part 1, then Part 2, then Part 3. Each lands as its own PR.

## Left to the builder

- Exact Caddyfile layout inside each site file.
- Whether hostname validation lives in `riku-util` or the router crate.
- Test layout, following the repo rules (sibling `_tests.rs`, no `?`).

## Calls made without the owner

- Legacy key warning instead of a hard failure.
- Caddy built in rather than a shell plugin.
- Internal template variables keep their names unless they collide.
