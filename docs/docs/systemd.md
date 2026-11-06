# Systemd Integration

`sudo riku init` installs and starts two system services. No manual service file copying is required.

| Unit | Runs as | Does |
|------|---------|------|
| `riku.service` | the deploy user | the process supervisor, `riku supervisor` |
| `riku-router-helper.service` | root | writes router config and reloads nginx or Caddy |

Both run the root owned `/usr/local/bin/riku`. Both start on boot. Without root, init writes a user unit for the supervisor only (`~/.config/systemd/user/riku.service`), and there is no router helper.

---

## Supervisor Service

The generated `/etc/systemd/system/riku.service` is hardened and limited. The parts that matter:

```ini
[Service]
Type=simple
User=riku
ExecStart=/usr/local/bin/riku supervisor
Restart=always
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
MemoryMax=512M
CPUQuota=50%
```

`ReadWritePaths` lists `~/.riku`, `/run/riku-sockets`, `/var/lib/riku/acme` and `/var/lib/riku/static`. Everything else on disk is read only to the supervisor and the apps it starts.

```bash
sudo systemctl status riku
sudo systemctl restart riku
sudo journalctl -u riku -f
```

---

## Router Helper Service

`riku-router-helper.service` runs `/usr/local/bin/riku router-helper` as root. It binds `/run/riku-router.sock` itself, which only the deploy user can use, and restarts after a crash. It runs with `ProtectHome=yes` and `NoNewPrivileges=yes`.

```bash
sudo systemctl status riku-router-helper
sudo journalctl -u riku-router-helper -n 50
sudo systemctl restart riku-router-helper
```

A deploy fails with the helper's message when it cannot reach the socket. See [Nginx](nginx.md#the-router-helper) for what the helper writes and where.

---

## Upgrading

A new `riku` binary needs root to install. Install it, then run init again so the units and the helper are rewritten, and restart the services:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sudo sh
sudo riku init
sudo systemctl restart riku riku-router-helper
```

Then, as the deploy user, run `riku doctor --fix` to redeploy apps that were routed by an older version.

---

## Uninstall

```bash
sudo systemctl disable --now riku riku-router-helper
sudo rm /etc/systemd/system/riku.service /etc/systemd/system/riku-router-helper.service
sudo systemctl daemon-reload
```

---

## See Also

- [Nginx Configuration](nginx.md) - How Riku generates nginx configs
- [Caddy Router](caddy.md) - The Caddy router
