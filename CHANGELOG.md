# Changelog

All notable changes to Riku will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 2026-07-08

- Removed `riku container` (build, export, deploy-remote, deploy-archive, check-remote), its supervisor route `POST /control/apps/:app/container/export` and the unused `riku-sync` crate. Build images off the server by pushing them to a registry and naming them with `image:` in compose; see [Build images off the server](docs/docs/container-images.md).

- Router helper and a smaller core route. `riku` now lives at the root owned `/usr/local/bin/riku`, and `scripts/install.sh` needs root for it (`| sudo sh`; `rk` is unchanged). `sudo riku init` starts `riku-router-helper.service`, which listens on `/run/riku-router.sock` and writes the nginx and Caddy sites (`/etc/riku/nginx`, `/etc/riku/caddy`) and certificates (`/etc/riku/certs`) as root, so the deploy user writes no router config. acme.sh renewals call the new `riku router-renew <app>`. Sites listen on IPv6 when the host has it, and WebSocket upgrades are proxied by default. A deploy that finds a removed key in the app's ENV fails and names it: `RIKU_NGINX_CLOUDFLARE_ACL`, `RIKU_NGINX_CACHE_PATH`, `RIKU_NGINX_CACHE_PREFIXES`, `RIKU_NGINX_CACHE_SIZE`, `RIKU_NGINX_CACHE_TIME`, `RIKU_NGINX_CACHE_EXPIRY`, `RIKU_NGINX_CACHE_REDIRECTS`, `RIKU_NGINX_CACHE_ANY`, `RIKU_NGINX_CACHE_CONTROL`, `RIKU_NGINX_CATCH_ALL`, `RIKU_NGINX_CSP`, `RIKU_NGINX_INCLUDE_FILE`, `RIKU_NGINX_IPV4_ADDRESS`, `RIKU_NGINX_IPV6_ADDRESS`, `RIKU_DISABLE_IPV6` and `RIKU_NGINX_SOCKET`. The `nginx.include_content` filter point is gone with the include file. To upgrade a host, install the new binary, run `sudo riku init` again, run `riku doctor --fix` as the deploy user to redeploy apps and rewrite stale keys and hooks, then `sudo systemctl restart riku`.

- Removed `riku-plugin-container`: riku runs container apps itself, so `riku install-plugins container` and the plugin `pull-service` path for `RIKU_WATCH_SERVICES` are gone.
- Container names changed to `<app>.<service>.<n>` and volume names to `<app>.<key>`, and compose service and volume names may not contain `.`; redeploy container apps. Data in volumes under the old `<project>_<key>` names is not reused, and the compose `name` key and `COMPOSE_PROJECT_NAME` no longer affect any name.
- The browser dashboard's login backs off per client: the fifth failed password locks that client out for 30 s, doubling with each further failure up to 15 min, and a success clears it. The client is the last `X-Forwarded-For` entry, so keep the dashboard bound to loopback behind riku's router.
- Dashboard logout now signs out every session, and sessions last 12 hours instead of 7 days. Restarting the dashboard also signs everyone out.
- `RIKU_NGINX_INCLUDE_FILE` snippets are refused when a value is above its cap: `client_max_body_size` 1g (and not 0), every `*_timeout` 3600s, `proxy_buffer_size` 1m, `proxy_buffers` 64 of 1m, `proxy_busy_buffers_size` 2m, `gzip_comp_level` 1 to 9, at most 32 `add_header` lines of 4 KiB each. Sizes and times must be a number with an optional nginx unit; compound times such as `1m30s` are refused.
- Scaling a worker kind down now removes its surplus copies even with `RIKU_AUTO_RESTART=false`; the flag only decides whether the kept copies restart.
- Removed `riku update` (and `rk update`). It never updated anything; rerun the installer to upgrade.
- `RIKU_NGINX_STATIC_PATHS` now works for every nginx site: the alias locations and the ACME challenge location come from one shared template, so plain HTTP web apps, wsgi apps and `RIKU_NGINX_STATIC` sites get them too. The HTTPS only redirect now sits in `location /`, so the challenge is answered on plain HTTP.
- A deploy with no web facing Procfile kind, and every container app deploy, removes `RIKU_NGINX_PORTMAP`, `RIKU_NGINX_EXTERNAL_PORT` and `RIKU_NGINX_WSGI` from the app's ENV, so an app that dropped its `web:` line is served by the static or default template instead of a stale port map.
- Every deploy sets the app's ENV file to `0600` and its env directory to `0700`, container apps included.
- `riku logs <app> <service>` knows the app's Procfile kinds and compose services, and a known service with no output yet prints a note instead of "unknown service".
- `riku stop` on a Procfile app also removes its `workers-available` configs and the uWSGI sockets riku made for its copies.
- `sudo riku init` adds a `listen 80 default_server; return 444;` server to the riku nginx include when `nginx -T` shows no other port 80 default server, so an unknown Host is no longer answered by another app's site.
- `riku ps` RESTARTS counts a container restart as soon as the replacement runs, matching the supervisor's `restart=` log line.
- The supervisor runs its `docker events` child only while riku containers exist.

### Upgrade Notes: Worker and Socket File Names

- Worker configs, process ids and uWSGI copy sockets are now named
  `<app>@<kind>.<n>.toml`, `<app>@<kind>.<n>` and `<app>@<n>.sock`, so two
  apps can no longer share a file name; redeploy every app after upgrading.
  App names ending in `.git` are refused.
- uWSGI sockets moved from `~/.riku/nginx` to `/run/riku-sockets` (mode
  `2750`, group `riku-sockets`), and are `0660`. `sudo riku init` creates
  the group with the deploy user and the nginx worker user in it, the
  directory, and `/etc/tmpfiles.d/riku.conf` to recreate it at boot. nginx
  gets no access to the deploy user's home. Run `sudo riku init`, then
  redeploy wsgi, jwsgi, rwsgi and php apps; old sockets in `~/.riku/nginx`
  can be deleted.
- Caddy site files moved from `~/.riku/caddy` to `/etc/riku/caddy` (owned
  by the deploy user, group `caddy`, mode `2750`), and the Caddyfile imports
  `/etc/riku/caddy/*.caddy`. `sudo riku init` no longer adds `caddy` to the
  deploy user's group or opens the deploy home and `~/.riku` to that group.
  On a host set up before this, run `sudo riku init`, delete the old
  `import .../.riku/caddy/*.caddy` line from `/etc/caddy/Caddyfile`, take
  `caddy` out of the deploy group (`sudo gpasswd -d caddy riku`), run
  `chmod g-x ~ ~/.riku` as the deploy user, restart Caddy and redeploy each
  app.
- nginx no longer serves files from the deploy home. The ACME http-01
  webroot moved from `~/.riku/acme` to `/var/lib/riku/acme`, and acme.sh
  now runs with umask `027` so its challenge files are group readable. A
  deploy copies the `RIKU_NGINX_STATIC` document root and each
  `RIKU_NGINX_STATIC_PATHS` source into `/var/lib/riku/static/<app>`
  (swapped in through one rename, removed on destroy), and nginx's `root`
  and `alias` point there. Both directories are owned by the deploy user,
  group `riku-sockets`, mode `2750`, under a root-owned `0755`
  `/var/lib/riku`, and are in the unit's `ReadWritePaths`. Sources must
  resolve inside the app checkout, symlinks included, or the deploy fails;
  an absolute `RIKU_NGINX_DOCUMENT_ROOT` outside it is refused. Run
  `sudo riku init` and redeploy static apps. `~/.riku/acme` now only holds
  the per app links to acme.sh certificate directories. In `RikuPaths`,
  `acme_root` is now the webroot, the acme.sh install is `acme_sh_root`, and
  `acme_www` became `acme_cert_links`.
- Worker configs, `ENV` and `LIVE_ENV` files are written `0600`, app env
  directories `0700`, and an app's TLS key `0600`. A deploy rewrites the
  worker configs and `LIVE_ENV` and tightens the env directory; an existing
  `ENV` file stays as it is until riku next writes it.

### Router Plugin Protocol 2

The `configure` request sent to a router plugin now carries a `backends` list
(each a TCP host and port, or a unix socket path) and a `protocol` field, in
place of the single `upstream_port`. A router plugin must declare
`[router] protocol = 2` in its manifest. A plugin without that table, or with
another protocol number, is refused before any verb runs. `unconfigure` still
runs for any protocol. A plugin may set `graceful_reload = true` under
`[router]` to say its `reload` keeps connections alive. See
`PLUGIN_PROTOCOL.md` section 6.2.

`RIKU_DOMAINS` now goes through domain validation for plugin routers, as it
already did for Caddy. An invalid value fails a plugin router deploy.

A tcp backend's `host` is the app's `RIKU_BIND_ADDRESS` (default
`127.0.0.1`); a value that is not an IP literal fails the deploy. Caddy keeps
proxying to `127.0.0.1`.

### Upgrade Notes: Routing and nginx

- `RIKU_BIND_ADDRESS` must be an IP literal (`127.0.0.1`, `::1`, `10.0.0.5`)
  for port mapped apps (`RIKU_NGINX_PORTMAP`). `localhost` used to work and
  now fails the deploy.
- A `RIKU_UPSTREAM_PORT` or `PORT` that is set but not a port number now
  fails the deploy, naming the key. This holds for nginx port mapped apps,
  Caddy and router plugins; Caddy and router plugins used to fall back to
  port 8080.
- `RIKU_NGINX_SOCKET` holding whitespace or nginx syntax characters (`;`,
  `{`, `}` and the like) now fails the deploy, for every app type including
  static and wsgi apps.
- nginx sites now proxy through a named `upstream` block. The name is
  `riku_` followed by the lowercase hex of the app name's bytes (`myapp`
  becomes `riku_6d79617070`), so apps whose names differ only in case no
  longer collide. A hand written include that used `proxy_pass
  http://<app>_UPSTREAM` must switch to the new name.
- HTTPS only sites (`RIKU_HTTPS_ONLY`) on nginx never rendered before: the
  template failed and its parts did not pass `nginx -t`. They now render and
  pass, including for port mapped and wsgi apps, and wsgi apps pass a
  `unix:` socket to `uwsgi_pass`.
- After upgrading, run `sudo riku init` and redeploy apps.
- nginx now loads riku's sites through one root-owned file,
  `/etc/nginx/conf.d/riku.conf`, which includes `~/.riku/nginx/*.conf`.
  Riku no longer writes under `/etc/nginx` at deploy time.
- A `.conf` file in `~/.riku/nginx` that is no app's config refuses deploys,
  because the include would load it.
- An `nginx.conf`, `nginx.custom.conf`, `nginx.custom` or `.nginx.conf`
  shipped in the app directory is ignored with a warning, and the generated
  config is used.
- `RIKU_NGINX_INCLUDE_FILE` must name a file that resolves inside the app
  directory. Its text must fit a strict grammar: printable ASCII, space, tab
  and newline only (no CR), no backslash, no `{` or `}`, quotes and `#`
  only at the start of a word, and every statement ended by `;`. Every
  statement must use one of `add_header`, `expires`,
  `client_max_body_size`, `client_body_timeout`, `keepalive_timeout`,
  `proxy_connect_timeout`, `proxy_read_timeout`, `proxy_send_timeout`,
  `proxy_buffering`, `proxy_buffer_size`, `proxy_buffers`,
  `proxy_busy_buffers_size`, `gzip`, `gzip_types`, `gzip_min_length`,
  `gzip_comp_level` or `gzip_vary`. The fallback that passed the value to
  nginx as an `include` path is gone. An existing snippet that breaks these
  rules now fails the deploy, naming the file and the reason.
- Output of `nginx.include_content` filter plugins gets the same check. A
  filter chain that returns invalid text or a value that is not a string
  fails the deploy instead of being passed through.
- The ACME challenge config is written as `acme-challenge.conf.disabled`,
  so the include does not load it.
- Server side HTTPS requests go through the new `riku-http` crate: reqwest
  on rustls with the ring provider. aws-lc is no longer built.

### Container Apps

- An app with a compose file (or a `COMPOSE_FILE`), or a Dockerfile or
  Containerfile at the app root, now deploys as containers on its next push.
  To keep the runtime it used before, set `RIKU_RUNTIME` to that runtime
  (for example `riku config:set myapp RIKU_RUNTIME=node`) before pushing. An
  app that switches from Procfile workers keeps them until its containers
  are healthy and routed.
- Riku runs every compose service as its own containers, `<app>-<service>-<n>`
  on the network `riku-<app>`, without a compose tool. The supervisor adopts
  them on start, restarts exited copies per the compose `restart` policy,
  and writes their output to `~/.riku/logs/<app>/<service>.<n>.log`.
- Deploys, `riku restart` and image watch updates replace copies one at a
  time. The routed service without writable storage overlaps: a new copy
  must be healthy and routed before the old one leaves the router and gets
  its stop signal and grace period. Other services, storage services among
  them, stop first and are down until the new copy is healthy. Router
  reloads are confirmed before an old copy stops.
- Compose services scale through `SCALING` or `riku ps <app> --scale
  <service>=<copies>`, without a rebuild. A service with writable storage,
  or one publishing a port other than the routed HTTP one, stays at one
  copy.
- `riku restart <app> [service]` and `riku stop <app> [service]` act on one
  compose service. Naming a service of a Procfile app is refused.
- A compose port that names a host IP, such as `"127.0.0.1:8080:8080"` or a
  long form entry with `host_ip`, is refused. Ports bind 127.0.0.1 by
  default; the service label `riku.public: "true"` publishes them on all
  interfaces.
- Setting only one of `RIKU_GHCR_USERNAME` and `RIKU_GHCR_TOKEN` now fails
  the deploy. The old container plugin skipped the login in that case.
- A container app's Procfile `release` command now runs in a one-off
  container from the newly built image, with the app's env and on the app's
  network, instead of on the host. It runs after the images are built and
  pulled and before any copy is replaced. A nonzero exit stops the deploy.
- `riku destroy` of a container app stops and removes its containers and
  network and keeps its volumes. It refuses, removing nothing, when the
  container engine does not answer. `riku destroy <app> --force` removes
  riku's files and route anyway and warns that containers may still run.
- `riku logs <app>` follows every `*.log` of the app except `deploy.log`.
  `riku logs <app> <service>` follows that service's `<service>.<n>.log`
  files, and a name with no log files is an error listing the names that
  have them. A service name that is not a plain name, such as `../other` or
  `a*`, is refused.
- The service riku makes for a Dockerfile app now restarts with
  `unless-stopped`: the supervisor starts a crashed copy again, and a copy
  stopped with `riku stop` stays stopped. Before, it was never restarted.
  The supervisor also no longer starts a copy that `riku stop` removed while
  its restart waited for the app's lock.
- A compose file, or a folder on its path, may now be a symlink whose real
  target stays inside the app directory (for example
  `docker-compose.yml -> deploy/prod.yml`). A link that leaves the app is
  refused, naming the path. Bind mount sources, the build context, the
  dockerfile and `env_file` still refuse every symlink.
- Container apps need Docker 23 or newer, or an engine with a compatible
  CLI.

### Dashboard Authentication

The Next.js browser dashboard (`dashboard/`) gets its own login, independent
of the backend's `RIKU_DASHBOARD_TOKEN`: a single shared password, never
stored in plaintext (`RIKU_DASHBOARD_PASSWORD_HASH`, scrypt salt+hash,
generated via `nub run hash-password`), gating every route through a
signed session cookie (`proxy.ts`). Fully bypassed when the hash env
var is unset, so existing local/dev usage is unaffected. After login the
page only follows a same-origin `from` path, so a crafted login link cannot
send the user to another site. The stale `audit:dashboard` script is gone.

### Incident Alerting

The supervisor's crash/restart path now emits plugin lifecycle events:
`app.restarted` on every crash it recovers from, `app.failed` when a crash
exceeds `max_restarts` and the instance is permanently removed instead (the
more urgent of the two: nothing brings it back without manual
intervention). The bundled `plugins/riku-notify` event-subscriber plugin
posts an incident report to a generic webhook, Discord, Slack, and/or
Telegram, each independently configured.

### Plugin System: Versatility Expansion

Closed a set of gaps identified in an audit of the plugin system's
extensibility, all additive and opt-in (no effect on any existing plugin):

- **Subscriber priority**: `[events] priority = N` orders delivery when
  multiple plugins subscribe to the same event (lower runs first).
- **Install/uninstall lifecycle hooks**: `[lifecycle] install`/`uninstall`
  lets any plugin (not just addons) run setup/cleanup on
  `riku plugins install`/`remove`, always best-effort.
- **Per-plugin scratch directory**: every plugin invocation, across every
  seam, now gets `RIKU_PLUGIN_DATA_PATH`
  (`data_root/plugin-data/<plugin-name>/`).
- **Filters**: a new value-transform seam (`[filters]`, verb `on_filter`):
  a plugin receives a value and hands back a (possibly transformed) one,
  chained across multiple filters in priority order. Always degrades to
  passthrough on any failure (timeout, non-zero exit, malformed output),
  a broken filter can only become a no-op. Shipped first use:
  `nginx.include_content`, letting installed plugins augment the generated
  nginx config (wired into all 5 nginx templates) without a full router
  plugin replacing routing entirely.
- **Plugin-to-plugin custom events**: a plugin declaring `[events] emit =
  true` can fire its own event via `riku plugin-emit <name> --data
  '<json>'`. Hard-namespaced to `plugin.custom.*` (anything else, including
  an attempt to spoof a kernel event name, is rejected), with a
  kernel-stamped `source_plugin` field a subscriber can trust.
- **UI panels**: a plugin declaring `[ui] nav_label = "..."` gets a nav
  entry and its own page in the Next.js dashboard (`ui_panel` verb).
  Structured JSON only: never HTML/JS, so a plugin can extend the
  dashboard's UI without being able to inject markup into it.

Every item above shipped with real end-to-end tests against actual
installed plugin bundles and real dispatch code (no mocks). See
`PLUGIN_PROTOCOL.md` for the full updated contract.

### One-Line Installer, `riku quickstart`, and First-Deploy Output

Phase 0 of the adoption funnel (`41bb349`, 2026-06-24): `scripts/install.sh`
(`curl | sh`) detects OS/arch, downloads and checksum-verifies the
matching release binary, and installs it, printing the
`riku init`/`quickstart` next steps. `riku quickstart` scaffolds a
runnable, dependency-free sample app (python or node), git-inits it, and
prints the exact `git remote add`/`git push` lines needed to deploy.
`git push` now ends with a prominent `<app> deployed!` plus the live URL
(from `NGINX_SERVER_NAME`), or a hint to add a domain if none is set.

### Fixed

- Corrected several stale documentation claims found in the course of this
  work: the dashboard was documented as "read-only" (it can deploy,
  restart, and manage addons, never was read-only); the router plugin
  seam was documented as "planned" (it has been shipped for some time,
  as a host-level singleton); `dashboard/README.md` described a
  CSRF/Origin-check mechanism that didn't exist in the actual code.
- Bumped `crossbeam-epoch` to clear RUSTSEC-2026-0204.
- Worker config files are attributed to an app by the `[worker] app` field
  they record, not by an `{app}-*` file name prefix. `riku destroy shop`
  and `riku stop shop` no longer delete the worker configs of an app named
  `shop-api`, and `ps`, `apps`, `info`, `stats`, hot reload and cron jobs
  no longer count or read them as `shop`'s.

### Changed

- `crates/riku-supervisor/src/config/mod.rs`: collapsed 12 repeated
  `env.get(key).and_then(|v| v.parse().ok()).unwrap_or_else(default)`
  blocks into `parse_env_or!`/`parse_env_opt!` macros.
  `crates/riku-util/src/resource_limits/mod.rs`: similarly deduplicated
  via a small `env_u64()` helper.
- A `RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path())` test
  constructor, hand-copied into 12+ test modules across the workspace, is
  now a single shared `RikuPaths::for_tests()`.

## [3.0.0] - 2026-04-09

### Plugin-Based Runtime System

This release removes all hardcoded runtime logic from the core binary. Runtime
detection and building is now fully delegated to external plugins, making the binary
significantly lighter and allowing any language to be supported without a recompile.

**Binary size impact:** ~3,500 lines of runtime-specific code deleted from the core.
All 263 unit tests and 191 integration tests continue to pass.

### Breaking Changes

- **Runtime plugins must be installed separately.** Run `riku install-plugins` after
  upgrading to download the bundled plugins (node, python, ruby, go, rust-lang, java,
  clojure, container) to `~/.riku/plugins/`. Without plugins, deploys will fail with a
  clear error message.
- **`RUNTIME=<name>` ENV var** now pins which plugin handles an app, replacing the old
  implicit priority system. Example: `riku config set myapp RUNTIME=node`.

### Added

- `src/plugins/runtime.rs`: plugin discovery, detection, build dispatch, env and start command extraction
- `plugins/node`: bundled Node.js shell script plugin (detects `package.json`)
- `plugins/python`: bundled Python shell script plugin (detects `requirements.txt`, `pyproject.toml`)
- `plugins/ruby`: bundled Ruby shell script plugin (detects `Gemfile`)
- `plugins/go`: bundled Go shell script plugin (detects `go.mod`, `Godeps`, `.go` files)
- `plugins/rust-lang`: bundled Rust shell script plugin (detects `Cargo.toml` + `rust-toolchain.toml`)
- `crates/riku-plugin-java`: Rust binary plugin for Java (Maven/Gradle)
- `crates/riku-plugin-clojure`: Rust binary plugin for Clojure (Lein/deps.edn)
- `crates/riku-plugin-container`: Rust binary plugin for containers (Docker/Podman, auto-detected)
- `riku install-plugins` CLI command: downloads bundled plugins from GitHub
- `riku install-plugins --plugins <list>`: install specific plugins only
- Cargo workspace: root package + `crates/riku-plugin-*` sub-crates

### Changed

- `src/deploy/mod.rs`: replaced runtime dispatch with plugin-based orchestration
- `src/deploy/workers.rs`: `create_workers_generic` now accepts `start_cmd: Option<&str>` for plugin-provided fallback command
- `src/plugins/executor.rs`: `plugin_timeout` and `wait_with_timeout` made `pub(crate)` for use by runtime.rs
- Integration tests: all full-deploy tests now use lightweight mock plugins; no npm/pip/bundler required on the test host

### Removed

All 16 hardcoded runtime files from `src/deploy/`:
`python.rs`, `python_workers.rs`, `node.rs`, `node_workers.rs`, `ruby.rs`, `go.rs`,
`rust.rs`, `java.rs`, `clojure.rs`, `container.rs`, `container_workers.rs`,
`container_export.rs`, `identity.rs`, `runtime.rs`, `runtime_tests.rs`, `macros.rs`

---

## [2.2.0] - 2026-02-26

### Production Hardening Refactor

This release closes the remaining gaps between the self-audit findings and a
production-ready state. All 214 tests continue to pass; `cargo clippy -D warnings`
is clean with zero warnings in production code.

### Breaking Changes

- **`PIKU_AUTO_RESTART` renamed to `RIKU_AUTO_RESTART`**: update your `ENV` files.
  The old variable name was a residual from the Python Piku port and has now been
  fully removed. All runtimes (Python, Node, Ruby, Go, Java, Clojure, Rust,
  Container, Identity) and all documentation now use the correct `RIKU_AUTO_RESTART`.

### Security Fixes

- **`cargo audit --deny warnings` now blocks releases**, CI will fail on any known
  CVE in the dependency tree instead of silently reporting it (`ci.yml`).
- **Nginx security headers hardened**: `nginx_static.conf.tera` and
  `nginx_portmap.conf.tera` now include `Referrer-Policy` and `Permissions-Policy`
  headers (the HTTPS-only template already had `HSTS`; `nginx_common.conf.tera`
  already had the full set).
- **Systemd `ReadWritePaths` tilde expansion fixed**: `setup.rs` now writes the
  absolute path to `~/.riku` (resolved at runtime) instead of the literal `~/.riku`
  string, which is not expanded by systemd on all distributions.
- **Predictable `/tmp` test path removed**: `tests/deploy-smoke/test-all.sh` now uses
  `mktemp -d` instead of the PID-based `/tmp/riku-test-$$` path that was vulnerable
  to symlink attacks.

### Dependency Upgrades

- **`reqwest` upgraded from v0.11 to v0.12**: v0.11 is in maintenance-only mode;
  v0.12 brings `hyper` 1.x, `http` 1.x, and updated TLS dependencies.

### Code Quality

- **All `unwrap()` calls in production paths eliminated**, replaced with
  `unwrap_or_default()` (for infallible `SystemTime` operations) and
  `ok_or_else(|| anyhow!(...))` (for path operations in `setup.rs` and `apps.rs`).
- **Duplicate `create_identity_workers` removed**: the ~170-line copy in
  `deploy/mod.rs` was dead code shadowing the canonical implementation in
  `deploy/identity.rs`. Only the `identity.rs` version remains.
- **Dead code suppressions removed or resolved**:
  - `#[allow(dead_code)]` removed from `deploy_identity` and `create_identity_workers`
    in `identity.rs` (they were already being called).
  - `#[allow(dead_code)]` removed from `remove_nginx_config` and
    `generate_acme_nginx_config` in `nginx.rs`; both are now wired into callers
    (`cmd_destroy` uses `remove_nginx_config`; `cmd_init` calls
    `generate_acme_nginx_config` for the ACME bootstrap config).
  - `install_systemd_service` (system-wide, root) is now called from `cmd_init`
    when running as root, removing its dead-code status.
  - `install_nginx_default_config` and `num_cpus` (genuinely unused) removed entirely.
- **Clippy clean**: `cargo clippy -- -D warnings` passes with zero errors or
  warnings in production code. Fixed 8 `useless_format!` instances across deploy
  modules and 1 `io_other_error` in `supervisor/stats.rs`.
- **`CONTRIBUTING.md` clone URL corrected**: was pointing to `piku.git`, now
  correctly points to `riku.git`.
- **`Runtime::Identity` variant now constructed**, the `None` branch in `do_deploy`
  now calls `found_app(&Runtime::Identity.to_string())` before dispatching, making
  the variant active and removing the dead-code warning.

### Documentation

- All references to `PIKU_AUTO_RESTART` updated to `RIKU_AUTO_RESTART` in README,
  docs site (env.md, faq.md), examples/README.md, API.md, and test scripts.
- `API.md` reference to `PIKU_RAW_SOURCE_URL` updated to `RIKU_RAW_SOURCE_URL`.

---

## [1.0.0] - 2026-02-23

### First Stable Release

Riku 1.0.0 is the first stable release of the Rust port of Piku, providing Heroku-like git push deployments.

### ✨ New Features

#### AI Agent Interface
- SSH-based automation for AI agents (Claude, Cursor, Copilot, etc.)
- Permission scopes: `readonly`, `staging`, `production`
- JSON output mode for reliable AI parsing
- Confirmation flow for destructive operations
- Rate limiting per agent
- Audit logging of all AI actions
- Commands: `agent --intro`, `agent --schema`, `agent <command>`

#### Documentation
- Comprehensive mkdocs documentation site
- CLI reference with all commands
- Environment variables guide
- Runtime-specific deployment guides
- Nginx configuration documentation
- Process supervisor documentation
- Plugin system documentation
- AI Agents integration guide
- Systemd integration guide

#### Developer Experience
- Updated `.gitignore` with comprehensive Rust project ignores
- Fixed GitHub Actions workflow formatting
- Code formatting with `cargo fmt`
- Linting with `cargo clippy`

### Improvements

- Fixed repository URLs (piku → riku)
- Improved SSH key scope parsing for AI agents
- Wired real deploy/destroy/restart/stop functions to agent commands
- Enhanced error handling with structured JSON responses
- Added confirmation tokens for destructive operations

### Technical Changes

- All 77 integration tests passing
- Release build optimized with LTO
- Documentation builds with mkdocs-material theme
- GitHub Actions CI/CD pipeline configured

### Documentation Updates

- Moved SYSTEMD.md to mkdocs
- Fixed incorrect repository references
- Added AI Agent Interface section to README
- Updated installation instructions

---

## [0.1.0] - 2026-02-17

### Initial Pre-release

Initial Rust port of Piku with core functionality:

- Git push deployments
- Multi-language support (Python, Node.js, Ruby, Go, Java, Clojure, Rust)
- Custom Rust process supervisor
- Nginx configuration generation
- Plugin system
- Cron job support
- Environment variable management
- Scaling support

### Test Coverage

- 109 unit tests
- 77 integration tests
- 11 deployment tests
- Total: 197 tests

---

## Version History

| Version | Date | Description |
|---------|------|-------------|
| 1.0.0 | 2026-02-23 | First stable release with AI Agent Interface |
| 0.1.0 | 2026-02-17 | Initial pre-release |
