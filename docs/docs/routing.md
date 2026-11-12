# App URLs

Every app gets a URL on its first push. There are three ways an app is
reached, chosen per app in [riku.toml](riku-toml.md) with `routing.mode`:

| Mode | URL | When |
| --- | --- | --- |
| `domain` | `https://shop.example.com` | The app has `domains`, or the server has a fallback domain. |
| `port` | `http://203.0.113.10:8100` | The app has no domain and the server has no fallback, or the app asks for it. |
| `path` | `http://203.0.113.10:8080/shop/` | The app asks for it. |

With no `routing.mode`, an app with a domain (its own or the fallback) is in
domain mode, and one without is in port mode. The push output ends with the
app's URL.

## The fallback domain

An app with no `domains` gets a name from the server's fallback pattern in
[settings.toml](settings.md):

```toml
[routing]
fallback_domain = "{app}.{ip}.sslip.io"
server_ip = "203.0.113.10"
```

Riku fills in `{app}` and `{ip}` and knows nothing else about the pattern, so
any wildcard DNS service or a wildcard record of your own works:
`{app}.apps.example.com` needs no `server_ip` at all. Remove
`fallback_domain` to turn the fallback off.

The fallback name is not covered by `https_only`, which applies to the app's
own domains only. Riku tries to get a certificate for it:

- nginx: the site is first served over plain HTTP so the ACME challenge
  reaches it, then one certificate is requested. With a certificate the app
  answers on both HTTP and HTTPS, with no redirect, so a lapsed renewal never
  takes it offline. Without one it answers on HTTP and the push says so;
  riku waits an hour before asking Let's Encrypt again. A fallback name never
  gets a self-signed certificate.
- Caddy: Caddy gets the certificate itself; the push says HTTP works now and
  HTTPS once Caddy has it.

An app name that is not a valid DNS label (`my_app`) gets no fallback name,
and the push says why.

## Server IP

`riku init` looks up the address the server uses for outbound traffic and
writes it as `server_ip` when it is a public IPv4 address. On a cloud VM
behind NAT the detected address is private, so init leaves `server_ip` out
(the fallback is off) and prints the line to add with the public address.
A private address would give a URL that only works inside that network.

## Port mode

```toml
[routing]
mode = "port"
port = 8181      # optional
```

The router listens on the port for any host name and proxies to the app.
Without `port`, riku picks the lowest free one from `port_range` (default
`8100-8199`) and keeps it for the app while it stays in port mode; it is
freed when the app gains a domain. `port` beside `domains` makes the app
answer on both.

A port is refused when it is below 1024 (80 and 443 serve domains, and a
bare IP cannot get a certificate for 443), when it is the Caddy admin port
2019 or the shared path port, when another app holds it, or when anything on
the server already listens on it. After the reload riku checks the router
really listens there and puts the old config back if it does not.

Open the port in the server's firewall; the push names it. A bare IP gets
plain HTTP only.

## Path mode

```toml
[routing]
mode = "path"
```

The app answers at `/<app>/` on the server's `shared_port` (default 8080).
The prefix is stripped before the request reaches the app and sent as
`X-Forwarded-Prefix`, and `/<app>` redirects to `/<app>/`. The app must
build its links with that base path; the push reminds you. Path mode takes
no `domains` or `port`, and is not available for uWSGI or container apps.

## Servers without a [routing] section

A settings.toml without `[routing]` means the defaults: no fallback domain,
`shared_port = 8080` and `port_range = "8100-8199"`. An app without a domain
of its own is then in port mode. `riku init` adds the section whenever it is
missing, and `riku doctor` prints a `routing` line saying which applies.
