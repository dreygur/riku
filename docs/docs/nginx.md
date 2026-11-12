# Nginx Configuration

Riku generates one nginx site per application. The deploy user never writes nginx config and never runs nginx: a small root service, the router helper, does both.

---

## How It Works

When you deploy an app, Riku:

1. Detects the app type (web, static, etc.)
2. Reads environment variables from `~/.riku/envs/<app>/ENV`
3. Builds a route: the domains, the backends, the TLS material, the static directories and the HTTP mode
4. Sends the route to the router helper over `/run/riku-router.sock`
5. The helper checks the route again, renders `/etc/riku/nginx/<app>.conf`, runs `nginx -t` and reloads nginx. A failed check or reload fails the deploy, and `riku doctor` reports the last failure

### The router helper

The helper is the root owned binary `/usr/local/bin/riku` running as `riku-router-helper.service`. `sudo riku init` installs and starts it. It listens on `/run/riku-router.sock`, which only the deploy user can use.

| Path | Holds |
|------|-------|
| `/etc/riku/nginx` | one site file per app, written by the helper |
| `/etc/riku/caddy` | the same for Caddy |
| `/etc/riku/certs` | the certificate and key of each app |
| `/var/lib/riku/static` | published static files (see Static Files) |
| `/var/lib/riku/acme` | the ACME challenge webroot |

nginx loads the site files through one file, `/etc/nginx/conf.d/riku.conf`, which holds a single line, `include /etc/riku/nginx/*.conf;`. Riku writes nothing else under `/etc/nginx`. Sites listen on `[::]` as well as `0.0.0.0` when the host has IPv6, and WebSocket upgrades are proxied by default.

The helper restarts on its own after a crash. Check it with `systemctl status riku-router-helper` and `journalctl -u riku-router-helper`.

---

## Configuration Files

### Generated Config

Location: `/etc/riku/nginx/<app>.conf`, root owned. Generated from the route on every deploy. Do not edit it.

### Custom Config

Not supported. An `nginx.conf`, `nginx.custom.conf`, `nginx.custom` or `.nginx.conf` in the app directory is ignored with a warning at deploy, and the generated config is used. There is also no per app snippet file. Every riku site is loaded into the one server-wide nginx, so config text from an app repo could take over other apps' domains.

---

## Environment Variables

### Basic Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_DOMAINS` | Domain name for the app | (none) |
| `RIKU_HTTPS_ONLY` | Force HTTPS redirect. On for `true`, `1`, `yes` or `on`; any other value is off | `false` |
| `RIKU_BIND_ADDRESS` | Backend bind address. Must be an IP literal such as `127.0.0.1` or `::1`; a host name like `localhost` fails the deploy of a port mapped app. With the router helper, `0.0.0.0` is routed to as `127.0.0.1` and any other address that is not loopback is refused | `127.0.0.1` |

### Static Files

| Variable | Description | Default |
|----------|-------------|---------|
| `RIKU_NGINX_STATIC` | Serve the app as a static site, no backend | (none) |
| `RIKU_NGINX_DOCUMENT_ROOT` | Static site directory, relative to the app checkout | `public` |
| `RIKU_NGINX_STATIC_PATHS` | URL path to directory mappings, directories relative to the app checkout | (none) |
| `RIKU_NGINX_ALLOW_GIT_FOLDERS` | Allow access to `.git` | `false` |

**Example:**
```bash
RIKU_NGINX_STATIC_PATHS=/static:public,/assets:static
```

The nginx worker cannot read the deploy user's home, so nginx never serves
from the checkout. Each deploy copies the document root (with
`RIKU_NGINX_STATIC`) and every `RIKU_NGINX_STATIC_PATHS` directory into
`/var/lib/riku/static/<app>/`:

```
/var/lib/riku/static/<app>/root       the document root
/var/lib/riku/static/<app>/paths/0    the first RIKU_NGINX_STATIC_PATHS entry
/var/lib/riku/static/<app>/paths/1    the second, and so on
```

The new copy is staged next to the old one and swapped in with one rename,
so a request never sees a half copied tree. `riku destroy` removes it.
Directories are `2750` and files `0640`, group `riku-sockets` (the nginx
worker's group), so no other account can read them.

Every source, and every symlink inside it, must resolve inside the app
checkout. A path or link that leads anywhere else fails the deploy. A
missing source is published as an empty directory. `sudo riku init` creates
`/var/lib/riku/static`; without it a static deploy fails and says so.

### What the route does not do

The route does not do caching, Cloudflare allow lists, Content-Security-Policy headers, SPA catch-all, a snippet include file, or extra listen addresses. An app that needs caching or a CSP sets the header itself.

`RIKU_NGINX_SOCKET` fails a deploy or `riku restart` of an app with a web facing process, naming the key, before any worker changes: the router helper picks socket paths itself, and each uWSGI copy listens on its own socket in `/run/riku-sockets`. The route refuses it in every routing mode too, so a static site that sets it fails at the Route step.

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
location /static {
    alias /var/lib/riku/static/myapp/paths/0;
    expires 30d;
    add_header Cache-Control "public, immutable";
}
```

With `RIKU_NGINX_STATIC` the server's `root` is
`/var/lib/riku/static/myapp/root`.

---

## SSL/HTTPS

### Automatic SSL (ACME)

Riku supports ACME (Let's Encrypt) for automatic SSL certificates.

Every generated server answers `/.well-known/acme-challenge` from the
webroot `/var/lib/riku/acme`, outside the deploy home. `sudo riku init`
creates it owned by the deploy user, group `riku-sockets`, mode `2750`.
When `~/.acme.sh/acme.sh` (or `$RIKU_ACME_ROOT/acme.sh`) exists, a deploy
with `RIKU_HTTPS_ONLY` and `RIKU_DOMAINS` runs it with that webroot and
umask `027`, so the challenge files are readable by nginx's group and no
one else.

acme.sh runs as the deploy user, but nginx reads its certificate from the helper's own copy under `/etc/riku/certs/<app>`. Riku passes acme.sh a reload command, `/usr/local/bin/riku router-renew <app>`, so each renewal sends the new files to the helper. Until a real certificate exists, the site uses a self signed one, and the next deploy tries to issue a real one.

To send the current certificate files to the helper by hand:

```bash
riku router-renew myapp
```

To turn HTTPS on:

```bash
riku settings set myapp routing.domains=example.com routing.https_only=true
```

then deploy again.

---

## Troubleshooting

### Test Nginx Configuration

```bash
sudo nginx -t
```

### View Generated Config

```bash
cat /etc/riku/nginx/myapp.conf
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
# Check the published copy nginx serves
ls -la /var/lib/riku/static/myapp/

# Verify routing.static_paths: the repo's riku.toml, then the server override
grep static_paths ~/.riku/apps/myapp/riku.toml
riku settings get myapp routing.static_paths
```

#### SSL Certificate Errors

Certificate not found, self signed, or expired.

```bash
# What the helper serves
sudo ls -la /etc/riku/certs/myapp/

# Send the renewed files to the helper
riku router-renew myapp
```

#### Config Not Applied

The deploy reports the helper's refusal or the failed `nginx -t`. If the helper itself is down:

```bash
sudo systemctl status riku-router-helper
sudo journalctl -u riku-router-helper -n 50
sudo systemctl restart riku-router-helper
```

---

## Beyond the Core Route

The generated site covers domains, backends, TLS, static files and the HTTP mode. For caching, rate limiting, header policies or an IP allow list, put the feature in the app, or run a proxy in front of riku. WebSocket upgrades already work, with no setting.

---

## See Also

- [Environment Variables](env.md) - All nginx-related ENV vars
- [CLI Reference](cli.md) - Config management commands
