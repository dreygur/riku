# Nginx Configuration

Riku automatically generates nginx configuration files for each application. Configurations are stored in `~/.riku/nginx/<app>.conf`.

---

## How It Works

When you deploy an app, Riku:

1. Detects the app type (web, static, etc.)
2. Reads environment variables from `~/.riku/envs/<app>/ENV`
3. Generates an nginx config using templates
4. Symlinks the config to nginx's `sites-enabled/`
5. Reloads nginx

---

## Configuration Files

### Generated Config

Location: `~/.riku/nginx/<app>.conf`

Generated automatically based on your app type and environment variables.

### Custom Config

You can provide custom nginx configuration:

1. **App directory:** `nginx.conf`, `nginx.custom.conf`, or `.nginx.conf`
2. **Include file:** Set `NGINX_INCLUDE_FILE=custom.conf`

Custom configs are included at the end of the generated config.

---

## Environment Variables

### Basic Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `NGINX_SERVER_NAME` | Domain name for the app | (none) |
| `NGINX_HTTPS_ONLY` | Force HTTPS redirect | `false` |
| `DISABLE_IPV6` | Disable IPv6 | `false` |
| `BIND_ADDRESS` | Backend bind address | `127.0.0.1` |

### Static Files

| Variable | Description | Default |
|----------|-------------|---------|
| `NGINX_STATIC_PATHS` | URL path to directory mappings | (none) |
| `NGINX_ALLOW_GIT_FOLDERS` | Allow access to `.git` | `false` |
| `NGINX_CATCH_ALL` | SPA catch-all file | (none) |

**Example:**
```bash
# Serve static files
NGINX_STATIC_PATHS=/static:public,/assets:static

# SPA routing
NGINX_CATCH_ALL=index.html
```

### Caching

| Variable | Description | Default |
|----------|-------------|---------|
| `NGINX_CACHE_PREFIXES` | URL prefixes to cache | (none) |
| `NGINX_CACHE_SIZE` | Cache size in GB | `1` |
| `NGINX_CACHE_TIME` | Cache time in seconds | `3600` |
| `NGINX_CACHE_EXPIRY` | Max cache entry age | `86400` |

**Example:**
```bash
NGINX_CACHE_PREFIXES=/api,/static,/images
NGINX_CACHE_SIZE=2
NGINX_CACHE_TIME=7200
```

### Security

| Variable | Description | Default |
|----------|-------------|---------|
| `NGINX_CLOUDFLARE_ACL` | Restrict to Cloudflare IPs | `false` |
| `NGINX_INCLUDE_FILE` | Custom nginx config snippet | (none) |

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

When `NGINX_HTTPS_ONLY=true`:

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

When `NGINX_STATIC_PATHS=/static:public`:

```nginx
location /static/ {
    alias /home/deploy/.riku/apps/myapp/public/;
    expires 30d;
    add_header Cache-Control "public, immutable";
}
```

### SPA Catch-All

When `NGINX_CATCH_ALL=index.html`:

```nginx
location / {
    try_files $uri $uri/ /index.html;
}
```

### Caching

When caching is enabled:

```nginx
# Cache zone definition
proxy_cache_path /home/deploy/.riku/cache/myapp levels=1:2
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

When `NGINX_CLOUDFLARE_ACL=true`:

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

**Note:** Manual edits may be overwritten on redeploy.

### Add Custom Config

Create `~/.riku/nginx/myapp.custom.conf`:

```nginx
# Custom rate limiting
location /api/ {
    limit_req zone=api burst=20 nodelay;
}
```

Set in ENV:
```bash
riku config set myapp NGINX_INCLUDE_FILE=myapp.custom.conf
```

---

## SSL/HTTPS

### Automatic SSL (ACME)

Riku supports ACME (Let's Encrypt) for automatic SSL certificates.

1. **Set domain:**
   ```bash
   riku config set myapp NGINX_SERVER_NAME=example.com
   ```

2. **Enable HTTPS:**
   ```bash
   riku config set myapp NGINX_HTTPS_ONLY=true
   ```

3. **Obtain certificate:**
   ```bash
   # Using acme.sh
   acme.sh --issue -d example.com --webroot /home/deploy/.riku/acme/www
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

# Verify NGINX_STATIC_PATHS
riku config get myapp NGINX_STATIC_PATHS
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

### Rate Limiting

Add to custom config:

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
