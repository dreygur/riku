# AI Agent Interface

`riku agent` is a JSON interface for AI agents. Each call prints one JSON
object on stdout, checks the agent's scope, rate limits it and writes an
audit log entry. A response with an error code exits 1; every other response,
a confirmation request included, exits 0. `riku agent --schema` lists every
command, parameter and error code.

## Setting up a key

Give each agent its own key with a forced command in `~/.ssh/authorized_keys`:

```bash
command="riku agent --scope readonly --agent-id monitor",no-port-forwarding,no-pty ssh-ed25519 AAAA... monitor
command="riku agent --scope staging --agent-id cursor",no-port-forwarding,no-pty ssh-ed25519 BBBB... cursor
```

The forced command holds only `riku agent` with `--scope`, `--agent-id` and
optionally `--json` (accepted, output is always JSON). Without `--scope` the
key is read only. A forced command that names a command itself always runs
that command and ignores the request. Never use a bare `riku` forced command
for an agent: it gives the client the whole CLI. Never allow `RIKU_*`
variables through sshd `AcceptEnv`.

## Calling it

The agent sends only the command:

```bash
ssh riku@server apps
ssh riku@server ps myapp
ssh riku@server run myapp -- ls -la   # put -- before arguments that start with -
ssh riku@server agent --intro         # scope, permitted commands, version
ssh riku@server agent --agent-help deploy
ssh riku@server 'config:set myapp "MSG=hello world"'
```

Start a request that begins with an option with the word `agent`: the local
ssh reads options after the host name too, so `ssh riku@server --intro` fails
before it reaches the server. ssh joins its arguments with spaces, so a value
that holds spaces needs quotes inside the remote command, as in the last line.

A leading `riku` or `riku agent` is ignored. The request may hold only the
command, its arguments, `--confirm <token>`, `--intro`, `--schema`,
`--agent-help` and `--json`. Any other option, `--scope` and `--agent-id`
included, is refused with `INVALID_PARAMETERS`. On the server the same calls
run as `riku agent ps myapp`.

## Scopes and commands

| Scope | Adds | Rate limit |
|-------|------|------------|
| `readonly` | `apps`, `logs`, `ps`, `config:get`, `config:show`, `stats` | 60/min |
| `staging` | `deploy`, `restart`, `config:set`, `config:unset` | 30/min |
| `production` | `stop`, `destroy`, `run` | 20/min |

- `config:get` and `config:show`: a readonly agent sees secret-looking values,
  and values that reference another variable, as `***`.
- `config:set` and `config:unset` redeploy. If the redeploy fails the error is
  `DEPLOY_FAILED` and the config is already saved.
- `logs` returns the last 100 lines of each log file.
- `run` executes as the riku user, so it can do anything riku can. It uses no
  shell, gets only PATH, HOME and the app's live env, caps each output stream
  at 64 KiB and is killed after 300 seconds. It returns `exit_code`, `stdout`,
  `stderr`, `truncated` and `timed_out`; a nonzero exit is `COMMAND_FAILED`
  with the same data.

## Confirmation

`destroy`, `stop` and `run` always ask first. `config:set` and `config:unset`
ask when a key they change has a name containing `SECRET`, `PASS`, `PWD`,
`TOKEN`, `_KEY`, `AUTH`, `CREDENTIAL`, `PRIVATE` or `DATABASE_URL`. The match
is by name only, so a password inside `REDIS_URL` does not ask.

```bash
ssh riku@server destroy myapp
# {"success": false, "confirmation_required": true, "confirm_token": "9f2c...", ...}
ssh riku@server destroy myapp --confirm 9f2c...   # after the human approves
```

A token works once, for the same action and app, within 5 minutes. A `run`,
`config:set` or `config:unset` token also only works for the exact same
command or values. The token goes back to the agent that asked: riku cannot
tell whether a human saw the request.

## Audit log

Every call is logged to `~/.riku/state/agent/audit.log`, one JSON object per
line with `time`, `agent`, `action`, `app` and `status`. Commands that change
something are refused with `STATE_UNAVAILABLE` if the log cannot be opened.
The state directory is private to the riku user, so read it with an admin
login:

```bash
ssh admin@server 'tail -n 50 ~riku/.riku/state/agent/audit.log'
```
