# caddy-router

A **router** plugin that swaps Riku's built-in nginx generation for
[Caddy](https://caddyserver.com), which provides automatic HTTPS out of the box.

Router plugins implement the `router` seam (`PLUGIN_PROTOCOL.md` §6.2). The
router is a host-level **singleton** — exactly one is active, selected with the
`RIKU_ROUTER` environment variable.

## Requirements

- `caddy` on the host (running, with its admin API reachable for `caddy reload`)
- `jq` for parsing the request JSON

## Install

```sh
riku plugins install ./examples/plugins/caddy-router
```

## Activate

Set `RIKU_ROUTER` in the environment Riku runs under (e.g. the systemd unit or
the deploy user's profile):

```sh
export RIKU_ROUTER=caddy
```

With `RIKU_ROUTER` unset (or `nginx`), Riku uses its built-in nginx router and
this plugin stays dormant.

## Verbs

| Verb        | Input (stdin JSON)                        | Effect                                   |
| ----------- | ----------------------------------------- | ---------------------------------------- |
| `configure` | `{app, domains, upstream_port, https}`    | writes `$RIKU_ROOT/caddy/sites/<app>.caddy` |
| `reload`    | —                                         | `caddy reload` with the generated Caddyfile |

Per-app config is shaped from the app's `ENV` (`NGINX_SERVER_NAME` → domains,
`PORT`/`NGINX_INTERNAL_PORT` → upstream port, `NGINX_HTTPS_ONLY` → https).

## Teardown

API v1 has no per-app router teardown verb, so `riku destroy` issues a best-effort
`reload`. Caddy keeps serving the app until you remove its site file and reload;
remove `$RIKU_ROOT/caddy/sites/<app>.caddy` to drop it fully.
