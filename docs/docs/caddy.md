# Caddy Router

Riku can route apps through Caddy instead of nginx. Caddy gets certificates for every app domain on its own, so there is no ACME setup on the riku side. nginx stays the default.

The router is a host setting, not a per-app one. It lives in `[router] name` of `~/.riku/settings.toml` under the deploy user's home: `nginx`, `caddy`, or the name of a router plugin. When it is not set, the router is nginx.

---

## Switching a Host to Caddy

Install Caddy from your distribution (or Caddy's own apt repository), then run init as root:

```bash
sudo apt install caddy
sudo RIKU_ROUTER=caddy riku init --ssh-key ~/.ssh/authorized_keys
```

`RIKU_ROUTER` is read only by `riku init`. Deploys, the supervisor and `riku doctor` read settings.toml. Running `riku init` again without `RIKU_ROUTER` keeps the current choice.

Root init creates the deploy user when it does not exist: `riku`, or the name in `RIKU_USER`.

### What init changes

Under Caddy, a root `riku init`:

- sets `[router] name = "caddy"` in `/home/riku/.riku/settings.toml`
- creates `/etc/riku` (root owned, `0755`) and `/etc/riku/caddy`, owned by root, group set to the `caddy` user's primary group, mode `2750`
- appends one line to `/etc/caddy/Caddyfile`, once:
  ```
  import /etc/riku/caddy/*.caddy
  ```
- moves Caddy's admin API to a unix socket, `admin unix//run/caddy/admin.sock`, so only `caddy` and root can reach it
- installs and starts the router helper (below)

The site files are the only thing Caddy reads from riku. It proxies to TCP ports, keeps its certificates in its own storage, and serves no files from disk, so the `caddy` user is put in no extra group and gets no access to the deploy user's home.

### The router helper

The deploy user does not write Caddy config or run `caddy`. A small root service does. It is the root owned `/usr/local/bin/riku` running as `riku-router-helper.service`, listening on `/run/riku-router.sock`. A deploy sends it the app's route, the helper checks the route again, writes `/etc/riku/caddy/<app>.caddy` and reloads Caddy. The nginx router uses the same helper, with sites in `/etc/riku/nginx` and certificates in `/etc/riku/certs`. See [Nginx](nginx.md#the-router-helper).

### Reloads

The helper reloads Caddy through the admin socket after it writes or removes a site, and waits for Caddy to accept the config. If the reload fails, the helper puts the previous site file back, the deploy fails and shows Caddy's message, and `riku doctor` reports the last failure. Caddy keeps serving its old config when it refuses a reload.

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

Each app with a domain gets one file, `/etc/riku/caddy/<app>.caddy`:

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

A bare domain makes Caddy ask Let's Encrypt and ZeroSSL for a certificate. A domain no public CA will sign, such as `shop.test`, is refused (`rejectedIdentifier`) and Caddy keeps retrying until the app is destroyed, so a test host makes outbound ACME requests for every such app. Riku does not change this.

An app that serves its own certificate is the exception. Riku writes the pair to `/etc/riku/caddy/certs/<app>/` (readable by the `caddy` group only) and the site file names it with a `tls` line, so Caddy issues nothing for that site. Removing the own certificate or the app removes the files.

---

## Limits

Caddy only proxies HTTP over TCP. Under Caddy, a deploy fails with `Caddy proxies TCP only` when the app:

- runs a uWSGI socket process (`wsgi:`, `jwsgi:`, `rwsgi:` or `php:` in the `Procfile`), or
- is a static site (`RIKU_NGINX_STATIC`).

Run a `web:` process that listens on `$PORT` instead. nginx specific keys (`RIKU_NGINX_ALLOW_GIT_FOLDERS`, custom `nginx.conf` files) have no effect under Caddy. `RIKU_NGINX_SOCKET` fails a deploy under either router.

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
