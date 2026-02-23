# Systemd Integration

Riku includes optional systemd service files for automatic nginx configuration reloading.

---

## Overview

When Riku deploys applications or updates configuration, it generates nginx configuration files. The systemd path watcher can automatically reload nginx when these configurations change.

---

## Installation

### Copy Service Files

```bash
sudo cp riku-nginx.service riku-nginx.path /etc/systemd/system/
```

### Enable and Start

```bash
# Enable the path watcher
sudo systemctl enable riku-nginx.path

# Start the watcher
sudo systemctl start riku-nginx.path

# Check status
systemctl status riku-nginx.path
```

---

## How It Works

1. **Path Unit** (`riku-nginx.path`): Watches `~/.riku/nginx/` directory for changes
2. **Service Unit** (`riku-nginx.service`): Runs `nginx -s reload` when changes detected

This ensures nginx always uses the latest configuration without manual intervention.

---

## Is This Required?

**No.** This is optional. Riku can manually reload nginx after configuration changes:

```bash
# Manual nginx reload
sudo systemctl reload nginx
```

The systemd automation is convenient for:
- Production deployments
- Environments where manual reloads are error-prone
- Automated deployment pipelines

---

## Troubleshooting

### Check Path Watcher Status

```bash
systemctl status riku-nginx.path
```

### View Logs

```bash
journalctl -u riku-nginx.path
journalctl -u riku-nginx.service
```

### Test nginx Configuration

Before reloading, verify nginx config is valid:

```bash
sudo nginx -t
```

---

## See Also

- [Nginx Configuration](nginx.md) - How Riku generates nginx configs
- [CLI Reference](cli.md) - Deploy and config commands
