# Systemd Integration

Riku includes optional systemd service files for running the supervisor daemon as a system service.

---

## Quick Start

### Install and Enable Service

```bash
# As root, from the riku repository
sudo cp contrib/systemd/*.service /etc/systemd/system/
sudo cp contrib/systemd/*.path /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable riku
sudo systemctl start riku

# Check status
sudo systemctl status riku
```

---

## Service Files

The following systemd units are available in `contrib/systemd/`:

| File | Purpose |
|------|---------|
| `riku.service` | Main supervisor daemon service |
| `riku-nginx.path` | Watches for nginx config changes |
| `riku-nginx-reload.service` | Reloads nginx automatically |

---

## Installation

### 1. Copy Service Files

```bash
# As root
sudo cp contrib/systemd/riku.service /etc/systemd/system/
sudo cp contrib/systemd/riku-nginx.path /etc/systemd/system/
sudo cp contrib/systemd/riku-nginx-reload.service /etc/systemd/system/
```

### 2. Update Paths (if needed)

Edit `/etc/systemd/system/riku.service` if your installation differs:

```ini
[Service]
WorkingDirectory=/home/deploy
ExecStart=/home/deploy/.local/bin/riku supervisor
```

### 3. Enable and Start

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable riku service (starts on boot)
sudo systemctl enable riku

# Start riku service
sudo systemctl start riku

# Check status
sudo systemctl status riku
```

### 4. Enable Nginx Auto-Reload (Optional)

```bash
# Enable path watcher
sudo systemctl enable riku-nginx.path

# Start watcher
sudo systemctl start riku-nginx.path
```

---

## Usage

### Check Status

```bash
# Check riku service status
sudo systemctl status riku

# View logs
sudo journalctl -u riku -f

# Check if supervisor is running
sudo systemctl is-active riku
```

### Restart Service

```bash
sudo systemctl restart riku
```

### Stop Service

```bash
sudo systemctl stop riku
```

### View Logs

```bash
# Recent logs
sudo journalctl -u riku -n 50

# Follow logs in real-time
sudo journalctl -u riku -f

# Logs from today
sudo journalctl -u riku --since today
```

---

## Configuration

### Resource Limits

The default service includes resource limits:

```ini
MemoryMax=512M
CPUQuota=50%
```

Adjust in `/etc/systemd/system/riku.service` if needed.

### Security Hardening

The service includes security features:

```ini
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=/home/deploy/.riku
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check for errors
sudo systemctl status riku
sudo journalctl -u riku -n 100

# Verify binary exists
ls -la /home/deploy/.local/bin/riku

# Test binary manually
sudo -u deploy /home/deploy/.local/bin/riku --help
```

### Nginx Not Reloading

```bash
# Check path watcher status
systemctl status riku-nginx.path

# Check nginx reload service
sudo systemctl status riku-nginx-reload.service

# Test nginx configuration
sudo nginx -t
```

---

## Uninstall

```bash
# Stop and disable services
sudo systemctl stop riku
sudo systemctl disable riku
sudo systemctl stop riku-nginx.path
sudo systemctl disable riku-nginx.path

# Remove service files
sudo rm /etc/systemd/system/riku.service
sudo rm /etc/systemd/system/riku-nginx.path
sudo rm /etc/systemd/system/riku-nginx-reload.service

# Reload systemd
sudo systemctl daemon-reload
```

---

## See Also

- [Systemd Files](https://github.com/dreygur/riku/tree/main/contrib/systemd) - Source files on GitHub
- [Nginx Configuration](nginx.md) - How Riku generates nginx configs
