# crates

The Cargo workspace members. The root `Cargo.toml` is a workspace manifest
only; `riku` is one member crate among these.

- [riku](./riku/README.md): the `riku` binary entry point, plus a `lib.rs` re-exporting the other crates.
- [riku-args](./riku-args/README.md): the clap command tree and help, with no server side dependencies.
- [riku-bundle](./riku-bundle/README.md): what a plugin bundle is and how it is signed.
- [riku-caddy](./riku-caddy/README.md): the Caddy router, one site file per app under `<riku root>/caddy`.
- [riku-cli](./riku-cli/README.md): server side command handlers (the CLI provider layer).
- [riku-client](./riku-client/README.md): `rk`, the laptop only client that runs local commands or forwards to a server over ssh.
- [riku-config](./riku-config/README.md): path and environment configuration.
- [riku-container](./riku-container/README.md): container services for apps that ship a compose file.
- [riku-dashboard](./riku-dashboard/README.md): the embedded read-only dashboard served by `riku dashboard`.
- [riku-deploy](./riku-deploy/README.md): deployment orchestration.
- [riku-error](./riku-error/README.md): shared error types.
- riku-http: builds the reqwest HTTP clients for the server side crates with ring as the rustls provider. Kept out of riku-util because `rk` links riku-util and must not link reqwest.
- [riku-nginx](./riku-nginx/README.md): nginx configuration generation.
- [riku-route](./riku-route/README.md): the route the router helper accepts and the one gate it passes before it is rendered.
- [riku-plugin-clojure](./riku-plugin-clojure/README.md): Clojure runtime plugin.
- [riku-plugin-java](./riku-plugin-java/README.md): Java runtime plugin.
- [riku-plugins](./riku-plugins/README.md): the plugin system (discovery, hooks, events, addons).
- [riku-remote](./riku-remote/README.md): laptop side behaviour shared by the `riku` and `rk` binaries.
- [riku-supervisor](./riku-supervisor/README.md): the supervisor daemon that manages app processes.
- [riku-sync](./riku-sync/README.md): riku's own file sync, used instead of rsync.
- [riku-util](./riku-util/README.md): shared helpers.
