# riku-dashboard (src)

## Interface
- `run` [lib.rs:run] (fn): binds, prints `riku-dashboard-bound <port>` on stdout, and serves; `DashboardState` [lib.rs:DashboardState] (struct, crate-private)
- `router`, `authorize` [routes.rs] (fn): the page and the read API, and the read gate
- `index`, `asset`, `ASSETS` [assets.rs]: `GET /` and `GET /assets/{*path}`, served from files under `../assets/` embedded at compile time
- `router`, `finish`, `authorize_mutation` [mutations.rs] (fn): app actions (restart, stop, redeploy, scale, rollback and others under `/api/apps/{app}/`), gated on a Bearer token
- `restart`, `stop`, `scale` [service_mutations.rs] (fn): mutating actions on container services
- `list`, `create`, `bind`, `unbind`, `backup`, `deprovision` [addons.rs] (fn): addon endpoints through `AddonService`
- `get_env`, `edit_env` [appcfg.rs] (fn), `EnvEdit` [appcfg.rs:EnvEdit] (struct): `GET` and `POST /api/apps/:app/env`
- `list` [installed.rs] (fn): `GET /api/plugins`
- `stream` [logs.rs:stream] (fn), `LogQuery` [logs.rs:LogQuery] (struct): live log tail over Server-Sent Events
- `list_sources`, `search`, `add_source`, `remove_source`, `install`, `remove_plugin`, `list_keys`, `add_key`, `remove_key` [market.rs] (fn): marketplace sources, plugin install and the signature trust keyring
- `doctor`, `backup_app`, `restore_app` [system.rs] (fn): `riku doctor` as JSON, and app backup and restore

## Invariants
- [assets.rs] An asset path is only a key into the fixed `ASSETS` table, never a filesystem path; the page and its assets are public like the index, and all data stays behind `authorize`
- [lib.rs] Binding a non-loopback address requires a token; a `Host` allowlist rejects non-loopback Host headers
- [mutations.rs] A token is always required for mutations and must arrive in the `Authorization: Bearer` header, never a query parameter
- [appcfg.rs, mutations.rs, addons.rs] Writes reuse the same service and command functions the CLI calls

## Side Effects
- [lib.rs] Binds the listening socket and reads `RIKU_DASHBOARD_TOKEN`
- [logs.rs] Tails every `*.log` under `{log_root}/{app}/`
- [appcfg.rs] Writes the app's `ENV` file through `config set` / `config unset`
- [system.rs] Writes backup archives and restores app files

## Verification
- Run: `cargo test -p riku-dashboard`
