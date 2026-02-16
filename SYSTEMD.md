# Riku - Systemd Service Files

## Optional: Nginx Auto-Reload

These systemd files automatically reload nginx when Riku updates nginx configurations.

### Installation (Optional)

```bash
# Copy service files
sudo cp riku-nginx.service riku-nginx.path /etc/systemd/system/

# Enable path watcher
sudo systemctl enable riku-nginx.path
sudo systemctl start riku-nginx.path
```

This is optional - Riku can manually reload nginx after config changes.
