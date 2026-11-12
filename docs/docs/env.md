# Environment Variables

Riku uses environment variables to configure application behavior, nginx, and the supervisor. Variables are stored in `~/.riku/envs/<app>/ENV`.

## Setting Environment Variables

### Using CLI

```bash
riku config set myapp KEY=value ANOTHER_KEY=value2
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
BASE_URL=https://$RIKU_DOMAINS
API_URL=$BASE_URL/api
```

---

Lifecycle hooks get the detected runtime as `RIKU_DETECTED_RUNTIME`, so it
never collides with the `RIKU_RUNTIME` override.

## Runtime Settings

### Worker replacement on deploy

Every successful deploy runs the new build: web facing kinds are replaced
one copy at a time, other kinds are stopped and started from the new
configs, and a kind that left the Procfile stops.

---

## Network Settings

### Port

```bash
riku config set myapp PORT=3000
```

- **Default:** unset. Each deploy picks a free port, passes it to the `web` process as `PORT` and routes to it.
- **Description:** pins the port instead, for a Procfile app that has to listen on a fixed one. Container apps are routed to the host port riku picks for each copy, not to `PORT` (see [Runtimes → Container](runtimes.md#container)).
- **Values:** 1 to 65535. A pinned port runs one `web` process: a deploy with `web` scaled above 1 is refused. So is a port another riku app uses, or one some other process on the host listens on, since the domain would otherwise be routed to that process. Both checks run before the running app is touched. Two copies cannot share the port, so a deploy stops the old `web` process before the new one starts, and the site is down in between.

### Bind Address

```bash
RIKU_BIND_ADDRESS=127.0.0.1
```

- **Default:** `127.0.0.1`
- **Description:** IP address for workers to bind to
- **Values:** Any valid IP address

### Server Name

```bash
RIKU_DOMAINS=example.com
```

- **Default:** (none)
- **Description:** Domain name for your application
- **Values:** Any valid domain name

**Example:**
```bash
riku settings set myapp routing.domains=myapp.example.com
```

### Force HTTPS

```bash
RIKU_HTTPS_ONLY=true
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
RIKU_NGINX_STATIC_PATHS=/static:public,/assets:static
```

- **Default:** (none)
- **Description:** Serve static files directly via nginx (bypass backend)
- **Format:** `url_path:directory[,url_path:directory...]`

**Example:**
```bash
# Serve /static/* from ./public directory
RIKU_NGINX_STATIC_PATHS=/static:public

# Multiple paths
RIKU_NGINX_STATIC_PATHS=/static:public,/assets:static,/images:img
```

### Refused Keys

A deploy fails, naming the key, when the app's ENV sets `RIKU_NGINX_SOCKET`: the router helper picks socket paths itself.

### Allow Git Folders

```bash
RIKU_NGINX_ALLOW_GIT_FOLDERS=false
```

- **Default:** `false`
- **Description:** Allow access to `.git` folders (security risk)
- **Values:** `true` | `false`

Setting this to `true` exposes your git history publicly.

---

## Node.js Settings

### Node Version

```bash
RIKU_NODE_VERSION=18.17.0
```

- **Default:** (none, uses whatever `node` is on `$PATH`)
- **Accepts:** `18`, `18.17`, `18.17.0`, `18.x`, `18.17.x`
- **Description:** the node riku builds with must match. `18` and `18.x`
  match any 18 release, `18.17` any 18.17 patch, `18.17.0` only that one.
  Riku uses the server's `node` and does not install another version, so on
  a mismatch the build fails and names both versions. Set it in riku.toml
  as `[runtime] version`.
- **Values:** Any valid Node.js version

### Node Package Manager

Riku builds Node apps with [nub](https://nubjs.com), which auto-detects the
lockfile in your repo (`npm`, `pnpm`, or `bun`), there is no package-manager
setting to configure. Commit a lockfile to pin your dependency versions. See
[Runtimes → Node.js](runtimes.md) for details.

---

## Python Settings

### Python Version

```bash
RIKU_PYTHON_VERSION=3.11.4
```

- **Default:** `"3"` (system Python 3)
- **Description:** Python version to use (passed to `python3 -m venv`)
- **Values:** Any valid Python 3.x version

### Package Manager

There is no setting: Python apps build with uv when it is installed on the
server, else with pip. See [Runtimes](runtimes.md#package-manager).

---

## Container / Compose Settings

See [Runtimes → Container](runtimes.md#container) for how detection, builds,
and auto-updates work end to end.

### Compose File

```bash
COMPOSE_FILE=compose.prod.yml
```

- **Default:** (none; the first of `compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml` present)
- **Description:** The compose file a compose app deploys with. Riku reads it itself; no `compose` tool runs
- **Values:** One path relative to the app root, such as `compose.prod.yml` or `deploy/compose.yml`

Setting it selects the container runtime even when the repo also has a `package.json` or other runtime marker. The path must stay inside the app: absolute paths, `..`, and symlinks pointing outside are rejected, as is a docker-style list (`a.yml:b.yml`). An invalid value fails the deploy instead of falling back to another runtime.

### GHCR Username

```bash
RIKU_GHCR_USERNAME=your-username
```

- **Default:** (none)
- **Description:** Username for `docker login ghcr.io`, run before pulling a compose app's images
- **Values:** Any GitHub username

Required together with `RIKU_GHCR_TOKEN` only for private GHCR images; omit both for public images. Setting only one of the two fails the deploy.

### GHCR Token

```bash
RIKU_GHCR_TOKEN=ghp_xxxxxxxxxxxx
```

- **Default:** (none)
- **Description:** Token (classic PAT with `read:packages`, or `GITHUB_TOKEN` in Actions) for `docker login ghcr.io`
- **Values:** Any valid GHCR-scoped token

**Example:**
```bash
riku settings set myapp secrets.ghcr_username=octocat secrets.ghcr_token=ghp_xxx
```

### Watched Services

```bash
RIKU_WATCH_SERVICES=web,worker
```

- **Default:** (none, auto-update disabled)
- **Description:** Compose services to re-pull periodically and replace, one copy at a time, when their registry tag names a new image, without a `git push`. Services built from source are skipped
- **Values:** Comma-separated compose service names

Checked every 60 seconds. Only meaningful for compose apps (see
[Runtimes → Container](runtimes.md#container)); ignored otherwise.

---

## SSL/ACME Settings

With nginx, riku issues certificates through acme.sh from Let's Encrypt,
without an account email; there is no setting for either. Caddy issues its
own certificates.

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
RIKU_DOMAINS=myapp.example.com
RIKU_HTTPS_ONLY=true

# Worker settings
RIKU_WORKER_TIMEOUT=3600
RIKU_WORKER_GRACE_PERIOD=60

# Static files
RIKU_NGINX_STATIC_PATHS=/static:public,/assets:static
```

---

## Viewing Configuration

### Show All Variables

```bash
riku config show myapp
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

1. **Never commit secrets** - Use `riku config set` for sensitive values
2. **Use meaningful names** - Prefix custom vars with your app name
3. **Document variables** - Keep a list of required env vars in your README
4. **Test locally** - Use `.env` files for local development
5. **Rotate secrets** - Change passwords and keys regularly

---

## Troubleshooting

### Variable Not Applied

1. Check spelling: `riku config get myapp KEY_NAME`
2. Deploy the app so its workers get the new env: `riku deploy myapp`
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

The router config is rebuilt on deploy. After changing routing variables (`RIKU_DOMAINS`, `RIKU_HTTPS_ONLY`, static paths), deploy again. If the deploy fails, the message comes from the router helper.
