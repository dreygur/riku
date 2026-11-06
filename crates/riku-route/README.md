# riku-route

The route the router helper accepts for one app, and the checks it runs on one.

## Interface
- `Route`, `RouteBackend`, `Listen`, `Tls` [schema.rs]: the wire form, every struct refusing unknown fields
- `check_nginx_route` [check.rs]: the one gate for nginx
- `check_caddy_route` [check.rs]: the gate plus what Caddy cannot take (unix sockets, static sites, port mappings, a tls pair)
- `answer` [answer.rs]: dispatches one request to the nginx or Caddy half of the helper

## Invariants
- [schema.rs] No field is a path; unix backends are copy numbers and the helper builds the socket path
- [check_tls.rs] `cert_pem` holds certificates only and `key_pem` exactly one private key that matches the first certificate
- [check.rs] TCP backends are loopback only; text fields are checked against the nginx directive breaking characters
