# Plugin System

!!! tip "Looking for installable, versioned plugins?"
    This page documents the original single-file plugins. For the modern
    manifest-based bundles: addons, notifiers, `riku plugins install`,
    marketplaces, and signature verification, see
    [Plugin Bundles](plugin-bundles.md) and the [Plugin Gallery](plugin-gallery.md).

Riku supports three categories of plugins, all stored in `~/.riku/plugins/`:

1. **Runtime plugins**: detect and build your application (e.g. `node`, `python`)
2. **Lifecycle hook plugins**: run at deploy stages (e.g. `riku-pre-deploy`)
3. **Client-side plugins**: run locally, coordinate with the server

---

## Runtime Plugins

Runtime plugins handle the detect/build/env/start lifecycle for a specific language
or framework. The core binary contains no hardcoded runtimes; all language support
comes from plugins.

### Installing Bundled Plugins

```bash
riku plugins install node                     # one runtime, by name
rk plugins install node                       # the same, from your laptop
riku install-plugins                          # all default runtime plugins
riku install-plugins --plugins node,python   # specific plugins only
```

`riku plugins install <name>` with a shipped runtime name installs that
runtime; anything with a `/`, a `:` or ending in `.git` is a plugin bundle
source instead. To install a local directory that shares a runtime's name,
write it as `./node`.

The shell runtimes `node`, `python`, `ruby`, `go` and `rust-lang` are built
into the riku binary and written out on demand, so they install offline and
always match the riku version. They are the default set. The `java` and
`clojure` plugins are prebuilt binaries downloaded from the GitHub release of
the running riku version and checked against the published `.sha256` file; a
mismatch is an error and nothing is written. They are installed only when
named. Container apps need no runtime plugin. The same command also installs the
first-party **event-subscriber** bundle `riku-notify` (`riku install-plugins
--plugins riku-notify`, not installed by default), see
[Plugin Bundles](plugin-bundles.md#event-subscribers-notifiers).

### Plugin Protocol

Each runtime plugin is an executable that implements four subcommands, plus an optional fifth:

| Subcommand | Behaviour |
|---|---|
| `<plugin> detect` | exit 0 = this plugin handles the app, exit 1 = skip |
| `<plugin> build` | install dependencies; stdout/stderr streamed to deploy log |
| `<plugin> env` | print `KEY=VALUE` lines to stdout; merged into worker environment |
| `<plugin> start` | print default start command; fills a Procfile entry with no command, and runs as `web` when the app has no Procfile |
| `<plugin> destroy` | optional: remove what the plugin started outside the app directory (for example containers or networks it started); called by `riku destroy` before the app's files go, and a non-zero exit is shown as a warning |

Context is passed as environment variables:

```
RIKU_APP          app name
RIKU_APP_PATH     path to checked-out source code
RIKU_ENV_PATH     path to app's env directory (~/.riku/envs/<app>)
RIKU_ROOT         riku root (~/.riku)
```

### Naming Convention

| Prefix | Category | Example |
|--------|----------|---------|
| (none) | Runtime plugin | `node`, `python`, `my-lang` |
| `riku-` | Lifecycle hook | `riku-pre-deploy`, `riku-post-build` |

### Detection Order

1. If `RIKU_RUNTIME=<name>` is set in the app ENV → use that plugin, skip detection
2. Otherwise → run `detect` on all non-`riku-*` plugins sorted alphabetically; first exit 0 wins
3. If no plugin matches, the deploy fails. When the app's files call for a
   shipped runtime that is not installed (`package.json` for node,
   `requirements.txt` for python, and so on), or `RIKU_RUNTIME` names one, the
   error names it: `app 'myapp' needs the node plugin. Install it with: rk
   plugins install node (on the server: riku plugins install node), then push
   again.` Otherwise it says `No runtime plugin matched` and suggests
   `RIKU_RUNTIME=<name>`.

### Writing a Custom Runtime Plugin

```bash
#!/usr/bin/env bash
set -euo pipefail
CMD="${1:-}"
APP_PATH="${RIKU_APP_PATH:-$(pwd)}"
APP="${RIKU_APP:-app}"

case "$CMD" in
  detect)
    # Exit 0 if this plugin handles the app
    [ -f "$APP_PATH/my-marker.conf" ] && exit 0
    exit 1
    ;;
  build)
    cd "$APP_PATH"
    my-build-tool install --production
    ;;
  env)
    echo "MY_RUNTIME=production"
    echo "PATH=$APP_PATH/bin:$PATH"
    ;;
  start)
    echo "my-runtime server.conf"
    ;;
  *)
    echo "Unknown subcommand: $CMD" >&2
    exit 1
    ;;
esac
```

Install it:

```bash
cp my-runtime ~/.riku/plugins/my-runtime
chmod +x ~/.riku/plugins/my-runtime
```

### Pinning a Runtime

To skip auto-detection:

```bash
riku settings set myapp runtime.name=my-runtime
```

---

## Lifecycle Hook Plugins

Lifecycle hooks are named with a `riku-` prefix and run automatically at deploy stages.

| Hook | Trigger | Failure behaviour |
|------|---------|-------------------|
| `riku-pre-deploy` | Before runtime detection | Aborts deploy |
| `riku-pre-build` | After detection, before build | Aborts deploy |
| `riku-post-build` | After build, before workers | Aborts deploy |
| `riku-post-deploy` | After workers are up | Non-fatal |

Hooks receive: `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`,
and `RIKU_DETECTED_RUNTIME` (the detected plugin name, if any).

Example notification hook:

```bash
#!/bin/sh
# ~/.riku/plugins/riku-post-deploy
curl -s -X POST "$SLACK_WEBHOOK_URL" \
  -d "{\"text\": \"Deployed $RIKU_APP\"}" || true
```

---

Riku does not run unknown subcommands as plugins: `riku hello` is an error.
To run your own code on the server, use a lifecycle hook above, an event
subscriber (see [Plugin Bundles](plugin-bundles.md)), or `riku run`.

---

## Client-Side Plugins

A client plugin replaces one riku command on your laptop. Riku looks for
`~/.riku/client-plugins/<command>` and, when it exists and is executable,
runs it instead of the built in command. Only these ten commands can be
replaced: `apps`, `config`, `deploy`, `destroy`, `logs`, `ps`, `stats`,
`run`, `restart`, `stop`. A file with any other name is never run.

### Plugin Location

```
~/.riku/client-plugins/
├── logs
└── deploy
```

### Plugin Interface

Client plugins receive these arguments:

| Argument | Description | Example |
|----------|-------------|---------|
| `$1` | Server address, empty when the command runs locally | `riku@edge.example` |
| `$2` | App name, empty when left out | `myapp` |
| `$3` | Command | `logs`, `config:get:PORT`, `config:set` |
| `$4+` | Additional arguments | (varies) |

### Example Client Plugin

Keeps a local copy of every log you read:

```bash
#!/bin/sh
# ~/.riku/client-plugins/logs

server="$1"
app="$2"
shift 3

ssh "$server" logs "$app" "$@" | tee -a "./$app.log"
```

Make it executable, then `riku logs myapp` runs it:

```bash
chmod +x ~/.riku/client-plugins/logs
```

---

## Important: The Riku Shell

The deploy user on the server has a **restricted shell** that only accepts Riku commands, not arbitrary shell commands.

### What Works

```bash
# Riku commands work
ssh "$server" logs "$app"
ssh "$server" config:get "$app" VAR
ssh "$server" run "$app" "python manage.py migrate"

# SCP works (bypasses shell)
scp "$server:.riku/apps/$app/file" .
```

Port forwarding does not work through the deploy key: it is installed with
`no-port-forwarding`, so `ssh -N -L ...` to it is refused. Forward ports
through an admin login on the server instead.

### What Doesn't Work

```bash
# Direct shell commands fail
ssh "$server" "cat ~/.riku/apps/$app/file"  # Fails
ssh "$server" "ls -la"  # Fails
```

### Workarounds

| Method | Works? | Notes |
|--------|--------|-------|
| `ssh "$server" logs "$app"` | Yes | `logs` is a Riku command |
| `ssh "$server" config:get "$app" VAR` | Yes | `config:get` is a Riku command |
| `ssh "$server" "cat file"` | No | `cat` is not a Riku command |
| `scp "$server:file" .` | Yes | SCP bypasses the Riku shell |
| `ssh -N -L port:localhost:port "$server"` | No | The deploy key has `no-port-forwarding`; use an admin login |

---

## Managing Plugins

### List Installed Plugins

```bash
riku plugin list     # client plugins in ~/.riku/client-plugins/
riku hook list       # lifecycle hooks in ~/.riku/plugins/
riku plugins list    # installed plugin bundles
```

### Check if a Client Plugin Exists

```bash
riku plugin exists <name>
```

### Install a Plugin

```bash
# Create plugins directory
mkdir -p ~/.riku/plugins
mkdir -p ~/.riku/client-plugins

# Copy plugin
cp my-plugin ~/.riku/plugins/
chmod +x ~/.riku/plugins/my-plugin

# Or install from URL
curl -sL https://example.com/plugin > ~/.riku/plugins/plugin-name
chmod +x ~/.riku/plugins/plugin-name
```

### Remove a Plugin

```bash
rm ~/.riku/plugins/plugin-name
rm ~/.riku/client-plugins/plugin-name
```

---

## Security Considerations

1. **Review plugins** - Only install plugins from trusted sources
2. **Limit permissions** - Plugins run as the deploy user
3. **Validate input** - Always sanitize arguments
4. **Avoid secrets** - Don't log sensitive environment variables

---

## Troubleshooting

### Plugin Not Found

1. Check it's in the right directory
2. Ensure it's executable: `chmod +x plugin-name`
3. Verify the name matches the command

### Plugin Fails

1. Check permissions: `ls -la ~/.riku/plugins/`
2. Test manually: `~/.riku/plugins/plugin-name app-name`
3. Check logs: `riku logs <app>`

### Permission Denied

Ensure the plugin is executable:

```bash
chmod +x ~/.riku/plugins/plugin-name
chmod +x ~/.riku/client-plugins/plugin-name
```

---

## See Also

- [CLI Reference](cli.md) - All Riku commands
- [Environment Variables](env.md) - Configure plugins via ENV
- [Examples](https://github.com/dreygur/riku/tree/main/examples/client-plugins) - Sample plugins
