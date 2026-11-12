---
name: riku
description: Use when deploying, configuring, inspecting or debugging apps on a riku server (git push micro-PaaS), when a repo has a riku git remote, riku.toml or Procfile, or when running riku, rk or "riku agent" commands over ssh.
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

`config` holds the app's own env vars. `settings` holds riku keys (`routing.*`, `runtime.*`, `workers.*`, `health.*`, `limits.*`, `secrets.*`). `config set` refuses `RIKU_*` keys and names the `settings` key to use. Precedence per key, later wins: repo `riku.toml`, `riku settings`; a settings key written into ENV is ignored. Server-only keys (`limits.*`, `secrets.*`, `routing.bind_address`) are refused in the repo `riku.toml`.

Also: `riku addon` (databases, caches), `riku plugins` (bundles, marketplaces), `riku doctor`, `sudo riku init`. Each has `--help` with examples.

## Agent interface (JSON)

Through an agent key send only the command (`ssh riku@host ps myapp`); on the server use `riku agent ps myapp`. Each call prints one JSON object; an error exits 1. Start with `ssh riku@host agent --intro` (your scope and commands) and `agent --schema` (every command and error code); a bare `ssh riku@host --intro` is eaten by the local ssh.

```bash
ssh riku@host 'config:set myapp "MSG=hello world"'   # quote values with spaces inside the remote command
ssh riku@host config:set myapp DEBUG=1         # redeploys
ssh riku@host run myapp -- python manage.py migrate
ssh riku@host destroy myapp                    # returns confirm_token
ssh riku@host destroy myapp --confirm <token>  # only after a human approves
```

`destroy`, `stop`, `run` and changes to secret-looking config keys return a confirm token first. Show the request to the human before sending `--confirm`; riku cannot check that you did. Details: `docs/docs/ai-agents.md`.

## Common Mistakes

| Mistake | Fix |
| --- | --- |
| `riku --json ps myapp` | Only `riku agent` speaks JSON, and always does: `riku agent ps myapp` |
| `riku agent run myapp ls -la` | Arguments starting with `-` need `--`: `riku agent run myapp -- ls -la` |
| Treating a confirmation request as failure | It exits 0 with `confirmation_required: true`; ask the human, then pass `--confirm` |
| Retrying config:set after `DEPLOY_FAILED` | The config was saved; fix the deploy and run `deploy`, do not set again |
| `run myapp "cd x && make"` | No shell: `run myapp -- sh -c "cd x && make"` |
| `riku config set myapp RIKU_DOMAINS=...` | `riku settings set myapp routing.domains=...` |
| Expecting `destroy` to confirm | Plain `riku destroy` deletes immediately. Ask the human first |
| Using `--local` with `rk` | `rk` has no `--local`; use `riku` on the server |
| Both `Procfile` and `[processes]` | Keep one |
