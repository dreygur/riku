# Plugin System

Riku supports three categories of plugins, all stored in `~/.riku/plugins/`:

1. **Runtime plugins** — detect and build your application (e.g. `node`, `python`)
2. **Lifecycle hook plugins** — run at deploy stages (e.g. `riku-pre-deploy`)
3. **Client-side plugins** — run locally, coordinate with the server

---

## Runtime Plugins

Runtime plugins handle the detect/build/env/start lifecycle for a specific language
or framework. The core binary contains no hardcoded runtimes; all language support
comes from plugins.

### Installing Bundled Plugins

```bash
riku install-plugins                          # all bundled plugins
riku install-plugins --plugins node,python   # specific plugins only
```

Bundled plugins available: `node`, `python`, `ruby`, `go`, `rust-lang`, `java`,
`clojure`, `container`.

### Plugin Protocol

Each runtime plugin is an executable that implements four subcommands:

| Subcommand | Behaviour |
|---|---|
| `<plugin> detect` | exit 0 = this plugin handles the app, exit 1 = skip |
| `<plugin> build` | install dependencies; stdout/stderr streamed to deploy log |
| `<plugin> env` | print `KEY=VALUE` lines to stdout; merged into worker environment |
| `<plugin> start` | print default start command; used if Procfile has no entry for a process type |

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

1. If `RUNTIME=<name>` is set in the app ENV → use that plugin, skip detection
2. Otherwise → run `detect` on all non-`riku-*` plugins sorted alphabetically; first exit 0 wins
3. If no plugin matches → deploy fails with: `"No runtime plugin matched. Run 'riku install-plugins' or set RUNTIME=<name>."`

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
riku config set myapp RUNTIME=my-runtime
```

---

## Lifecycle Hook Plugins

Lifecycle hooks are named with a `riku-` prefix and run automatically at deploy stages.

| Hook | Trigger | Failure behaviour |
|------|---------|-------------------|
| `riku-pre-deploy` | Before runtime detection | Aborts deploy |
| `riku-pre-build` | After detection, before build | Aborts deploy |
| `riku-post-build` | After build, before workers | Non-fatal (logs warning) |
| `riku-post-deploy` | After workers are up | Non-fatal |

Hooks receive: `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`,
and `RIKU_RUNTIME` (the detected plugin name, if any).

Example notification hook:

```bash
#!/bin/sh
# ~/.riku/plugins/riku-post-deploy
curl -s -X POST "$SLACK_WEBHOOK_URL" \
  -d "{\"text\": \"Deployed $RIKU_APP\"}" || true
```

---

## Server-Side Utility Plugins

Server-side plugins are executable scripts or binaries placed in `~/.riku/plugins/`. They can be invoked as subcommands to extend Riku's functionality.

### Plugin Location

```
~/.riku/plugins/
├── backup
├── tunnel
├── code
└── custom-plugin
```

### Plugin Interface

Plugins receive these arguments:

| Argument | Description | Example |
|----------|-------------|---------|
| `$1` | App name | `myapp` |
| `$2+` | Additional arguments | (varies) |

### Example Plugin

```bash
#!/bin/sh
# ~/.riku/plugins/hello

app="$1"
shift

echo "Hello from plugin! App: $app"
echo "Arguments: $@"

# Access app environment
ENV_FILE="$HOME/.riku/envs/$app/ENV"
if [ -f "$ENV_FILE" ]; then
    echo "Environment file exists"
fi
```

Make it executable:

```bash
chmod +x ~/.riku/plugins/hello
```

Invoke:

```bash
riku hello myapp --verbose
```

### Use Cases

- **Custom deployments** - Add custom build steps
- **Database migrations** - Run migrations after deploy
- **Asset compilation** - Compile assets post-deploy
- **Notifications** - Send Slack/email notifications
- **Backups** - Create automated backups
- **Health checks** - Custom health verification

---

## Client-Side Plugins

Client-side plugins extend the `riku` command with local functionality. They enable commands that require coordination between your local machine and the Riku server.

### Plugin Location

```
~/.riku/client-plugins/
├── open
├── backup
├── tunnel
└── custom-plugin
```

### Plugin Interface

Client plugins receive these arguments:

| Argument | Description | Example |
|----------|-------------|---------|
| `$1` | Server | `deploy@myserver.com` |
| `$2` | App name | `myapp` |
| `$3` | Full command | `backup:data` |
| `$4+` | Additional arguments | (varies) |

### Example Client Plugin

```bash
#!/bin/sh
# ~/.riku/client-plugins/open

server="$1"
app="$2"
cmd="$3"
shift 3

case "$cmd" in
    open)
        # Fetch the domain from server config
        domain=$(ssh "$server" config:get "$app" NGINX_SERVER_NAME)
        if [ -n "$domain" ]; then
            # Open in browser
            xdg-open "https://$domain" 2>/dev/null || \
            open "https://$domain" 2>/dev/null || \
            echo "Open https://$domain in your browser"
        else
            echo "No domain configured for $app"
            exit 1
        fi
        ;;
    *)
        echo "Unknown command: $cmd"
        exit 1
        ;;
esac
```

Make it executable:

```bash
chmod +x ~/.riku/client-plugins/open
```

Invoke:

```bash
riku open
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

# SSH with -N works (no command)
ssh -N -L 8000:localhost:8000 "$server"
```

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
| `ssh -N -L port:localhost:port "$server"` | Yes | `-N` means no command executed |

---

## Managing Plugins

### List Installed Plugins

```bash
# Server-side plugins
riku plugin list

# Client-side plugins
ls -la ~/.riku/client-plugins/
```

### Check if a Plugin Exists

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

## Example Plugins

### Backup Plugin (Client-Side)

Downloads app files to a local directory.

```bash
#!/bin/sh
# ~/.riku/client-plugins/backup

server="$1"
app="$2"
cmd="$3"
shift 3

backup_dir="./${app}-$(date +%Y%m%d-%H%M%S)"

case "$cmd" in
    backup)
        mkdir -p "$backup_dir"
        echo "Backing up $app to $backup_dir..."
        scp -r "$server:.riku/apps/$app/" "$backup_dir/"
        echo "Backup complete!"
        ;;
    backup:data)
        mkdir -p "$backup_dir"
        echo "Backing up data only..."
        scp -r "$server:.riku/data/$app/" "$backup_dir/data/"
        echo "Data backup complete!"
        ;;
    *)
        echo "Usage: riku backup [data]"
        exit 1
        ;;
esac
```

### Tunnel Plugin (Client-Side)

Creates SSH port tunnels to access services on the server.

```bash
#!/bin/sh
# ~/.riku/client-plugins/tunnel

server="$1"
app="$2"
cmd="$3"
local_port="$4"
remote_port="$5"
shift 5

case "$cmd" in
    tunnel)
        if [ -z "$local_port" ] || [ -z "$remote_port" ]; then
            echo "Usage: riku tunnel <local_port> <remote_port>"
            exit 1
        fi
        echo "Creating tunnel: localhost:$local_port -> server:$remote_port"
        ssh -N -L "$local_port:localhost:$remote_port" "$server"
        ;;
    tunnel:db)
        case "$local_port" in
            postgres|pg)
                remote_port=5432
                ;;
            redis)
                remote_port=6379
                ;;
            mysql)
                remote_port=3306
                ;;
            *)
                echo "Unknown database: $local_port"
                exit 1
                ;;
        esac
        echo "Creating tunnel for $local_port (port $remote_port)..."
        ssh -N -L "$remote_port:localhost:$remote_port" "$server"
        ;;
    *)
        echo "Usage: riku tunnel <local_port> <remote_port>"
        echo "       riku tunnel:db <postgres|redis|mysql>"
        exit 1
        ;;
esac
```

### Migration Plugin (Server-Side)

Runs database migrations after deployment.

```bash
#!/bin/sh
# ~/.riku/plugins/migrate

app="$1"
shift

ENV_FILE="$HOME/.riku/envs/$app/ENV"

# Load environment
if [ -f "$ENV_FILE" ]; then
    set -a
    . "$ENV_FILE"
    set +a
fi

echo "Running migrations for $app..."

# Detect runtime and run appropriate migrations
if [ -f "$HOME/.riku/apps/$app/manage.py" ]; then
    # Django
    cd "$HOME/.riku/apps/$app"
    python manage.py migrate
elif [ -f "$HOME/.riku/apps/$app/package.json" ]; then
    # Node.js
    cd "$HOME/.riku/apps/$app"
    npm run migrate
else
    echo "No migrations found for $app"
fi
```

---

## Combining Client and Server Plugins

Client plugins can coordinate with server-side plugins for complex workflows.

### Example: VS Code Remote

**Server plugin** (`~/.riku/plugins/code-tunnel`):
```bash
#!/bin/sh
app="$1"
tunnel_name="riku-$app"

case "$2" in
    ensure)
        # Start tunnel if not running
        if ! pgrep -f "code-tunnel.*$tunnel_name" > /dev/null; then
            code tunnel --name "$tunnel_name" &
        fi
        echo "$tunnel_name"
        ;;
    stop)
        pkill -f "code-tunnel.*$tunnel_name"
        ;;
esac
```

**Client plugin** (`~/.riku/client-plugins/code`):
```bash
#!/bin/sh
server="$1"
app="$2"
cmd="$3"
shift 3

case "$cmd" in
    code)
        # Ensure tunnel is running on server
        tunnel_name=$(ssh "$server" code-tunnel:ensure "$app")
        # Launch local VS Code
        code --remote "tunnel+$tunnel_name" "/home/deploy/.riku/apps/$app"
        ;;
    code:stop)
        ssh "$server" code-tunnel:stop "$app"
        ;;
esac
```

---

## Plugin Development Tips

1. **Keep it simple** - Plugins should do one thing well
2. **Handle errors** - Check for missing files, invalid args
3. **Provide help** - Show usage when called without args
4. **Use exit codes** - `0` for success, non-zero for errors
5. **Document** - Add comments and a README
6. **Test locally** - Test plugins before deploying

### Plugin Template

```bash
#!/bin/sh
# ~/.riku/plugins/my-plugin

# Parse arguments
app="$1"
shift

# Validate
if [ -z "$app" ]; then
    echo "Usage: riku my-plugin <app> [options]"
    exit 1
fi

# Check app exists
if [ ! -d "$HOME/.riku/apps/$app" ]; then
    echo "App '$app' not found"
    exit 1
fi

# Load environment
ENV_FILE="$HOME/.riku/envs/$app/ENV"
if [ -f "$ENV_FILE" ]; then
    set -a
    . "$ENV_FILE"
    set +a
fi

# Main logic
echo "Running my-plugin for $app..."
# Your code here

echo "Done!"
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
