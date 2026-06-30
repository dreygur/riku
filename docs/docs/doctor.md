# Doctor

`riku doctor` diagnoses a Riku installation and reports what is healthy and what
needs attention. Run it after `riku init`, when something misbehaves, or as a
periodic health check.

## Run it

```bash
riku doctor
```

Some checks (system directories, nginx, systemd) inspect privileged state — run
with `sudo` for the most complete report:

```bash
sudo riku doctor
```

## What it checks

`doctor` inspects the major moving parts of an installation:

| Area | Checks |
|------|--------|
| **Dependencies** | Required external tools are present on the host |
| **Directories** | The `~/.riku/` layout exists with correct structure |
| **Systemd** | Riku services are installed and their state |
| **Nginx** | Nginx is available and configs are valid |
| **Disk** | Sufficient free disk space |
| **SSH** | The deploy user's SSH access is set up |

Each check reports a status so you can fix issues before they affect deploys.

## Typical workflow

```bash
# Fresh server
riku init
sudo riku doctor      # confirm everything came up correctly

# Later, if deploys fail
sudo riku doctor      # find the broken piece
```

!!! tip "Plugins have their own doctor"
    To validate installed plugin bundles (API compatibility and integrity), use
    [`riku plugins doctor`](marketplace.md) instead — it checks the plugin
    ecosystem rather than the core installation.
