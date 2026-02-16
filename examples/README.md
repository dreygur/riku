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
git remote add riku deploy@your-server:my-node-app
git push riku master
```

## Examples Overview

| Example | Runtime | Description |
|---------|---------|-------------|
| `nodejs-wisp/` | Node.js | Simple HTTP server |
| `golang/` | Go | Go web application |
| `rust/` | Rust | Rust web server |
| `python-postgres/` | Python | Django app with PostgreSQL |
| `client-plugins/` | Plugin | Example Riku plugin |

## Configuration Files

### ENV File

The `ENV` file contains environment variables for your app:

```bash
# Basic configuration
NGINX_SERVER_NAME=example.com
NGINX_HTTPS_ONLY=true

# Worker settings
PIKU_AUTO_RESTART=true
BIND_ADDRESS=127.0.0.1

# Node.js specific
NODE_VERSION=18.17.0
NODE_PACKAGE_MANAGER=npm

# Nginx caching
NGINX_CACHE_PREFIXES=/api/cache
NGINX_CACHE_TIME=3600
```

See `../docs/ENV.md` for the complete list of environment variables.

### SCALING File

Control how many worker processes run:

```bash
# Scale web workers
web=2

# Scale background workers
worker=1
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
# Run the test script
../tests/deploy/quick-test.sh test-node node

# Or run manually
cd nodejs-wisp
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
- See `../docs/FAQ.md`
- Check `../docs/INSTALL.md`
- Review `../docs/ENV.md`
