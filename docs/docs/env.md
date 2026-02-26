# Environment Variables

Riku uses environment variables to configure application behavior, nginx, and the supervisor. Variables are stored in `~/.riku/envs/<app>/ENV`.

## Setting Environment Variables

### Using CLI

```bash
riku config:set myapp KEY=value ANOTHER_KEY=value2
```

### Using ENV File

Edit `~/.riku/envs/<app>/ENV` directly:

```bash
DATABASE_URL=postgres://localhost/mydb
SECRET_KEY=supersecret
DEBUG=false
```

### Variable Interpolation

Environment variables support shell-style interpolation:

```bash
BASE_URL=https://$NGINX_SERVER_NAME
API_URL=$BASE_URL/api
```

---

## Runtime Settings

### Auto Restart

```bash
RIKU_AUTO_RESTART=true
```

- **Default:** `true`
- **Description:** Automatically restart workers on deploy
- **Values:** `true` | `false`

When set to `false`, you must manually restart with `riku restart <app>`.

---

## Network Settings

### Bind Address

```bash
BIND_ADDRESS=127.0.0.1
```

- **Default:** `0.0.0.0`
- **Description:** IP address for workers to bind to
- **Values:** Any valid IP address

### Disable IPv6

```bash
DISABLE_IPV6=true
```

- **Default:** `false`
- **Description:** Disable IPv6 in nginx configuration
- **Values:** `true` | `false`

### Server Name

```bash
NGINX_SERVER_NAME=example.com
```

- **Default:** (none)
- **Description:** Domain name for your application
- **Values:** Any valid domain name

**Example:**
```bash
riku config:set myapp NGINX_SERVER_NAME=myapp.example.com
```

### Force HTTPS

```bash
NGINX_HTTPS_ONLY=true
```

- **Default:** `false`
- **Description:** Redirect all HTTP traffic to HTTPS
- **Values:** `true` | `false`

---

## Worker Management

### Worker Timeout

```bash
RIKU_WORKER_TIMEOUT=3600
```

- **Default:** `3600` (1 hour)
- **Description:** Kill unresponsive workers after N seconds
- **Values:** Positive integer (seconds)

### Grace Period

```bash
RIKU_WORKER_GRACE_PERIOD=60
```

- **Default:** `60` seconds
- **Description:** Graceful shutdown period before force kill
- **Values:** Positive integer (seconds)

### Max Restarts

```bash
RIKU_MAX_RESTARTS=10
```

- **Default:** `10`
- **Description:** Maximum restart attempts before marking as failed
- **Values:** Positive integer

### Worker Processes

```bash
RIKU_WORKER_PROCESSES=web=2,worker=1
```

- **Default:** (uses SCALING file)
- **Description:** Alternative to SCALING file for process counts
- **Values:** Comma-separated `type=count` pairs

---

## Nginx Configuration

### Static Paths

```bash
NGINX_STATIC_PATHS=/static:public,/assets:static
```

- **Default:** (none)
- **Description:** Serve static files directly via nginx (bypass backend)
- **Format:** `url_path:directory[,url_path:directory...]`

**Example:**
```bash
# Serve /static/* from ./public directory
NGINX_STATIC_PATHS=/static:public

# Multiple paths
NGINX_STATIC_PATHS=/static:public,/assets:static,/images:img
```

### Include Custom Config

```bash
NGINX_INCLUDE_FILE=custom.conf
```

- **Default:** (none)
- **Description:** Include custom nginx configuration file
- **Values:** Path to nginx config snippet

The file should be placed in `~/.riku/nginx/` or your app directory.

### Allow Git Folders

```bash
NGINX_ALLOW_GIT_FOLDERS=false
```

- **Default:** `false`
- **Description:** Allow access to `.git` folders (security risk)
- **Values:** `true` | `false`

**Warning:** Setting this to `true` exposes your git history publicly.

### Catch-All for SPA

```bash
NGINX_CATCH_ALL=index.html
```

- **Default:** (none)
- **Description:** Serve this file for all unmatched routes (SPA routing)
- **Values:** Filename

Useful for React, Vue, Angular single-page applications.

### Cloudflare ACL

```bash
NGINX_CLOUDFLARE_ACL=true
```

- **Default:** `false`
- **Description:** Restrict access to Cloudflare IP ranges only
- **Values:** `true` | `false`

Combine with Cloudflare's "Always Use HTTPS" page rule.

---

## Nginx Caching

### Cache Prefixes

```bash
NGINX_CACHE_PREFIXES=/api,/images,/static
```

- **Default:** (none)
- **Description:** URL prefixes to cache
- **Format:** Comma-separated URL path prefixes

### Cache Size

```bash
NGINX_CACHE_SIZE=1
```

- **Default:** `1`
- **Description:** Cache size in gigabytes
- **Values:** Positive integer (GB)

### Cache Time

```bash
NGINX_CACHE_TIME=3600
```

- **Default:** `3600` (1 hour)
- **Description:** How long to cache responses (seconds)
- **Values:** Positive integer (seconds)

### Cache Expiry

```bash
NGINX_CACHE_EXPIRY=86400
```

- **Default:** `86400` (24 hours)
- **Description:** Maximum cache entry lifetime (seconds)
- **Values:** Positive integer (seconds)

---

## Node.js Settings

### Node Version

```bash
NODE_VERSION=18.17.0
```

- **Default:** `18.17.0` (LTS)
- **Description:** Node.js version to install via nodeenv
- **Values:** Any valid Node.js version

### Node Package Manager

```bash
NODE_PACKAGE_MANAGER=yarn
```

- **Default:** `npm`
- **Description:** Package manager to use for installing dependencies
- **Values:** `npm` | `yarn` | `pnpm`

---

## Python Settings

### Python Version

```bash
PYTHON_VERSION=3.11.4
```

- **Default:** `3.11.4`
- **Description:** Python version to install via pyenv
- **Values:** Any valid Python 3.x version

### Package Manager

```bash
PYTHON_PACKAGE_MANAGER=poetry
```

- **Default:** `pip`
- **Description:** Package manager for Python dependencies
- **Values:** `pip` | `poetry` | `uv`

---

## SSL/ACME Settings

### ACME Root CA

```bash
ACME_ROOT_CA=letsencrypt.org
```

- **Default:** `letsencrypt.org`
- **Description:** Certificate authority for SSL certificates
- **Values:** ACME-compatible CA domain

### ACME Email

```bash
ACME_EMAIL=admin@example.com
```

- **Default:** (none)
- **Description:** Email for ACME certificate notifications
- **Values:** Valid email address

---

## Logging Settings

### Log Level

```bash
LOG_LEVEL=info
```

- **Default:** `info`
- **Description:** Logging verbosity
- **Values:** `debug` | `info` | `warn` | `error`

### Log Rotation Size

```bash
LOG_ROTATION_SIZE=10485760
```

- **Default:** `10485760` (10MB)
- **Description:** Rotate logs when they exceed this size (bytes)
- **Values:** Positive integer (bytes)

---

## Complete Example

Example `~/.riku/envs/myapp/ENV`:

```bash
# Application settings
DATABASE_URL=postgres://localhost/myapp_prod
SECRET_KEY=supersecret123
DEBUG=false

# Domain and HTTPS
NGINX_SERVER_NAME=myapp.example.com
NGINX_HTTPS_ONLY=true

# Worker settings
RIKU_WORKER_TIMEOUT=3600
RIKU_WORKER_GRACE_PERIOD=60

# Static files
NGINX_STATIC_PATHS=/static:public,/assets:static

# SPA routing
NGINX_CATCH_ALL=index.html

# Caching
NGINX_CACHE_PREFIXES=/api,/static
NGINX_CACHE_TIME=3600

# SSL
ACME_EMAIL=admin@example.com
```

---

## Viewing Configuration

### Show All Variables

```bash
riku config myapp
```

### Get Single Variable

```bash
riku config get myapp DATABASE_URL
```

### Live Configuration

```bash
riku config live myapp
```

Shows the actual configuration being used by the supervisor.

---

## Best Practices

1. **Never commit secrets** - Use `riku config:set` for sensitive values
2. **Use meaningful names** - Prefix custom vars with your app name
3. **Document variables** - Keep a list of required env vars in your README
4. **Test locally** - Use `.env` files for local development
5. **Rotate secrets** - Change passwords and keys regularly

---

## Troubleshooting

### Variable Not Applied

1. Check spelling: `riku config get myapp KEY_NAME`
2. Restart the app: `riku restart myapp`
3. Check live config: `riku config live myapp`

### Interpolation Not Working

Ensure you're using `$VAR` syntax, not `${VAR}`:

```bash
# Correct
API_URL=$BASE_URL/api

# May not work
API_URL=${BASE_URL}/api
```

### Nginx Config Not Updated

After changing nginx-related variables:

```bash
riku restart myapp
sudo nginx -t && sudo systemctl reload nginx
```
