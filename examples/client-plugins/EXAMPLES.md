# Example Client Plugins

This directory contains example client plugins for Riku. Client plugins extend the `riku` command with local functionality.

## Available Examples

| Plugin | Description | Use Case |
|--------|-------------|----------|
| `open` | Open app in browser | Quick access to deployed apps |
| `backup` | Download app files | Backup and disaster recovery |
| `tunnel` | Create SSH tunnels | Database access, admin panels |
| `example` | Template plugin | Learn plugin development |

## Installation

To install any plugin:

```bash
# Create the plugins directory
mkdir -p ~/.riku/client-plugins

# Copy a plugin
cp examples/client-plugins/open ~/.riku/client-plugins/

# Make it executable
chmod +x ~/.riku/client-plugins/open
```

## Testing Plugins

After installation, test a plugin:

```bash
# List installed plugins
riku plugin list

# Check if plugin exists
riku plugin exists open

# Use the plugin
riku open myapp
```

## Plugin Details

### open

Opens your app's URL in the default browser.

```bash
# Basic usage
riku open myapp

# With path
riku open myapp /admin
```

**Features:**
- Auto-detects browser (Chrome, Firefox, Safari, etc.)
- Cross-platform (macOS, Linux, Windows)
- Uses HTTPS by default
- Falls back to app name if NGINX_SERVER_NAME not set

### backup

Downloads app files to your local machine.

```bash
# Backup to current directory
riku backup myapp

# Backup to custom directory
riku backup myapp ./backups

# Backup only data directory
riku backup:data myapp
```

**Output:**
```
Backing up myapp to ./myapp-20240101-120000/...
Done: ./myapp-20240101-120000
total 24
-rw-r--r-- 1 user user 1234 Jan 1 12:00 app.py
-rw-r--r-- 1 user user  567 Jan 1 12:00 requirements.txt
```

### tunnel

Creates SSH tunnels to access server services.

```bash
# Forward port 8000
riku tunnel myapp 8000

# Forward different local port
riku tunnel myapp 8000 3000

# Database tunnel (postgres)
riku tunnel:db myapp postgres

# Database tunnel (redis)
riku tunnel:db myapp redis

# Custom port
riku tunnel:db myapp 9000
```

**Use cases:**
- Access remote databases locally
- Connect to admin interfaces
- Debug services not exposed publicly

### example

Template plugin showing the plugin interface.

```bash
# Basic example
riku example myapp

# Status subcommand
riku example:status myapp

# Download a file
riku example:download myapp config.txt

# Upload a file
riku example:upload myapp local.txt

# Create tunnel
riku example:tunnel myapp 8000
```

**Learn from this plugin:**
- Argument parsing
- Subcommand handling
- SSH integration
- Error handling
- User feedback

## Writing Your Own Plugins

See the main [README.md](README.md) for plugin development guide.

Quick start:

```bash
# Copy the example template
cp example ~/.riku/client-plugins/myplugin

# Edit with your logic
nano ~/.riku/client-plugins/myplugin

# Make executable
chmod +x ~/.riku/client-plugins/myplugin

# Test it
riku myplugin myapp
```

## Plugin Interface

Plugins receive arguments:

| Position | Variable | Example |
|----------|----------|---------|
| $1 | server | `deploy@server.com` |
| $2 | app | `myapp` |
| $3 | command | `backup:data` |
| $4+ | extra | file paths, options |

## Troubleshooting

**Plugin not found:**
```bash
# Check installation
ls -la ~/.riku/client-plugins/

# Verify executable
file ~/.riku/client-plugins/myplugin
```

**Permission denied:**
```bash
chmod +x ~/.riku/client-plugins/myplugin
```

**Debug plugin:**
```bash
# Add set -x to see execution
head -1 ~/.riku/client-plugins/myplugin
# Change to: #!/bin/sh -x
```

## Security Notes

1. **Review plugins** before installing
2. **Check permissions** - plugins run as your user
3. **Validate inputs** - don't trust user arguments
4. **Use SSH safely** - prefer key-based auth

## Contributing

Share your plugins with the community! Submit them as examples or create a plugin repository.
