# Caddy Router

Riku can route apps through Caddy instead of nginx. Caddy gets certificates for every app domain on its own, so there is no ACME setup on the riku side. nginx stays the default.

The router is a host setting, not a per-app one. It lives in `~/.riku/ROUTER` under the deploy user's home: `nginx`, `caddy`, or the name of a router plugin. A missing file means nginx.

---

## Switching a Host to Caddy

Install Caddy from your distribution (or Caddy's own apt repository), create the deploy user, then run init as root:

```bash
sudo apt install caddy
sudo adduser --disabled-password --gecos '' deploy
sudo RIKU_ROUTER=caddy riku init
```

`RIKU_ROUTER` is read only by `riku init`. Deploys, the supervisor and `riku doctor` read the `ROUTER` file. Running `riku init` again without `RIKU_ROUTER` keeps the current choice.

Root init fails if the deploy user does not exist. Set `RIKU_USER` when it is not called `deploy`.

### What init changes

Under Caddy, a root `riku init`:

- writes `caddy` to `/home/deploy/.riku/ROUTER`
- appends one line to `/etc/caddy/Caddyfile`, once:
  ```
  import /home/deploy/.riku/caddy/*.caddy
  ```
- adds the `caddy` user to the deploy user's primary group
- gives that group traverse access (`g+x`, not listing) to the deploy home and `~/.riku`
- creates `~/.riku/caddy` with mode `0750`, owned by the deploy user
- installs `riku-caddy.path` and `riku-caddy-reload.service`: the path unit watches `~/.riku/caddy` and runs `systemctl reload caddy` when a site file changes
- disables `riku-nginx.path` if an earlier init enabled it

Everything inside the deploy user's home is created and changed as the deploy user, never as root.

### Set the ACME email

Caddy's account email is a global option in the system Caddyfile, not a riku setting. Add it at the top of `/etc/caddy/Caddyfile`:

```
{
	email you@example.com
}
```

Then `sudo systemctl reload caddy`. The `RIKU_ACME_*` keys only apply to nginx.

---

## Site Files

Each app with a domain gets one file, `~/.riku/caddy/<app>.caddy`:

```
# riku app: shop
shop.example.com, www.shop.example.com, http://shop.example.com, http://www.shop.example.com {
	reverse_proxy 127.0.0.1:5000
}
```

- The domains come from `RIKU_DOMAINS`, split on spaces and commas. Each one is checked as a hostname, so a value such as `a.com {` fails the deploy and the previous site file stays as it was.
- The port is the app's web worker port, the same one nginx would proxy to.
- An app without `RIKU_DOMAINS` gets no site. Dropping `RIKU_DOMAINS` and redeploying removes the old site file, so Caddy stops serving a stale route.
- `riku destroy` removes the site file.

Do not edit these files by hand: every deploy rewrites them.

---

## HTTPS

| `RIKU_HTTPS_ONLY` | Behavior |
| ----------------- | -------- |
| `true`, `1`, `yes` or `on` | Bare domains only. Caddy serves HTTPS and redirects plain HTTP to it. |
| anything else, or unset | Bare domains plus `http://` addresses. Caddy serves HTTPS and also answers plain HTTP without a redirect. |

---

## Limits

Caddy only proxies HTTP over TCP. Under Caddy, a deploy fails with `Caddy proxies TCP only` when the app:

- runs a uWSGI socket process (`wsgi:`, `jwsgi:`, `rwsgi:` or `php:` in the `Procfile`), or
- is a static site (`RIKU_NGINX_STATIC`).

Run a `web:` process that listens on `$PORT` instead. nginx specific keys (`RIKU_NGINX_CACHE_*`, `RIKU_NGINX_ALLOW_GIT_FOLDERS`, custom `nginx.conf` files) have no effect under Caddy.

---

## Checking a Caddy Host

```bash
riku doctor
```

On a Caddy host, doctor checks that `caddy` is installed, that `/etc/caddy/Caddyfile` has the import line (and prints the line to add when it is missing), and runs `caddy validate` on the Caddyfile. Run `sudo riku doctor` when it cannot read the Caddyfile; under sudo it still checks the deploy user's riku tree.

`riku apps info <app>` shows the app's `Caddy Site:` path.

---

## Switching Back to nginx

```bash
sudo RIKU_ROUTER=nginx riku init
```

Then redeploy each app so nginx gets its config. Remove the `import` line from `/etc/caddy/Caddyfile` and stop Caddy if nothing else uses it.
