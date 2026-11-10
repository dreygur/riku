# Your own certificate

By default riku asks Let's Encrypt for a certificate for each app, or makes
a self-signed one when that is not possible. When you already have a
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
applied again right away, so no deploy is needed.

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

removes the certificate, the key and the marker. The router keeps serving the
old certificate until the next deploy, which issues a new one the usual way.
`unset` refuses an app that has no own certificate.

## Renewal is yours

riku does not renew your certificate. `riku doctor` warns when it ends in
fewer than 14 days and fails once it has expired. Run `riku certs set` again
with the new files; the same command replaces the old pair.

Destroying the app removes its own certificate too.
