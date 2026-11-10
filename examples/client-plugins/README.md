# Riku Client-Side Plugins

A client plugin replaces one riku command on your laptop. When you run
`riku <command>` (or `rk <command>`), riku looks for an executable at:

```
~/.riku/client-plugins/<command>
```

and runs it instead of the built in command. Only these ten commands can be
replaced: `apps`, `config`, `deploy`, `destroy`, `logs`, `ps`, `stats`,
`run`, `restart`, `stop`. A file with any other name is never run, and there
is no prefix matching: `riku logs:tail` does not run a `logs` plugin.

## Plugin Interface

| Argument | Description | Example |
|----------|-------------|---------|
| `$1` | Server address, empty when the command runs locally | `riku@edge.example` |
| `$2` | App name, empty when left out | `myapp` |
| `$3` | Command | `logs`, `config:get:PORT`, `config:set` |
| `$4+` | Additional arguments | (varies) |

## Example: keep a copy of every log you read

```sh
#!/bin/sh
# ~/.riku/client-plugins/logs

server="$1"
app="$2"
shift 3

ssh "$server" logs "$app" "$@" | tee -a "./$app.log"
```

```bash
mkdir -p ~/.riku/client-plugins
cp my-logs-plugin ~/.riku/client-plugins/logs
chmod +x ~/.riku/client-plugins/logs
riku plugin list          # shows installed client plugins
riku plugin exists logs
```

The `example` script in this directory shows argument handling. It only runs
once it is installed under one of the ten names above, with its `case`
changed to match that command.

## The Riku Shell

The deploy user on the server has a restricted shell that only accepts riku
commands, so a plugin talks to the server through them:

| Method | Works? | Notes |
|--------|--------|-------|
| `ssh "$server" logs "$app"` | Yes | `logs` is a riku command |
| `ssh "$server" config:get "$app" VAR` | Yes | `config:get` is a riku command |
| `ssh "$server" "cat ~/.riku/apps/$app/file"` | No | `cat` is not a riku command |
| `scp "$server:.riku/apps/$app/file" .` | Yes | scp bypasses the riku shell |
| `ssh -N -L 8000:localhost:8000 "$server"` | No | the deploy key has `no-port-forwarding`; use an admin login |

## See Also

- [Plugins](../../docs/docs/plugins.md): client plugins, lifecycle hooks, runtime plugins
