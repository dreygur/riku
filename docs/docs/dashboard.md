# Dashboard

Riku has two dashboards, both talking to the same backend API
(`crates/riku-dashboard`):

1. **Embedded dashboard** — a single vanilla HTML/JS page baked into the
   `riku` binary itself. No separate install, no Node runtime on the server.
   Started with `riku dashboard`, documented below.
2. **Browser dashboard** (`dashboard/`) — a fuller Next.js UI (app cards,
   metrics, marketplace, addons, log streaming) deployed as its own process.
   See [`dashboard/README.md`](https://github.com/dreygur/riku/blob/main/dashboard/README.md)
   for how to build and run it.

Neither is read-only: both can call the backend's mutating routes (addon
provision/bind/unbind/deprovision, plugin install, release rollback, and
more) — **always** put a token or password in front of either one before
exposing it beyond loopback.

## Embedded dashboard

```bash
# Binds to 127.0.0.1:8088 by default (local only)
riku dashboard
```

By default it listens on loopback, so it is only reachable from the server
itself. Use SSH port-forwarding to view it from your laptop:

```bash
ssh -L 8088:127.0.0.1:8088 deploy@your-server
# then open http://localhost:8088 in your browser
```

### Options

| Option | Description |
|--------|-------------|
| `--bind <BIND>` | Address to bind (`host:port`). Default: `127.0.0.1:8088` |
| `--token <TOKEN>` | Require this token on the API (also via `RIKU_DASHBOARD_TOKEN`) |

### Exposing it on a network

If you bind to a non-loopback address, **always set a token**:

```bash
riku dashboard --bind 0.0.0.0:8088 --token "$(openssl rand -hex 32)"
```

The same token can be supplied through the `RIKU_DASHBOARD_TOKEN` environment
variable instead of the flag:

```bash
export RIKU_DASHBOARD_TOKEN="$(openssl rand -hex 32)"
riku dashboard --bind 0.0.0.0:8088
```

!!! warning "Not read-only — protect it"
    The dashboard can deploy, roll back, restart, and manage addons — it is
    not a passive viewer. Never bind it to a public address without a token,
    and prefer SSH tunneling or a reverse proxy with TLS for remote access.

### Examples

```bash
riku dashboard
riku dashboard --bind 127.0.0.1:9000
riku dashboard --bind 0.0.0.0:8088 --token <tok>
```

### Running it as a service

To keep the dashboard running, supervise it with systemd alongside the rest of
Riku. See [Systemd integration](systemd.md) for unit examples.

## Browser dashboard (`dashboard/`)

The Next.js app in `dashboard/` proxies the same backend API server-side
(`app/api/riku/[...path]/route.ts` attaches `RIKU_DASHBOARD_TOKEN` so it never
reaches the browser), and adds its own login layer on top — a single shared
password, independent of the backend token above.

```bash
# generates a scrypt hash to put in the environment, never the plaintext
nub run hash-password -- 'your-chosen-password'
# → RIKU_DASHBOARD_PASSWORD_HASH=<salt>:<hash>

export RIKU_DASHBOARD_PASSWORD_HASH=<salt>:<hash>
export RIKU_DASHBOARD_TOKEN=<the backend token from above>
nub run build && nub run start
```

If `RIKU_DASHBOARD_PASSWORD_HASH` is unset, the login layer is fully bypassed
(matches every other Riku feature's "don't break default/local usage" default)
— set it before exposing this beyond localhost. Visiting any page without a
valid session redirects to `/login`; the `/api/riku/*` proxy itself returns
`401` rather than redirecting. See `dashboard/README.md` for the full
environment variable reference.
