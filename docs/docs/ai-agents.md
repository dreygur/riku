# AI Agent Interface

`riku agent` is a JSON interface for AI agents (Claude, Cursor, Copilot and
others). Each call prints one JSON object on stdout, checks the agent's
permission scope, rate limits it and writes an audit log entry.

---

## Calling it

Over ssh, through a key whose forced command is `riku agent --scope ...`
(see [SSH key setup](#ssh-key-setup)), the agent sends only the command:

```bash
ssh riku@server apps
ssh riku@server ps myapp
ssh riku@server config:get myapp DATABASE_URL
ssh riku@server run myapp -- ls -la   # put -- before arguments that start with -
ssh riku@server --intro               # scope, permitted commands, version
ssh riku@server --schema              # full command schema (JSON)
ssh riku@server --agent-help deploy   # help for one command
```

A leading `riku` or `riku agent` in the request is ignored, so
`ssh riku@server riku agent ps myapp` works too. The request may only hold the
command, its arguments, `--confirm <token>`, `--intro`, `--schema`,
`--agent-help` and `--json`. Any other option, `--scope` and `--agent-id`
included, is refused with `INVALID_PARAMETERS`: scope and identity only ever
come from the forced command.

On the server itself the same commands run as `riku agent ps myapp`.

`--json` is accepted so existing forced commands keep working, but changes
nothing: the output is always JSON. It is a flag of `riku agent`, not of
`riku` itself, so `riku --json ps` is an error.

### Exit status

A response that carries an error exits 1, and riku also prints
`Error: agent <command> failed: <CODE>` on stderr. Every other response exits
0, including a confirmation request. Read `success` and `error.code` from
stdout for the details.

---

## Scopes

| Scope | Commands |
|-------|----------|
| `readonly` | `apps`, `logs`, `ps`, `config:get`, `config:show`, `stats` |
| `staging` | readonly plus `deploy`, `restart`, `run`, `config:set`, `config:unset` |
| `production` | staging plus `stop`, `destroy` |

A call without `--scope` is read only. The scope comes from that key's own
forced command, never from another line in `authorized_keys` or from the
environment. The name in the audit log comes from `--agent-id` in the same
forced command. Never allow `RIKU_*` variables through sshd `AcceptEnv`.

The forced command of an agent key must start with `riku agent`. A bare `riku`
forced command gives the client full access and lets it choose everything.

---

## Commands

| Command | Arguments | Notes |
|---------|-----------|-------|
| `apps` | | Lists apps, `running` is true when workers are enabled |
| `deploy` | `<app>` | Runs the deploy and returns when it finishes |
| `destroy` | `<app>` | Needs confirmation |
| `config:get` | `<app> <key>` | |
| `config:show` | `<app>` | |
| `config:set` | `<app> KEY=value ...` | Redeploys. Secret-looking keys need confirmation |
| `config:unset` | `<app> KEY ...` | Redeploys. Secret-looking keys need confirmation |
| `logs` | `<app> [process]` | Last 100 lines of each log file |
| `ps` | `<app>` | |
| `restart` | `<app> [process]` | |
| `stop` | `<app>` | Needs confirmation |
| `run` | `<app> <command>...` | No shell, live env, app directory. Returns `exit_code`, `stdout`, `stderr`, `truncated`, `timed_out`. Each stream is capped at 64 KiB and the command is killed after 300 seconds |
| `stats` | `[app]` | All apps when no app is given |

---

## Responses

Success:

```json
{"success": true, "confirmation_required": false, "data": {"apps": [{"name": "myapp", "running": true}]}}
```

Error:

```json
{"success": false, "confirmation_required": false, "error": {"code": "APP_NOT_FOUND", "message": "Application 'myapp' not found"}}
```

Confirmation request:

```json
{"success": false, "confirmation_required": true, "message": "Human confirmation required for destroy", "confirm_token": "9f2c..."}
```

### Error codes

| Code | Meaning |
|------|---------|
| `APP_NOT_FOUND` | Application does not exist |
| `KEY_NOT_FOUND` | Configuration key is not set |
| `PERMISSION_DENIED` | The scope does not allow the command |
| `INVALID_COMMAND` | Unknown command |
| `INVALID_PARAMETERS` | Missing or invalid arguments |
| `INVALID_TOKEN` | Confirmation token is unknown, used, expired or for another action |
| `RATE_LIMIT_EXCEEDED` | Too many requests |
| `STATE_UNAVAILABLE` | Rate limit or audit state cannot be read or written |
| `COMMAND_FAILED` | `run` could not start, exited nonzero or timed out; `data` still holds the output |
| `CONFIG_FAILED` | The ENV file could not be written; nothing changed |
| `DEPLOY_FAILED` | Deploy failed. After `config:set` or `config:unset` the config was saved |
| `RESTART_FAILED`, `STOP_FAILED`, `DESTROY_FAILED` | The operation itself failed |

---

## Confirmation flow

`destroy` and `stop` always ask for confirmation first. `config:set` and
`config:unset` ask when a key name contains `SECRET`, `PASSWORD`, `TOKEN`,
`_KEY` or `DATABASE_URL`.

```bash
ssh riku@server destroy myapp
# {"confirmation_required": true, "confirm_token": "9f2c...", ...}

# The agent asks the human. After they approve:
ssh riku@server destroy myapp --confirm 9f2c...
```

A token is used once, is bound to its action and app, and expires after 5
minutes. A `config:set` or `config:unset` token is also bound to the exact
keys and values, so it cannot be spent on a different change.

---

## SSH key setup

Each line in `~/.ssh/authorized_keys` can restrict a key:

```bash
# Read only (monitoring, logs, config:get)
command="riku agent --scope readonly --agent-id monitor",no-port-forwarding,no-pty ssh-ed25519 AAAA... monitor

# Staging (can deploy and restart)
command="riku agent --scope staging --agent-id cursor-staging",no-port-forwarding,no-pty ssh-ed25519 BBBB... cursor-staging
```

| Option | Purpose |
|--------|---------|
| `command="..."` | Fix the command and its scope |
| `no-port-forwarding` | Prevent ssh tunnels |
| `no-pty` | No interactive shell |
| `no-X11-forwarding` | Disable X11 forwarding |
| `from="IP"` | Only allow these addresses |

The forced command must hold only `riku agent` with `--scope`, `--agent-id`
and `--json`. Then the client's request supplies the command. A forced command
that names a command itself (`riku agent --scope readonly apps`) always runs
that command and ignores the request.

---

## Audit log

Every call is logged to `~/.riku/state/agent/audit.log`, one JSON object per
line with the fields `time`, `agent`, `action`, `app` and `status`. Control
characters in a client supplied value are escaped, so an entry cannot be split
or forged.

```
{"time":"2026-02-23T14:30:00+00:00","agent":"cursor-staging","action":"deploy","app":"myapp","status":"success"}
```

Commands that change something open the audit log before they run and are
refused with `STATE_UNAVAILABLE` if it cannot be opened. Read only commands
still run if the log cannot be written. The state directory is readable only
by the riku user. Read it with an admin login:

```bash
ssh admin@server 'tail -n 50 ~riku/.riku/state/agent/audit.log'
```

---

## Rate limits

| Scope | Commands per minute |
|-------|---------------------|
| `readonly` | 60 |
| `staging` | 30 |
| `production` | 20 |

Over the limit the response is `RATE_LIMIT_EXCEEDED`.

---

## Security notes

- Give each agent its own key and the smallest scope that works.
- Never share agent keys between systems; rotate them and revoke a leaked key at once.
- Review the audit log, and alert on `destroy` and `stop`.

---

## See also

- [CLI Reference](cli.md)
- [Environment Variables](env.md)
- [Plugin System](plugins.md)
