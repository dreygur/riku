# Riku Systemd Service Files

This directory contains systemd service files for running Riku as a system service.

## Files

- `riku.service` - Main Riku supervisor daemon service
- `riku-nginx.path` - Watches for nginx configuration changes
- `riku-nginx-reload.service` - Reloads nginx when configs change

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
- `WorkingDirectory` - Path to deploy user home
- `ExecStart` - Path to riku binary

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

# Check status
systemctl status riku-nginx.path
```

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

### High Resource Usage

The service includes resource limits:
- Memory: 512MB max
- CPU: 50% quota

Adjust in `/etc/systemd/system/riku.service` if needed:
```ini
MemoryMax=1G
CPUQuota=100%
```

## Security

The service includes security hardening:
- Runs as `deploy` user (not root)
- `NoNewPrivileges=true` - Prevents privilege escalation
- `ProtectSystem=strict` - Read-only system directories
- `ProtectHome=read-only` - Read-only home directories
- `PrivateTmp=true` - Isolated /tmp directory
- `ReadWritePaths` - Only allows writing to riku directories

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
