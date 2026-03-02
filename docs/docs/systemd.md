# Systemd Integration

Riku automatically generates and installs a systemd user service when you run `riku init`. No manual service file copying is required.

---

## Quick Start

### Enable the Service

After running `riku init`, the service file is placed at `~/.config/systemd/user/riku.service`. Enable and start it:

```bash
systemctl --user daemon-reload
systemctl --user enable riku
systemctl --user start riku

# Check status
systemctl --user status riku
```

---

## Service File

The generated service file (`~/.config/systemd/user/riku.service`) looks like:

```ini
[Unit]
Description=Riku Process Supervisor
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/riku supervisor
Restart=always
Environment=PATH=/usr/local/bin:/usr/bin:/bin

[Install]
WantedBy=default.target
```

**Note:** The binary is installed at `~/.local/bin/riku` (i.e., `%h/.local/bin/riku` in systemd notation).

---

## Nginx Auto-Reload

To automatically reload nginx when Riku generates new configurations, enable a path watcher. Create `~/.config/systemd/user/riku-nginx.path`:

```ini
[Unit]
Description=Watch for Riku nginx config changes

[Path]
PathChanged=%h/.riku/nginx

[Install]
WantedBy=default.target
```

And `~/.config/systemd/user/riku-nginx-reload.service`:

```ini
[Unit]
Description=Reload nginx on Riku config change

[Service]
Type=oneshot
ExecStart=/usr/bin/sudo /bin/systemctl reload nginx
```

Enable:

```bash
systemctl --user daemon-reload
systemctl --user enable riku-nginx.path
systemctl --user start riku-nginx.path
```

---

## Usage

### Check Status

```bash
# Check riku service status
systemctl --user status riku

# View logs
journalctl --user -u riku -f

# Check if supervisor is running
systemctl --user is-active riku
```

### Restart Service

```bash
systemctl --user restart riku
```

### Stop Service

```bash
systemctl --user stop riku
```

### View Logs

```bash
# Recent logs
journalctl --user -u riku -n 50

# Follow logs in real-time
journalctl --user -u riku -f

# Logs from today
journalctl --user -u riku --since today
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check for errors
systemctl --user status riku
journalctl --user -u riku -n 100

# Verify binary exists
ls -la ~/.local/bin/riku

# Test binary manually
~/.local/bin/riku --help
```

### Nginx Not Reloading

```bash
# Check path watcher status
systemctl --user status riku-nginx.path

# Test nginx configuration
sudo nginx -t
```

---

## Uninstall

```bash
# Stop and disable services
systemctl --user stop riku
systemctl --user disable riku

# Remove service files
rm ~/.config/systemd/user/riku.service
rm -f ~/.config/systemd/user/riku-nginx.path
rm -f ~/.config/systemd/user/riku-nginx-reload.service

# Reload systemd
systemctl --user daemon-reload
```

---

## See Also

- [Nginx Configuration](nginx.md) - How Riku generates nginx configs
