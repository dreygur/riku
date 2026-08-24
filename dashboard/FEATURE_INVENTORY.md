# Riku: Feature Inventory for the Dashboard

Every capability riku exposes, grouped by domain, with the **CLI** that backs it,
the **HTTP** surface (if any today), and the **dashboard surface** it should map to.

Legend for *HTTP today*:
- **yes**: endpoint exists (supervisor control plane or embedded dashboard)
- **partial**: read-only data exists, no action endpoint
- **no**: CLI-only, needs a control-plane/proxy endpoint to drive from the browser

The dashboard's Next.js server runs on the same host as riku, so "no" items can
be exposed either by (a) adding a control-plane route, or (b) the Next API proxy
shelling out to the `riku` binary. Prefer (a) for anything mutating.

---

## 1. App lifecycle

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| List apps + state matrix | `riku apps` / `__dump-state` | yes, `GET /api/state`, `GET /metrics/apps` | App list / overview grid |
| App detail | `riku apps info <app>` | yes, (state) | App detail page |
| Create app | `riku apps create <app>` | yes, `POST /control/apps` | "New app" action |
| Deploy / redeploy | `riku deploy <app>` | yes, `POST /control/apps/:app/deploy` | Redeploy button |
| Restart | `riku restart <app>` | yes, `POST /control/apps/:app/restart` | Restart button |
| Stop | `riku stop <app>` | yes, `POST /control/apps/:app/stop` | Stop button |
| Destroy | `riku destroy <app>` | yes, `DELETE /control/apps/:app` | Destroy (guarded) |
| Rollback + history | `riku rollback <app> [--to] [--list]` | yes, `POST …/rollback`, `GET …/releases` (embedded) | Release timeline + roll back |
| Run one-off command | `riku run <app> <cmd…>` | no | Console / "run command" |

## 2. Logs

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Live worker + deploy logs | `riku logs <app> [proc]` | yes, `GET /api/apps/:app/logs` (SSE, embedded) | Live log console (per app + per worker) |
| Deploy log (history) | - | partial, file | Deploy timeline view |

## 3. Processes, scaling, metrics

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Process counts / status | `riku ps <app>` | yes, state / metrics | Worker table per app |
| Scale | `riku ps <app> --scale web=2` | yes, `POST …/scale` (embedded) | Inline scaler per process kind |
| Live metrics (cpu/mem/req) | `riku stats all|app` | partial, `GET /metrics`, `/metrics/apps`, `/metrics/apps/:app`, `GET /metrics/stream` (SSE) | Charts/sparklines, host gauges |

## 4. Configuration & environment

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Show / get config | `riku config show|get` | yes, `GET /api/apps/:app/env` | Env viewer |
| Set / unset config | `riku config set|unset` | yes, `POST /api/apps/:app/env` `{set,unset}` | Env editor (key/value) |
| Live (resolved) env | `riku config live` | no | "Live env" panel |
| Routing (domain, https, static, cache, cloudflare) | env keys | partial, (state routing) | Routing panel |

## 5. Plugins, addons, marketplace

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Runtime/hook plugins installed | `riku plugin list`, `riku hook list` | partial, `GET /plugins`, `GET /hooks` | Plugins panel |
| Install bundled runtimes | `riku install-plugins [--plugins]` | yes, `POST /control/plugins/install` | "Install runtimes" |
| Plugin bundles: install/list/remove | `riku plugins install|list|remove` | no | Plugin manager |
| Search / add from marketplace | `riku plugins search|add` | no | Marketplace browser |
| Marketplace add/list/remove | `riku plugins marketplace …` | no | Marketplace sources |
| Scaffold a plugin | `riku plugins scaffold` | no | (dev tool, optional) |
| Signing: keygen/sign/trust | `riku plugins keygen|sign|trust …` | no | Trust keyring panel |
| Plugins doctor | `riku plugins doctor` | no | Plugin health check |
| Addons: list/create/bind/unbind/destroy/backup | `riku addon …` | yes, `GET/POST/DELETE /api/addons[/:instance][/bind|unbind|backup]` | Addons / managed datastores |

## 6. Backups

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Backup app | `riku backup <app>` | yes, `POST /api/apps/:app/backup` | Backup action + artifact list |
| Restore app | `riku restore <app> <file>` | no | Restore (guarded, file picker) |

## 7. Containers

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Build/export app image | `riku container …` | yes, `POST /control/apps/:app/container/export` | "Export image" action |

## 8. Diagnostics & system

| Feature | CLI | HTTP today | Dashboard surface |
|---|---|---|---|
| Doctor (nginx/systemd/perms/disk/cert) | `riku doctor` | yes, `GET /api/doctor` | System health page |
| Supervisor health + uptime | - | yes, `GET /health` | Header status / host panel |
| Update binary | `riku update` | no | (system: optional) |
| Init server | `riku init` | no | (setup: out of scope) |

## 9. Security model the dashboard must honor

- **Read-only** routes (`/health`, `/metrics*`, `/plugins`, `/hooks`, `/api/state`,
  releases, logs), open on loopback; require token off-loopback.
- **Mutating** routes (`/control/*`, scale, rollback, env writes), require the
  control token (`~/.riku/control.token`) and a same-origin/CSRF check. The
  Next proxy deputizes the token server-side; the browser never sees it.
- Status wire format is **snake_case** (`running`, `oom_killed`, …).

## 10. Now wired on the embedded `/api/*` server

Added so the dashboard can drive them without the CLI: **env** (`GET`/`POST
/api/apps/:app/env`), **addons** (`/api/addons*`), **app backup**
(`POST /api/apps/:app/backup`), **doctor** (`GET /api/doctor`).

Still CLI-only (add when a panel needs them): `config live`, `run`,
plugin install/list/remove/search/add, marketplace `*`, trust keyring,
plugins doctor, `restore`, plugins scaffold/keygen/sign.
