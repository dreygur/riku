# Riku Example Applications

This directory contains example applications demonstrating how to deploy different runtimes to Riku.

## Quick Start

1. Choose an example app from this directory
2. Copy it to a new directory
3. Initialize git and deploy

```bash
# Copy an example
cp -r nodejs-wisp ~/my-node-app
cd ~/my-node-app

# Initialize git
git init
git add .
git commit -m "Initial commit"

# Deploy (replace with your server)
git remote add riku riku@edge.example:my-node-app
git push riku main
```

## Examples Overview

| Example | Runtime | Description |
|---------|---------|-------------|
| `nodejs/` | Node.js | Express HTTP server |
| `nodejs-wisp/` | Node.js | Simple HTTP server |
| `golang/` | Go | Go web application |
| `rust/` | Rust | Rust web server |
| `python-postgres/` | Python | Django app with PostgreSQL |
| `compose-ghcr/` | Container | Compose app pulling a GHCR image, with auto-update |
| `client-plugins/` | Plugin | Example client-side plugins |
| `server-hooks/` | Plugin | Example server-side lifecycle hook plugins |
| `plugins/` | Plugin | Addon and event bundles: postgres, redis, sqlite-volume, webhook-notify |

## Configuration Files

### App Environment

riku reads an app's environment only from the server
(`~/.riku/envs/<app>/ENV`), never from a file in the repo. Set your app's own
variables with `riku config set`:

```bash
riku config set myapp DATABASE_URL=postgres://localhost/myapp SECRET_KEY=change-me
```

See `../docs/docs/env.md` for the variables riku sets for your app.

### riku.toml

riku's own settings (domains, HTTPS, static files, workers, the runtime and
its version) are not read from `ENV`. Commit them in a `riku.toml` next to
your code, or set them with `riku settings set`:

```toml
[routing]
domains = ["example.com"]
https_only = true

[runtime]
name = "node"
version = "22"
```

See `riku.toml` in this directory and `../docs/docs/riku-toml.md` for every key.

### Scaling

Each process kind runs one copy until you scale it. A `SCALING` file in the
repo is not read; scale on the server, which redeploys the app:

```bash
riku ps myapp --scale web=2 worker=1
```

### Procfile

Define your processes:

```bash
# Web process (required for HTTP apps)
web: node server.js

# Background workers
worker: python worker.py

# Cron jobs
cron: 0 2 * * * /path/to/script.sh
```

## Testing Examples Locally

You can test examples before deploying:

```bash
cd nodejs
node server.js
```

## Deployment Checklist

Before deploying, ensure:

- [ ] Riku is installed on your server
- [ ] SSH key is configured
- [ ] Domain DNS points to your server
- [ ] Nginx is running
- [ ] Required runtimes are installed (Node, Python, etc.)

## Troubleshooting

**App won't start:**
```bash
# Check logs
riku logs myapp

# Restart app
riku restart myapp
```

**Build fails:**
```bash
# Check requirements
riku run myapp node --version
riku run myapp python3 --version
```

**Need help?**
- See `../docs/docs/faq.md`
- Check `../docs/docs/installation.md`
- Review `../docs/docs/env.md`
