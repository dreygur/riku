# Your own certificate

By default an app's certificate comes from Let's Encrypt: with nginx riku
asks for it (or makes a self-signed one when that is not possible), with
Caddy, Caddy does. When you already have a
certificate (from a company CA, a wildcard you bought, an origin
certificate), give it to the app instead. riku then serves it as is and never
issues, renews or replaces it.

## Set it

Send the certificate chain and the private key as one PEM stream on stdin.
The order does not matter, but the leaf certificate must come first in the
chain.

```bash
cat fullchain.pem key.pem | rk certs set myapp
```

This works the same with `riku` on the server. Before anything is saved
riku checks that:

- the chain parses and the key matches the first certificate;
- the certificate has not expired.

It only warns, and still installs, when the certificate does not cover one
of the app's `RIKU_DOMAINS`. A wildcard such as `*.shop.example` covers one
label (`www.shop.example`, not `a.b.shop.example` or `shop.example`).

The key is written with mode `0600` and is never printed. The route is
applied again right away, so no deploy is needed. Both routers serve it:
nginx directly, Caddy through a `tls` line in the app's site (plain HTTP
addresses then get a separate site block, which Caddy requires).

## Show it

```bash
rk certs show myapp
```

prints the source (`own certificate`, `Let's Encrypt`, `self-signed` or
`none`), the names the certificate covers and its end date. The source is
worked out from the files in the server's nginx directory: a full chain file
without the own certificate marker means acme.sh installed it.

## Remove it

```bash
rk certs unset myapp
```

removes the certificate, the key and the marker, then applies the route
again, so a certificate is issued the usual way right away. `unset` refuses
an app that has no own certificate.

## Renewal is yours

riku does not renew your certificate. `riku doctor` warns when it ends in
fewer than 14 days and fails once it has expired. Run `riku certs set` again
with the new files; the same command replaces the old pair.

Destroying the app removes its own certificate too.
