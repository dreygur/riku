# Nginx Configuration

Riku automatically generates nginx configuration files for each application. Configurations are stored in `~/.riku/nginx/<app>.conf`.

---

## How It Works

When you deploy an app, Riku:

1. Detects the app type (web, static, etc.)
2. Reads environment variables from `~/.riku/envs/<app>/ENV`
3. Generates an nginx config using templates
4. Writes it as `<app>.conf.new`, checks it with `nginx -t`, and only then renames it to `<app>.conf`
5. Reloads nginx and waits for the reload to be confirmed (see [Caddy reloads](caddy.md#reloads) for what confirmed means for each router). A failed reload fails the deploy, and `riku doctor` reports the last failure

nginx loads every `~/.riku/nginx/*.conf` through one file, `/etc/nginx/conf.d/riku.conf`, which `sudo riku init` installs. It holds a single line, `include /home/<user>/.riku/nginx/*.conf;`. Riku never writes anything else under `/etc/nginx`. Re-running `sudo riku init` also removes the per-app symlinks older versions left in `/etc/nginx/sites-enabled/`, which would otherwise load each site twice. Until that is in order, deploy and destroy refuse to run and say what to fix: a missing include file, leftover links, or a `*.conf` in `~/.riku/nginx/` that is no app's config. `riku doctor` reports the same. On a host without nginx this check is skipped.

---

## Configuration Files

### Generated Config

Location: `~/.riku/nginx/<app>.conf`

Generated automatically based on your app type and environment variables.

### Custom Config

Replacing the generated config is not supported. An `nginx.conf`, `nginx.custom.conf`, `nginx.custom` or `.nginx.conf` in the app directory is ignored with a warning at deploy, and the generated config is used. Every riku site is loaded into the one server-wide nginx, so a file from an app repo could otherwise take over other apps' domains.

To add directives to the generated `server` block, set `RIKU_NGINX_INCLUDE_FILE` to a snippet in the app directory (see below).

---

## Environment Variables

### Basic Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_DOMAINS` | Domain name for the app | (none) |
| `RIKU_HTTPS_ONLY` | Force HTTPS redirect. On for `true`, `1`, `yes` or `on`; any other value is off | `false` |
| `RIKU_DISABLE_IPV6` | Disable IPv6 | `false` |
| `RIKU_BIND_ADDRESS` | Backend bind address. Must be an IP literal such as `127.0.0.1` or `::1`; a host name like `localhost` fails the deploy of a port mapped app | `127.0.0.1` |

### Static Files

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_NGINX_STATIC_PATHS` | URL path to directory mappings | (none) |
| `RIKU_NGINX_ALLOW_GIT_FOLDERS` | Allow access to `.git` | `false` |
| `RIKU_NGINX_CATCH_ALL` | SPA catch-all file | (none) |

**Example:**
```bash
# Serve static files
RIKU_NGINX_STATIC_PATHS=/static:public,/assets:static

# SPA routing
RIKU_NGINX_CATCH_ALL=index.html
```

### Caching

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_NGINX_CACHE_PREFIXES` | URL prefixes to cache | (none) |
| `RIKU_NGINX_CACHE_SIZE` | Cache size in GB | `1` |
| `RIKU_NGINX_CACHE_TIME` | Cache time in seconds | `3600` |
| `RIKU_NGINX_CACHE_EXPIRY` | Max cache entry age | `86400` |

**Example:**
```bash
RIKU_NGINX_CACHE_PREFIXES=/api,/static,/images
RIKU_NGINX_CACHE_SIZE=2
RIKU_NGINX_CACHE_TIME=7200
```

### Security

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_NGINX_CLOUDFLARE_ACL` | Restrict to Cloudflare IPs | `false` |
| `RIKU_NGINX_INCLUDE_FILE` | Snippet in the app directory copied into the server block (plain directives only, see below) | (none) |

---

## Generated Config Structure

### Basic Web App

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name example.com;

    # Logging
    access_log /var/log/nginx/example.com.access.log;
    error_log /var/log/nginx/example.com.error.log;

    # Client upload size
    client_max_body_size 100M;

    # Backend proxy
    location / {
        proxy_pass http://127.0.0.1:5000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### HTTPS Redirect

When `RIKU_HTTPS_ONLY=true`:

```nginx
server {
    listen 80;
    server_name example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate /etc/ssl/certs/example.com.crt;
    ssl_certificate_key /etc/ssl/private/example.com.key;

    # ... rest of config
}
```

### Static Files

When `RIKU_NGINX_STATIC_PATHS=/static:public`:

```nginx
location /static/ {
    alias /home/riku/.riku/apps/myapp/public/;
    expires 30d;
    add_header Cache-Control "public, immutable";
}
```

### SPA Catch-All

When `RIKU_NGINX_CATCH_ALL=index.html`:

```nginx
location / {
    try_files $uri $uri/ /index.html;
}
```

### Caching

When caching is enabled:

```nginx
# Cache zone definition
proxy_cache_path /home/riku/.riku/cache/myapp levels=1:2
    keys_zone=myapp_cache:10m max_size=2g inactive=60m;

# In location block
location /api/ {
    proxy_cache myapp_cache;
    proxy_cache_valid 200 2h;
    proxy_cache_valid 404 1m;
    add_header X-Cache-Status $upstream_cache_status;
}
```

### Cloudflare ACL

When `RIKU_NGINX_CLOUDFLARE_ACL=true`:

```nginx
# Allow Cloudflare IPs only
include /etc/nginx/cloudflare-acl.conf;

# Deny all other direct access
deny all;
```

---

## Manual Configuration

### Edit Generated Config

```bash
# Edit the generated config
nano ~/.riku/nginx/myapp.conf

# Test nginx config
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

Manual edits may be overwritten on redeploy.

### Add Custom Config

Create `nginx.include` in the app directory. Its content is copied into the app's `server` block, and every statement must use one of these directives:

`add_header`, `expires`, `client_max_body_size`, `client_body_timeout`, `keepalive_timeout`, `proxy_connect_timeout`, `proxy_read_timeout`, `proxy_send_timeout`, `proxy_buffering`, `proxy_buffer_size`, `proxy_buffers`, `proxy_busy_buffers_size`, `gzip`, `gzip_types`, `gzip_min_length`, `gzip_comp_level`, `gzip_vary`

```nginx
# Larger uploads and a stricter frame policy
client_max_body_size 50m;
add_header X-Frame-Options DENY always;
```

The deploy is refused, naming the file and the reason, if the file is missing or resolves outside the app directory, or if it uses any other directive, contains `{` or `}`, or has quoting nginx could read differently (see `RIKU_NGINX_INCLUDE_FILE` in the environment reference). Blocks such as `location`, and directives like `proxy_pass`, `root` or `proxy_set_header`, are therefore not possible here. Do not put extra `.conf` files in `~/.riku/nginx/`: nginx loads every `*.conf` there.

Installed filter plugins on the `nginx.include_content` chain may rewrite the snippet. Their output gets the same grammar and directive check, and output that fails it, or is not a string, fails the deploy.

Set in ENV:
```bash
riku config set myapp RIKU_NGINX_INCLUDE_FILE=nginx.include
```

---

## SSL/HTTPS

### Automatic SSL (ACME)

Riku supports ACME (Let's Encrypt) for automatic SSL certificates.

1. **Set domain:**
   ```bash
   riku config set myapp RIKU_DOMAINS=example.com
   ```

2. **Enable HTTPS:**
   ```bash
   riku config set myapp RIKU_HTTPS_ONLY=true
   ```

3. **Obtain certificate:**
   ```bash
   # Using acme.sh
   acme.sh --issue -d example.com --webroot /home/riku/.riku/acme/www
   ```

4. **Install certificate:**
   ```bash
   acme.sh --install-cert -d example.com \
     --cert-file ~/.riku/acme/example.com/cert.pem \
     --key-file ~/.riku/acme/example.com/key.pem \
     --fullchain-file ~/.riku/acme/example.com/fullchain.pem
   ```

5. **Restart app:**
   ```bash
   riku restart myapp
   ```

### Manual SSL

Place certificates in a standard location and configure nginx to use them.

---

## Troubleshooting

### Test Nginx Configuration

```bash
sudo nginx -t
```

### View Generated Config

```bash
cat ~/.riku/nginx/myapp.conf
```

### Check Nginx Logs

```bash
# Access log
sudo tail -f /var/log/nginx/example.com.access.log

# Error log
sudo tail -f /var/log/nginx/example.com.error.log
```

### Common Issues

#### 502 Bad Gateway

The backend isn't running or isn't listening on the expected port.

```bash
# Check if app is running
riku ps myapp

# Check backend port
riku config live myapp | grep PORT

# Restart app
riku restart myapp
```

#### 404 Not Found

Static files not found or wrong path.

```bash
# Check static paths
ls -la ~/.riku/apps/myapp/public/

# Verify RIKU_NGINX_STATIC_PATHS
riku config get myapp RIKU_NGINX_STATIC_PATHS
```

#### SSL Certificate Errors

Certificate not found or expired.

```bash
# Check certificate
ls -la ~/.riku/acme/example.com/

# Renew certificate
acme.sh --renew -d example.com
```

#### Config Not Applied

Nginx not reloaded after config change.

```bash
# Reload nginx
sudo systemctl reload nginx

# Or restart nginx
sudo systemctl restart nginx
```

---

## Advanced Configuration

These examples are for an nginx config you manage yourself. An app's `RIKU_NGINX_INCLUDE_FILE` snippet accepts only the directives listed under Add Custom Config: the custom headers below work there, the gzip example works without `gzip_proxied`, and rate limiting (`limit_req_zone` lives at http level) and the WebSocket `location` block cannot be set per app.

### Rate Limiting

Not possible through the app snippet (see above):

```nginx
# Define zone (in http block)
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

# Apply in location
location /api/ {
    limit_req zone=api burst=20 nodelay;
}
```

### Gzip Compression

```nginx
gzip on;
gzip_vary on;
gzip_proxied any;
gzip_comp_level 6;
gzip_types text/plain text/css text/xml application/json application/javascript application/xml;
```

### Custom Headers

```nginx
add_header X-Frame-Options "SAMEORIGIN" always;
add_header X-Content-Type-Options "nosniff" always;
add_header X-XSS-Protection "1; mode=block" always;
```

### WebSocket Support

```nginx
location /ws/ {
    proxy_pass http://127.0.0.1:5000;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

---

## See Also

- [Environment Variables](env.md) - All nginx-related ENV vars
- [CLI Reference](cli.md) - Config management commands
