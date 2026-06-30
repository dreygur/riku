# Dashboard

Riku ships a **read-only web dashboard** embedded directly in the binary — no
separate install, no Node runtime on the server. It gives you a browser view of
your apps: the app list, live log streaming, and deploy history.

## Start the dashboard

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

## Options

| Option | Description |
|--------|-------------|
| `--bind <BIND>` | Address to bind (`host:port`). Default: `127.0.0.1:8088` |
| `--token <TOKEN>` | Require this token on the API (also via `RIKU_DASHBOARD_TOKEN`) |

## Exposing it on a network

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

!!! warning "Read-only, but protect it anyway"
    The dashboard is read-only — it does not deploy or mutate apps — but it
    exposes logs and app metadata. Never bind it to a public address without a
    token, and prefer SSH tunneling or a reverse proxy with TLS for remote
    access.

## Examples

```bash
riku dashboard
riku dashboard --bind 127.0.0.1:9000
riku dashboard --bind 0.0.0.0:8088 --token <tok>
```

## Running it as a service

To keep the dashboard running, supervise it with systemd alongside the rest of
Riku. See [Systemd integration](systemd.md) for unit examples.
