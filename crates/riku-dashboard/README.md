# riku-dashboard

The dashboard backend compiled into the `riku` binary and served by
`riku dashboard`: a small Axum server with one embedded HTML page and a JSON
API. It binds `127.0.0.1` by default, needs a token (`RIKU_DASHBOARD_TOKEN` or
`--token`) to bind any other address, and rejects non-loopback `Host` headers.
Mutating endpoints always require the token in the `Authorization: Bearer`
header.

The Next.js frontend in [`dashboard/`](../../dashboard/README.md) proxies to
this server.

- [src](./src/README.md)

Run the tests with `cargo test -p riku-dashboard`.
