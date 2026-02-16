# Riku Plugin System

The Riku plugin system allows you to extend Riku's functionality through executable scripts or binaries.

## Overview

Plugins are executable files placed in `~/.riku/plugins/`. They can be invoked as subcommands to extend Riku's functionality.

## Plugin Location

```
~/.riku/plugins/
├── backup          # Custom backup plugin
├── metrics         # Metrics collection plugin
└── notify          # Deployment notification plugin
```

## Creating a Plugin

### Basic Structure

Plugins can be written in any language. Shell scripts are the most common:

```bash
#!/bin/bash
# ~/.riku/plugins/hello

echo "Hello from Riku plugin!"
echo "Arguments: $@"
```

Make it executable:
```bash
chmod +x ~/.riku/plugins/hello
```

### Plugin Arguments

Plugins receive command-line arguments:

```bash
#!/bin/bash
# ~/.riku/plugins/deploy-notify

APP="$1"
ACTION="$2"

echo "Deployment $ACTION for app: $APP"

# Send notification (example: curl to Slack webhook)
# curl -X POST -H 'Content-type: application/json' \
#   --data "{\"text\":\"App $APP was $ACTION\"}" \
#   $SLACK_WEBHOOK_URL
```

Usage:
```bash
riku plugin deploy-notify myapp deployed
```

### Accessing Riku Environment

Plugins can access Riku's environment and app data:

```bash
#!/bin/bash
# ~/.riku/plugins/app-info

APP="$1"
RIKU_ROOT="${RIKU_ROOT:-$HOME/.riku}"

if [ -z "$APP" ]; then
    echo "Usage: riku plugin app-info <app-name>"
    exit 1
fi

APP_DIR="$RIKU_ROOT/apps/$APP"
ENV_FILE="$RIKU_ROOT/envs/$APP/ENV"

echo "App: $APP"
echo "Directory: $APP_DIR"
echo "Environment variables:"
if [ -f "$ENV_FILE" ]; then
    cat "$ENV_FILE"
else
    echo "  (no ENV file)"
fi
```

## Plugin Hooks (Future)

Future versions may support automatic plugin hooks:

- `pre-deploy` - Run before deployment
- `post-deploy` - Run after successful deployment
- `pre-restart` - Run before app restart
- `post-restart` - Run after app restart

Example hook plugin:
```bash
#!/bin/bash
# ~/.riku/plugins/hooks/post-deploy

APP="$1"

# Log deployment
echo "$(date): Deployed $APP" >> ~/.riku/deployments.log

# Send notification
curl -X POST "https://hooks.example.com/deploy?app=$APP"
```

## Example Plugins

### 1. Backup Plugin

```bash
#!/bin/bash
# ~/.riku/plugins/backup

APP="$1"
RIKU_ROOT="${RIKU_ROOT:-$HOME/.riku}"
BACKUP_DIR="${BACKUP_DIR:-$HOME/backups}"

if [ -z "$APP" ]; then
    echo "Usage: riku plugin backup <app-name>"
    exit 1
fi

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/${APP}_${TIMESTAMP}.tar.gz"

mkdir -p "$BACKUP_DIR"
tar -czf "$BACKUP_FILE" -C "$RIKU_ROOT/apps" "$APP"

echo "Backup created: $BACKUP_FILE"
```

### 2. Metrics Plugin

```bash
#!/bin/bash
# ~/.riku/plugins/metrics

APP="$1"
RIKU_ROOT="${RIKU_ROOT:-$HOME/.riku}"
LOG_DIR="$RIKU_ROOT/logs/$APP"

if [ -z "$APP" ]; then
    echo "Usage: riku plugin metrics <app-name>"
    exit 1
fi

echo "=== Metrics for $APP ==="
echo ""

# Count requests in logs
if [ -d "$LOG_DIR" ]; then
    REQUEST_COUNT=$(find "$LOG_DIR" -name "*.log" -exec cat {} \; | wc -l)
    echo "Total log entries: $REQUEST_COUNT"
fi

# Check disk usage
APP_SIZE=$(du -sh "$RIKU_ROOT/apps/$APP" 2>/dev/null | cut -f1)
echo "App size: $APP_SIZE"

# Count processes
PROC_COUNT=$(ls "$RIKU_ROOT/workers-enabled/${APP}_"* 2>/dev/null | wc -l)
echo "Worker processes: $PROC_COUNT"
```

### 3. Notification Plugin

```bash
#!/bin/bash
# ~/.riku/plugins/notify

MESSAGE="$1"
WEBHOOK_URL="${SLACK_WEBHOOK_URL:-}"

if [ -z "$MESSAGE" ]; then
    echo "Usage: riku plugin notify <message>"
    exit 1
fi

if [ -z "$WEBHOOK_URL" ]; then
    echo "SLACK_WEBHOOK_URL not set"
    exit 1
fi

curl -X POST -H 'Content-type: application/json' \
    --data "{\"text\":\"$MESSAGE\"}" \
    "$WEBHOOK_URL"

echo "Notification sent"
```

## Best Practices

1. **Error Handling**: Always handle errors gracefully
   ```bash
   if [ ! -d "$APP_DIR" ]; then
       echo "Error: App not found" >&2
       exit 1
   fi
   ```

2. **Logging**: Log plugin actions
   ```bash
   echo "$(date): Plugin action" >> ~/.riku/plugin.log
   ```

3. **Permissions**: Ensure plugins are executable
   ```bash
   chmod +x ~/.riku/plugins/*
   ```

4. **Testing**: Test plugins before using in production
   ```bash
   ~/.riku/plugins/my-plugin test-args
   ```

5. **Documentation**: Document plugin usage in comments
   ```bash
   # Usage: riku plugin my-plugin <app-name> [options]
   # Description: Does something useful
   ```

## Security Considerations

1. **Review Plugins**: Only run plugins you trust
2. **Limit Permissions**: Plugins run as the deploy user
3. **Validate Input**: Always validate plugin arguments
4. **Avoid Secrets**: Don't hardcode secrets in plugins

## Troubleshooting

### Plugin Not Found
```bash
# Check if plugin exists
ls -la ~/.riku/plugins/

# Check if executable
file ~/.riku/plugins/my-plugin
```

### Permission Denied
```bash
chmod +x ~/.riku/plugins/my-plugin
```

### Debug Plugin
```bash
# Run with bash debug
bash -x ~/.riku/plugins/my-plugin args
```

## Future Enhancements

Potential future features:

- Plugin marketplace/repository
- Automatic plugin updates
- Plugin configuration via ENV
- Plugin dependencies
- Plugin sandboxing

## Contributing Plugins

Share your plugins with the community by submitting them to the Riku repository or creating a separate plugin repository.
