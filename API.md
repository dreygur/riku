# Riku API Documentation

## Overview

Riku is a Rust port of the Piku micro-PaaS, providing Heroku-like git push
deployments to small servers. Since the v3.0.0 rewrite, riku is a Cargo
**workspace**: each concern lives in its own crate under `crates/`, and the
`riku` crate is a thin binary that re-exports the others under legacy module
names (`riku::cli`, `riku::deploy`, etc. see `crates/riku/src/lib.rs`) so
existing `crate::` paths in downstream code didn't need to change.

Language-specific deploy logic (`deploy/python.rs`, `deploy/node.rs`, …) no
longer exists: build/run for every runtime is now delegated to **runtime
plugins** (§ Plugin System below), and the core binary carries no
language-specific code at all.

## Workspace crates

| Crate | Layer | Purpose |
|---|---|---|
| `riku` | binary | `main.rs` parses CLI args and dispatches; `lib.rs` re-exports every other crate for the binary and integration tests |
| `riku-config` | repository | `RikuPaths`: resolves every on-disk directory the rest of the system needs |
| `riku-util` | repository | Shared helpers: env parsing, Procfile parsing, cron, SSH keys, resource limits, secure comparisons |
| `riku-error` | repository | Shared error types (`DeployError`, …) |
| `riku-nginx` | service | Nginx config generation: Tera context construction, template selection, SSL, Cloudflare ACLs |
| `riku-plugins` | service | The entire plugin system, see below (`riku-supervisor`, `riku-deploy`, and `riku-nginx` all depend on it for hook dispatch, so it sits below them, not above, despite the name) |
| `riku-deploy` | service | Deployment orchestration: git sync, hooks, worker/env setup, router dispatch, releases, locking, scaling |
| `riku-supervisor` | service | Process supervisor: spawn/health-check/restart, cgroups, log rotation, cron, and its own axum HTTP API (`/health`, `/metrics`, `/control/*`) |
| `riku-cli` | provider | CLI command handlers, argument parsing (clap), user-facing output |
| `riku-dashboard` | provider | Embedded, single-binary HTML+JS dashboard and its own read/write API, served by `riku dashboard` |
| `riku-plugin-java`, `riku-plugin-clojure`, `riku-plugin-container` | - | Standalone Rust binary runtime plugins, compiled separately and distributed as GitHub release assets |

There is also a separate, non-workspace Next.js application at `dashboard/`
(top-level, not under `crates/`), a fuller browser dashboard that proxies
`riku-supervisor`'s HTTP API and adds its own password-gated login. It is
built and deployed independently of the Rust binary.

## `riku-cli`: command handlers

Organized by subcommand area, one directory per area:

- `apps/`: `create.rs`, `deploy.rs`, `destroy.rs`, `config.rs`, `control.rs`, `info.rs`, `list.rs`, `logs.rs`, `rollback.rs`, `stats.rs`, `install_plugins.rs`, `plugin_emit.rs`, `dump_state.rs`, `process/`
- `addon/`: `riku addon` subcommands (create/bind/unbind/destroy/backup/list)
- `plugins/`: `riku plugins` subcommands (install/remove/list/doctor/scaffold, marketplace/trust management)
- `client_plugins/`: discovery and execution of client-side plugins (run on the developer's machine, not the server)
- `git/`: `hook.rs` (post-receive handler), `receive_pack.rs`, `repo.rs`
- `doctor/`: `riku doctor` diagnostic checks
- `quickstart/`: `riku quickstart` app scaffolding
- `setup/`: `riku init`/setup: binary install, SSH, systemd/user-service units
- `agent/`: machine-readable agent-mode CLI surface (schema, auth, execute)
- `cli.rs`: all clap command/subcommand definitions
- `routing.rs`: dispatches client-plugin overrides of built-in commands
- `hooks.rs`, `backup.rs`, `container.rs`, `control_actions.rs`, `cmds.rs`, `scp.rs`, supporting handlers

## `riku-deploy`: deployment orchestration

`do_deploy()` (`lib.rs`) is the deploy pipeline entry point, called from
`riku-cli`'s `cmd_deploy` (itself reached via the git post-receive hook or a
local-path deploy). In order: acquire a per-app deploy lock (`lock.rs`) →
sync the working tree (`git_ops.rs`) → parse the `Procfile` → apply scaling
deltas (`scaling.rs`) → run `preflight` → load app `ENV` → **pre-deploy**
hook (`hooks.rs`, abort-on-failure) → detect a runtime plugin
(`riku_plugins::runtime`) → **pre-build** hook → build via the plugin →
merge the plugin's `env` output → **post-build** hook → run `release` →
write `LIVE_ENV` (`env_setup.rs`) → write worker TOML configs
(`workers.rs`) → configure the router (`router.rs`: nginx by default, or
a router plugin) → spawn processes (`supervisor_ctl.rs::spawn_app`) →
**post-deploy** hook (non-fatal) → record the release (`releases.rs`) for
`riku rollback`.

`worker_control.rs` / `service_update.rs` handle restart/stop/scale outside
the deploy path. `backup.rs`/`container_runtime.rs` back `riku backup` and
`riku container` respectively.

## `riku-plugins`: the plugin system

The full contract is `PLUGIN_PROTOCOL.md`; this is the module map.

- `manifest.rs`: `PluginManifest` (name, version, `plugin_type`, `api`,
  `entry`, optional `checksum`/`signature`, `capabilities`, and the opt-in
  blocks `lifecycle`, `events`, `filters`, `ui`)
- `bundles.rs`: discovers installed plugin bundles under `~/.riku/plugins/`
- `install.rs`: `PluginInstaller`: install/remove, checksum + signature
  verification, `on_install`/`on_uninstall` lifecycle hook invocation
- `lockfile.rs`, `signing.rs`, `riku-plugins.lock`, Ed25519 keyring
- `runtime.rs`: the 4-verb runtime protocol (`detect`/`build`/`env`/`start`)
- `addon/`: the addon seam (`AddonService`, `InstanceRecord`; verbs
  `provision`/`bind`/`unbind`/`deprovision`/`backup`)
- `router/`: the router seam (`configure`/`reload`), a host-level singleton
- `events/`: `EventBus`, `EventName` (kernel-emitted: `deploy.*`,
  `build.*`, `app.restarted`, `app.failed`), plugin-emitted
  `plugin.custom.*` events with kernel-stamped `source_plugin` provenance
- `filters/`: `FilterBus`: a value-transform chain (`on_filter` verb),
  always degrades to passthrough on any plugin failure
- `ui/`: `ui_panel` verb dispatch for the Next.js dashboard's plugin panels
  (structured JSON only, never HTML/JS)
- `sandbox/`: Landlock + `no_new_privs` capability enforcement from a
  plugin's declared `[capabilities]`
- `executor.rs`: shared spawn/timeout helpers used by every dispatch site
- `plugin_data.rs`: the per-plugin scratch directory (`RIKU_PLUGIN_DATA_PATH`)
- `marketplace/`, `discovery.rs`, marketplace index/search, plugin listing

## `riku-supervisor`: process supervision

- `process/`: `spawn.rs`/`spawned.rs` (spawn and track a `SpawnedProcess`),
  `health_check.rs` (`ProcessManager::check_processes`: crash detection,
  restart with backoff, `app.restarted`/`app.failed` event emission),
  `stop.rs`, `generation.rs`/`orchestration.rs` (canary/rollback), `isolation.rs`
- `daemon/`: the supervisor's main loop, config-file watching, cron tasks
- `health/`: the supervisor's own axum HTTP API: `mod.rs` (routing),
  `control.rs` (`/control/*` mutating actions: create/deploy/restart/stop
  app, install plugins), `plugins.rs` (`/plugins`, `/hooks` read-only
  listings), `auth.rs` (token auth), `actions.rs`/`responses.rs`
- `stats/`: `StatsManager`, resource usage tracking, health-check state
- `config/`: `WorkerConfig`/`WorkerInfo`/`WorkerOptions`, TOML (de)serialization
- `cgroups/`, `log_rotation/`, cgroup v2 isolation limits, log file rotation

## `riku-nginx`: nginx config generation

- `context.rs`: builds the Tera template context from sanitized env vars;
  `insert_include_file()` reads the app's `NGINX_INCLUDE_FILE` (if any) as a
  seed value, then runs it through the `nginx.include_content` filter chain
  before inserting it
- `template.rs`: `generate_nginx_config_from_template()`, `select_template()`
  (picks one of 5 `.tera` templates by env flags), install/reload the
  `/etc/nginx/sites-enabled/` symlink
- `ssl.rs`, `cloudflare.rs`, `sanitize.rs`, `validate.rs`

## `riku-dashboard`: embedded dashboard

- `routes.rs`: route table for both the static `index.html` and the JSON
  API it calls (`/api/state`, `/api/apps/:app/*`, `/api/plugins`,
  `/api/plugins/:name/ui`, `/api/addons/*`, `/api/marketplace/*`, `/api/doctor`)
- `installed.rs`, `market.rs`, `addons.rs`, `mutations.rs`, `system.rs`,
  `logs.rs`, `appcfg.rs`, the individual handler modules
- `index.html`: the whole frontend, embedded via `include_str!`

## Key data structures

### `PluginManifest` (`riku-plugins::manifest`)
`name`, `version`, `plugin_type` (`Runtime`/`Addon`/`Router`/`Notifier`/`Hook`),
`api`, `entry`, optional `checksum`/`signature`/`description`/`author`,
`capabilities` (`network`, `writes`, `privileged`), `lifecycle`
(`install`/`uninstall`), `events` (`subscribe`, `mode`, `priority`, `emit`),
`filters` (`subscribe`, `priority`).

### `WorkerConfig` (`riku-supervisor::config`)
`worker: WorkerInfo` (app, kind, command, ordinal), `env: HashMap<String,
String>`, `options: WorkerOptions` (working_dir, log_file, uid/gid, timeout,
grace_period, max_restarts, optional health_check/isolation).

### `RikuPaths` (`riku-config`)
`riku_root`, `riku_script`, `plugin_root`, `app_root`, `data_root`,
`env_root`, `git_root`, `log_root`, `nginx_root`, `cache_root`,
`workers_root`, `workers_available`, `workers_enabled`, `acme_root`,
`acme_www`. Built via `RikuPaths::from_env()` (honors `$RIKU_ROOT`/`$HOME`)
or `RikuPaths::for_tests()`.

## Error handling

`anyhow::Result<T>` is the standard return type across the workspace; errors
propagate up to the CLI/dispatch layer, where they're reported to the user
or logged via `tracing`.

## Testing

Every crate carries its own unit and integration tests
(`cargo test --workspace`), plus a shell-based deployment test suite under
`tests/deploy/`. Run `cargo test` for the Rust suite; see `CONTRIBUTING.md`
for the full test workflow.
