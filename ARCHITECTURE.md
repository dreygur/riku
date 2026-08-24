# Riku Architecture Design Document

## Overview

Riku is a complete Rust port of the Piku micro-PaaS, designed to provide
Heroku-like git push deployments to small servers without Docker. This
document outlines the architecture, design decisions, and implementation
details of the Riku system.

## Goals

1. **Performance**: Efficient Rust implementation, no interpreted runtime
2. **Compatibility**: Maintain full compatibility with existing Piku workflows
3. **Reliability**: Improve stability and error handling
4. **Maintainability**: Clean layering, no language-specific code in core
5. **Extensibility**: A real plugin system, not just new runtimes, but
   addons, routers, event subscribers, value-transform filters, and
   dashboard UI panels

## High-Level Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Git Client    │───▶│   Riku Server    │───▶│  Applications   │
│                 │    │                  │    │                 │
│  git push       │    │  ┌─────────────┐ │    │  Managed by     │
│  (deploys)      │    │  │ Supervisor  │ │    │  Supervisor     │
└─────────────────┘    │  │ (daemon)    │ │    └─────────────────┘
                        │  └─────────────┘ │
                        │  ┌─────────────┐ │    ┌─────────────────┐
                        │  │ Nginx       │ │    │  Plugins        │
                        │  │ (reverse    │ │◀──▶│  runtime/addon/ │
                        │  │  proxy)     │ │    │  router/filter/ │
                        │  └─────────────┘ │    │  event/ui       │
                        └──────────────────┘    └─────────────────┘
```

## Workspace layout

Riku is a Cargo **workspace**: each concern is its own crate under
`crates/`, following a repository / service / provider layering (see this
project's `.claude/CLAUDE.md` for the stated rule, repositories only
read/write data with no logic, services orchestrate logic with no direct
user I/O, providers wire services together and own user-facing output).
The `riku` crate is a thin binary that re-exports every other crate under
its historical module names (`riku::cli`, `riku::deploy`, …), so existing
`crate::`-relative code across the workspace didn't need path changes when
the crates were split out.

| Layer | Crates |
|---|---|
| Repository | `riku-config` (`RikuPaths`), `riku-util`, `riku-error` |
| Service | `riku-nginx`, `riku-deploy`, `riku-supervisor`, `riku-plugins` |
| Provider | `riku-cli`, `riku-dashboard` |
| Standalone runtime plugins | `riku-plugin-java`, `riku-plugin-clojure`, `riku-plugin-container` |

`riku-plugins` is Service, not Provider, despite living next to `riku-cli`
in spirit: `riku-supervisor`, `riku-deploy`, and `riku-nginx` all depend on
it directly (for lifecycle hook dispatch, filters, and events), which only
makes sense if it sits below them in the dependency graph. A Provider
label would mean those three services depend on something above them.

See `API.md` for the per-crate module map.

## Component Architecture

### 1. CLI Layer (`riku-cli`)

Handles user commands and orchestrates operations:

- **Command Parsing**: `clap` (`cli.rs`)
- **Command Routing**: `main.rs` dispatches to `riku-cli` handlers, checking
  client-plugin overrides first (`routing.rs`)
- **Input Validation**: validates user inputs before processing
- **Error Handling**: user-friendly error messages, `tracing` for structured logs

### 2. Configuration System (`riku-config`)

Repository layer: pure path/env resolution, no business logic:

- **Path Resolution**: `RikuPaths::from_env()` resolves every on-disk
  directory from `$RIKU_ROOT`/`$HOME`
- **Directory Structure**: maintains compatibility with Piku's layout
- **Default Values**: sensible defaults for every path field

### 3. Deployment Engine (`riku-deploy`)

`do_deploy()` orchestrates a deploy by calling into the plugin system for
runtime-specific work: the deploy engine itself carries no language-specific
code:

- **Plugin Discovery**: scans `~/.riku/plugins/` for runtime plugin executables
- **Runtime Detection**: delegates to plugins via `detect` (exit 0 = match)
- **Build Dispatch**: calls `build` on the matched plugin; streams
  stdout/stderr to the deploy log
- **Environment Merging**: calls `env` on the plugin; merges `KEY=VALUE`
  output into worker env
- **Worker Configuration**: writes TOML worker configs for the supervisor
- **Router Configuration**: `router.rs` dispatches to nginx (default) or an
  installed router plugin
- **Start Command Fallback**: uses the plugin's `start` output if the
  `Procfile` has no command for a process type

#### Plugin-Based Runtime Dispatch

1. If `RUNTIME=<name>` is in the app ENV → use that plugin directly
2. Otherwise → run `detect` on all non-`riku-*` plugins, sorted
   alphabetically; first exit 0 wins
3. If no plugin matches → deploy fails with a clear error

Plugins receive context via environment variables: `RIKU_APP`,
`RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`, `RIKU_PLUGIN_API`,
`RIKU_PLUGIN_NAME`, `RIKU_PLUGIN_DATA_PATH`.

#### Bundled Runtime Plugins

- `node`: Node.js (npm/yarn/pnpm), detects `package.json`
- `python`: Python (pip/Poetry/uv), detects `requirements.txt` / `pyproject.toml`
- `ruby`: Ruby (Bundler), detects `Gemfile`
- `go`: Go (modules/godeps), detects `go.mod` / `Godeps` / `.go` files
- `rust-lang`: Rust (Cargo), detects `Cargo.toml` + `rust-toolchain.toml`
- `riku-plugin-java`: Java (Maven/Gradle), detects `pom.xml` / `build.gradle`
- `riku-plugin-clojure`: Clojure (Lein/deps.edn), detects `project.clj` / `deps.edn`
- `riku-plugin-container`: Docker/Podman, detects `Dockerfile` / `Containerfile` / `docker-compose.yml`

### 4. Process Supervisor (`riku-supervisor`)

Manages application processes and provides process lifecycle management:

- **Process Management**: spawns, monitors, restarts application processes
  (`process/spawn.rs`, `process/spawned.rs`)
- **Crash Detection & Recovery**: `process/health_check.rs`'s
  `check_processes()` detects a dead process, restarts it with exponential
  backoff (or permanently gives up past `max_restarts`), and fires the
  `app.restarted`/`app.failed` plugin events either way
- **File Watching**: watches worker TOML files for changes, restarts on change
- **Log Rotation**: `log_rotation/`, size/retention-based rotation
- **Cron Scheduling**: `daemon/cron_tasks.rs`, Procfile `cron:` entries
- **Its own HTTP API**: `health/` runs a separate axum server exposing
  `/health`, `/metrics*`, read-only `/plugins`/`/hooks`, and mutating
  `/control/*` actions (create/deploy/restart/stop app, install plugins),
  this is distinct from the `riku-dashboard` crate below, which is its own
  server with its own route table

#### Supervisor modules

- `process/`: spawn/health-check/stop/generation (canary)/orchestration/isolation
- `daemon/`: main loop, config-file watcher, cron tasks, maintenance
- `health/`: the supervisor's own HTTP API (control-plane + metrics)
- `stats/`: resource usage and health-check state tracking
- `config/`: `WorkerConfig` and TOML (de)serialization
- `cgroups/`: cgroup v2 isolation limits
- `log_rotation/`: log file rotation and cleanup

### 5. Nginx Integration (`riku-nginx`)

Generates nginx configurations for applications:

- **Template System**: Tera templates, one of 5 selected per-app by env
  flags (`select_template()`)
- **Multiple Config Types**: plain HTTP, HTTPS-only, static file serving,
  WSGI socket, external port mapping
- **Plugin Augmentation**: `NGINX_INCLUDE_FILE` content is run through the
  `nginx.include_content` filter chain (§ Plugin System) before being
  inlined: any number of installed filter plugins can each contribute a
  snippet to the generated config, without replacing it wholesale
- **ACME Integration**: Let's Encrypt certificate challenges
- **Validation**: generated configs are checked with `nginx -t` before use

### 6. Plugin System (`riku-plugins`)

The full wire contract is `PLUGIN_PROTOCOL.md`; this is the architectural
summary. A plugin is a directory (`riku-plugin.toml` manifest + one or more
executables) under `~/.riku/plugins/`, discovered fresh on every dispatch,
installing a new plugin needs no supervisor restart. Every invocation is a
fresh child process: verb as `argv[1]`, structured input (if any) as one
JSON line on stdin, context via env vars, response on stdout, logs on
stderr, bounded by a shared timeout.

Riku extends in two ways:

- **Behavior seams**: the kernel calls *into* the plugin to get work done.
  - `runtime`: `detect`/`build`/`env`/`start` (§ above)
  - `addon`: `provision`/`bind`/`unbind`/`deprovision`/`backup`; a managed
    resource (database, cache, …) with named instances, each with its own
    data directory
  - `router`: `configure`/`reload`; a host-level singleton, swaps out
    nginx entirely (`RIKU_ROUTER=<name>`)
- **Event subscribers**: the plugin reacts to lifecycle events the kernel
  emits (`deploy.*`, `build.*`, `app.restarted`, `app.failed`, …), verb
  `on_event`, always fire-and-forget (`observe` mode) or veto-capable on
  pre-phase events (`gate` mode, elevated trust). Subscribers on the same
  event run in `priority` order. A plugin may also declare `events.emit =
  true` to fire its own namespaced `plugin.custom.*` events via `riku
  plugin-emit`: the kernel stamps `source_plugin` so a subscriber can
  always tell a real kernel event from a plugin-claimed one.
- **Filters**: the value-transform counterpart to events: verb
  `on_filter`, a plugin receives a value and hands back a (possibly
  transformed) one. Multiple filters on the same name chain in priority
  order. Must degrade safely: any failure (non-zero exit, timeout,
  malformed output) passes the input through unchanged rather than
  breaking the caller: this is why filters have no veto/`gate` mode.
- **UI panels**: verb `ui_panel`, Next.js dashboard only: a plugin returns
  structured JSON (never HTML/JS) that the dashboard renders as its own
  page, reached from a dynamically-added nav entry.
- **Lifecycle hooks**: orthogonal to all of the above: any plugin may
  declare `on_install`/`on_uninstall`, invoked by `riku plugins
  install`/`remove`, always best-effort (a failing hook never blocks the
  install or removal it's attached to).

There is also a legacy, separate hook mechanism (`manager.rs`/`hooks.rs`):
four fixed `riku-*`-prefixed executables (`riku-pre-deploy`,
`riku-pre-build`, `riku-post-build`, `riku-post-deploy`) invoked directly
by `riku-deploy` at the corresponding stage: predates the manifest-based
event bus and is kept for backward compatibility.

**Trust model.** `PluginInstaller` verifies a pinned checksum (rejects on
mismatch) and, if the manifest carries an Ed25519 `signature`, requires it
to verify against a keyring the operator explicitly trusts
(`riku plugins trust add`): an unverified signed bundle is rejected, not
merely flagged. Declared `[capabilities]` (`network`, `writes`,
`privileged`) are enforced at spawn time via Landlock + `PR_SET_NO_NEW_PRIVS`
(`sandbox/`) wherever the kernel supports it, degrading to a logged no-op
(with `no_new_privs` still applied) on older kernels. `privileged = true` is
the one capability that opts a plugin *out* of the sandbox entirely.

### 7. Two dashboards

- **`riku-dashboard`** (in-workspace crate): a single embedded HTML/JS page
  baked into the `riku` binary (`include_str!`), served by `riku dashboard`.
  Its own axum server exposes both the static page and a JSON API
  (`/api/state`, `/api/apps/:app/*`, `/api/plugins`, `/api/plugins/:name/ui`,
  `/api/addons/*`, `/api/marketplace/*`). Not read-only, it can deploy,
  restart, and manage addons, so a token (`RIKU_DASHBOARD_TOKEN`) is
  required before binding it beyond loopback.
- **`dashboard/`** (top-level, *not* a workspace crate), a separate Next.js
  16 / React 19 application, built and deployed independently. It proxies
  the same backend (either dashboard's API, or `riku-supervisor`'s own
  `/control/*` API depending on the action) same-origin, so the browser
  never sees the backend token, and adds its own login: a single shared
  password (scrypt-hashed, `RIKU_DASHBOARD_PASSWORD_HASH`), a signed session
  cookie, enforced by `middleware.ts` on every route. Auth is fully bypassed
  when the password hash env var is unset, matching riku's general
  "don't break default/local usage" philosophy.

### 8. Utility Functions (`riku-util`)

Shared, dependency-free helpers used across the workspace:

- **String Processing**: name sanitization, `expandvars()`-style env expansion
- **File Operations**: Procfile/settings parsing
- **Network Utilities**: free port detection
- **System Utilities**: process execution helpers, requirement checks,
  cron parsing, resource-limit configuration, constant-time comparisons

## Data Flow

### Application Deployment Flow

1. **Git Hook Trigger**: git post-receive hook receives new commits. First
   push for a new app is detected by checking for a `HEAD` file inside the
   target bare repo (`riku_repo.join("HEAD").exists()` in
   `riku-cli/src/git/receive_pack.rs`), not by checking whether the repo
   directory itself exists: a plain directory-existence check is
   unreliable here because the `hooks/` subdirectory created later in the
   same code path would otherwise make the repo appear to "exist" before
   `git init --bare` has actually run, silently skipping initialization.
2. **Code Checkout**: code is synced to the application directory (`git_ops.rs`)
3. **Procfile Parsing**: empty/missing `Procfile` aborts the deploy
4. **Scaling deltas applied**, then **`preflight`** command runs if present
5. **ENV Loading**: app environment variables loaded from `~/.riku/envs/<app>/ENV`
6. **pre-deploy hook**: aborts the deploy on failure
7. **Plugin Discovery & Runtime Detection**: `RUNTIME=` override, or first
   `detect`-exit-0 plugin wins; no match = error
8. **pre-build hook**
9. **Build**: the matched plugin's `build` subcommand; output streamed to
   the deploy log
10. **Env merge**: the plugin's `env` output merged into the app environment
11. **post-build hook**
12. **`release`** command runs if present
13. **`LIVE_ENV` written**, worker TOML configs generated
    (`Procfile` + plugin `start` command as fallback)
14. **Router configured**: nginx (default) regenerates and reloads its
    config, or a router plugin's `configure`/`reload` runs instead
15. **Process Start**: supervisor spawns the new worker configs
16. **post-deploy hook**: non-fatal, a failure is a warning, not an abort
17. **Release recorded** for `riku rollback`

### Process Management Flow

1. **Config File Creation**: deployment writes a TOML worker config
2. **Symlink Creation**: config is symlinked into `workers-enabled/`
3. **File Watcher Notification**: supervisor detects the filesystem change
4. **Process Spawn**: supervisor spawns the new process
5. **Health Monitoring**: continuous crash/health-check monitoring
6. **Automatic Recovery**: a crash triggers a backoff-scheduled restart and
   an `app.restarted` event; exceeding `max_restarts` permanently removes
   the process instead and fires `app.failed`
7. **Config Updates**: modified configs trigger process restarts
8. **Cleanup**: removed configs stop their processes

## Configuration Formats

### Worker Configuration (TOML)

```toml
[worker]
app = "myapp"
kind = "web"
command = "python app.py"
ordinal = 1

[env]
PORT = "5000"
DATABASE_URL = "sqlite:///db.sqlite3"

[options]
working_dir = "/home/riku/.riku/apps/myapp"
log_file = "/home/riku/.riku/logs/myapp/web.1.log"
```

### Scaling Configuration

```
web=2
worker=4
```

### Environment Configuration

```
KEY1=VALUE1
KEY2=VALUE2
```

## Security Model

### SSH Access Control

- SSH key restrictions prevent shell access
- Commands are restricted to Riku operations only
- Public keys are added with command restrictions

### Process Isolation

- Applications run as the deploy user, never as root
- Optional per-worker cgroup v2 isolation (memory/CPU/pids limits)
- Resource limits configurable via `RIKU_MAX_*` env vars

### Unprivileged Worker / Nginx Interaction Model

- The `riku supervisor` daemon and all spawned application workers run
  entirely as the unprivileged deploy user (e.g. `riku`), the daemon is
  never started as root in a correctly configured deployment.
- Nginx's master process runs as root by OS package default, which the
  deploy user cannot signal directly. Riku does not solve this by running
  itself as root; instead, the deploy user is granted a narrowly-scoped
  passwordless sudo rule limited to the `nginx` binary itself (config test
  and reload only: no shell, no other commands), reached through a
  `nginx` wrapper placed ahead of `/usr/sbin/nginx` in `PATH`. This keeps
  the supervisor and every application process fully unprivileged while
  still allowing config reloads to take effect.
- The per-app nginx vhost symlink directory (`/etc/nginx/sites-enabled/`)
  is made group-writable by the deploy user rather than granting broader
  filesystem privileges.
- See `tests/stress/container/sudoers-riku-nginx` and
  `tests/stress/container/nginx-wrapper.sh` for the reference
  implementation, verified end-to-end in the containerized integration
  suite (see Testing Strategy below).

### Plugin Sandboxing

- Checksum verification is mandatory when a manifest pins one; mismatch
  rejects the install outright.
- Ed25519 signatures (optional) require a trusted key to verify, or the
  install is rejected: not merely warned.
- Declared capabilities (`network`, `writes`, `privileged`) are enforced
  at spawn time via Landlock + `no_new_privs`, best-effort on kernels
  without full Landlock support.

### Input Validation

- App names, plugin names sanitized against path traversal
- Nginx-bound env values sanitized against template/config injection
- Symlink targets verified to stay within the riku directory tree

## Performance Characteristics

### Memory Usage

- Low memory footprint (compiled binary, no interpreter)
- Efficient data structures minimize allocations
- Supervisor only loads active worker configurations

### Startup Time

- Fast startup, single static binary
- Supervisor initializes quickly

### Concurrency

- Supervisor uses file watching (`notify` crate) instead of polling
- Process monitoring is event-driven
- Plugin dispatch (event/filter delivery) can run off the main
  supervisor tick on its own thread so a slow or unreachable plugin never
  delays health checks for other apps

## Error Handling Strategy

### Graceful Degradation

- The system continues operating when individual apps fail
- Configuration errors don't affect other applications
- A broken filter/event-subscriber plugin degrades to a no-op (passthrough,
  or a skipped delivery) rather than breaking the caller

### Logging

- Colored console output for CLI-facing messages; `tracing` for structured
  internal logs
- Process logs are stored in application-specific directories under `logs/`

### Recovery

- Automatic process restart with exponential backoff (jittered, capped at 60s)
- `max_restarts` past which a process is permanently removed rather than
  retried forever
- Release history (`riku rollback`) for redeploying a prior commit

## Testing Strategy

### Unit Tests

Every crate carries its own unit test suite (`cargo test --workspace`),
covering utility functions, runtime/plugin dispatch, worker configuration,
and edge-case handling. New plugin-system features in particular are
verified end-to-end against a real installed plugin bundle and real
dispatch code, not mocks (see recent additions to `riku-plugins`,
`riku-supervisor`, `riku-nginx` test modules for the pattern).

### Integration Tests

- Full deployment workflows
- Process lifecycle management
- Configuration updates
- Error condition handling

### Containerized Production Integration Suite

`tests/stress/container/run_container_test.sh` builds a real
target server image (Ubuntu 24.04, sshd, nginx, the compiled `riku`
binary, bundled runtime plugins), provisions a throwaway SSH keypair,
boots the container, performs an actual `git push` deploy of a mock app
over SSH, then drives concurrent HTTP load against the nginx-proxied
app and collects a structured pass/fail verdict (502/504 count, zombie
process check, supervisor liveness). It runs with either Docker or
Podman: the script detects whichever is on `PATH` and uses it
transparently, no flags needed:

```bash
./tests/stress/container/run_container_test.sh
```

See `tests/stress/README.md` for the full suite (lifecycle stress,
fd/leak monitor, chaos signal tests, resource limit audit) and
`tests/stress/container/` for this containerized suite specifically.
There is also a separate `tests/demo/` podman-compose sandbox used for
manual/browser-driven verification of dashboard and plugin changes.

## Deployment Compatibility

### Directory Structure

```
~/.riku/
├── apps/               # Application code (checked-out source)
├── data/                # Persistent data
│   └── plugin-data/     # Per-plugin scratch directories
├── envs/               # Environment variables (<app>/ENV, <app>/LIVE_ENV)
├── repos/              # Git bare repositories
├── logs/               # App logs (<app>/deploy.log, <app>/web.1.log, …)
├── nginx/              # Nginx configurations
├── cache/              # Nginx cache files
├── workers/            # Worker process configurations
├── workers-available/  # Available worker TOML configs
├── workers-enabled/    # Enabled worker configs (symlinks)
├── acme/               # ACME/Let's Encrypt certificates
└── plugins/            # Plugin bundles (runtime, addon, router, event/filter/UI, legacy hooks)
```

### File Formats

- `Procfile` support for process definitions
- `SCALING` file for process counts
- `ENV` files for environment variables
- `riku-plugin.toml` manifests for plugin bundles
- Standard git hooks for deployment

## Future Extensions

See `ROADMAP.md` for the maintained, prioritized list. At a glance: a
one-line installer and `riku quickstart` (solo-dev DX), stateful-app addons
beyond Postgres, a marketplace/distribution model for third-party plugins,
a WASM sandbox for untrusted plugin authors, and green/blue deployments.

## Implementation Notes

### Rust-Specific Decisions

- **Error Handling**: `anyhow` for application errors
- **CLI Framework**: `clap`
- **Templating**: `tera` for nginx config generation
- **File Watching**: `notify`
- **Process Management**: `nix` for Unix process operations
- **HTTP**: `axum` for both the supervisor's and the embedded dashboard's APIs
- **Plugin Manifests**: `toml` + `serde`

This architecture provides a solid foundation for a high-performance,
reliable, extensible micro-PaaS while maintaining full compatibility with
the existing Piku ecosystem.
