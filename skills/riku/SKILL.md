---
name: riku
description: Use when deploying, configuring, inspecting or debugging apps on a riku server (git push micro-PaaS, Piku port), when a repo has a riku git remote, riku.toml or Procfile, or when running riku, rk or "riku agent" commands over ssh.
---

# Riku

## Overview

Riku is a single-binary micro-PaaS: `git push` to the server deploys the app. Two binaries:

- `riku`: full binary. On the server it runs everything. On a laptop it forwards server commands over ssh to a registered server.
- `rk`: laptop-only client. Forwards only, never runs server code, has no `--local`.

Run `riku <command> --help` before guessing flags. Every subcommand prints real examples.

## Targeting

| Situation | What riku does |
| --- | --- |
| Inside a repo whose remote is `riku@host:app` | Server and app taken from the remote; `riku restart` is enough |
| `-s <name>` | Run on that registered server |
| `--local` (`riku` only) | Run on this machine |
| Default server set | Used when nothing else selects one |

The chosen target is printed on stderr as `[*] name (user@host)`. `rk` with no server exits with `no server selected`.

## Deploy

```bash
git remote add riku riku@edge.example:myapp
git push riku main            # deploys, prints "Live at http://..."
riku logs myapp --deploy -f   # follow the deploy log
```

Processes come from `riku.toml` `[processes]` or a `Procfile`, never both (deploy stops if both exist). `release` runs once per deploy. Cron schedules are UTC.

## Quick Reference

Laptop setup:

```bash
riku server add edge riku@edge.example [--admin root@edge.example] [--key ~/.ssh/id.pub]
riku server list | default edge | remove edge
riku quickstart myapp --runtime node --remote riku@edge.example
```

Apps and processes:

```bash
riku apps                      # list
riku apps create myapp
riku apps info myapp
riku ps [myapp] [-v]           # status, -v adds health
riku ps myapp --scale web=2 worker=1
riku logs myapp [web]          # runtime logs, optional process filter
riku restart myapp [service]   # service = one container service
riku stop myapp [service]
riku deploy myapp              # force redeploy
riku run myapp python manage.py shell
riku stats all | riku stats app myapp
riku rollback myapp --list | riku rollback myapp [--to <sha>]
riku backup myapp [--out file.tar.gz] | riku restore myapp file.tar.gz
riku destroy myapp [--force]   # no prompt, app name always required
```

Configuration (two different stores):

```bash
riku config show|live myapp            # app ENV vars
riku config get myapp KEY
riku config set myapp K1=v1 K2=v2      # redeploys
riku config unset myapp K1             # redeploys
riku settings show myapp               # riku settings, secrets masked
riku settings set myapp routing.domains=a.example,b.example limits.max_memory_mb=1024
riku settings get|unset myapp <key>
cat fullchain.pem key.pem | riku certs set myapp   # own TLS cert
riku certs show|unset myapp
```

`config` holds the app's own env vars. `settings` holds riku keys (`routing.*`, `runtime.*`, `workers.*`, `health.*`, `limits.*`, `secrets.*`). `config set` refuses `RIKU_*` keys and names the `settings` key to use. Precedence per key, later wins: repo `riku.toml`, legacy `RIKU_*` in ENV, `riku settings`. Server-only keys (`limits.*`, `secrets.*`, `routing.bind_address`) are refused in the repo `riku.toml`.

Addons and plugins:

```bash
riku addon create postgres db1 | bind db1 myapp | unbind db1 myapp | backup db1 | destroy db1
riku plugins add postgres[@marketplace] | install <dir|github:owner/repo> | list | remove <name> | search
riku hook list | riku plugin list      # server hooks | laptop client plugins
```

Server admin (run on the server):

```bash
sudo riku init [--no-systemd] [--ssh-key FILE]
sudo riku deploy-key add 'ssh-ed25519 AAAA... me@laptop'
riku doctor [--fix]
riku dashboard [--bind 127.0.0.1:8088] [--token T]    # on laptop: opens browser, needs --admin server
```

## Agent interface (JSON)

`riku agent` prints one JSON object per call on stdout. A response with an error code exits 1; success and confirmation requests exit 0.

```bash
riku agent --intro                          # scope and permitted commands
riku agent --schema                         # full JSON command schema
riku agent ps myapp
riku agent config:get myapp KEY
riku agent logs myapp [web]                 # newest 100 lines per log file
riku agent destroy myapp                    # returns confirm_token
riku agent destroy myapp --confirm <token>  # only after a human approves
```

Scopes: `readonly` (apps, logs, ps, config:get, config:show, stats), `staging` (adds deploy, restart), `production` (adds stop, destroy). `run` and `config:set` return `NOT_IMPLEMENTED`; use `riku run` and `riku config set`. Scope comes from the forced command in `authorized_keys`; no `--scope` means readonly. The forced-command path over ssh does not pass the client command through yet, so run `riku agent` on the server.

## Common Mistakes

| Mistake | Fix |
| --- | --- |
| `riku --json ps myapp` | Only `riku agent` speaks JSON, and always does: `riku agent ps myapp` |
| `riku agent run myapp ls -la` | Arguments starting with `-` need `--`: `riku agent run myapp -- ls -la` |
| Treating a confirmation request as failure | It exits 0 with `confirmation_required: true`; ask the human, then pass `--confirm` |
| `riku config set myapp RIKU_DOMAINS=...` | `riku settings set myapp routing.domains=...` |
| Expecting `destroy` to confirm | Plain `riku destroy` deletes immediately. Ask the human first |
| Using `--local` with `rk` | `rk` has no `--local`; use `riku` on the server |
| Both `Procfile` and `[processes]` | Keep one |
