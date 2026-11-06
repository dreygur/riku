# riku-dashboard

The dashboard backend compiled into the `riku` binary and served by
`riku dashboard`: a small Axum server with an embedded UI and a JSON API. It
binds `127.0.0.1` by default, needs a token (`RIKU_DASHBOARD_TOKEN` or
`--token`) to bind any other address, and rejects non-loopback `Host` headers.
Mutating endpoints always require the token in the `Authorization: Bearer`
header.

The Next.js frontend in [`dashboard/`](../../dashboard/README.md) proxies to
this server.

The embedded UI lives in [`assets/`](./assets): `index.html` (markup only),
`app.css`, and native ES modules under `js/`. There is no build step. Each
file is compiled into the binary with `include_str!` and served at
`/assets/<path>`; a new module must be added to the `ASSETS` table in
`src/assets.rs`, and a test fails if it is not. The modules are plain
JavaScript with `// @ts-check` and JSDoc types.

- [src](./src/README.md)

Run the tests:

```sh
cargo test -p riku-dashboard
cd crates/riku-dashboard/assets
npx -y -p typescript@5 tsc -p jsconfig.json   # type-check the modules
node --test 'js/tests/*.test.js'              # unit tests for the pure modules
```
