# Container services, scaling and rolling deploys

## Why

A compose app today runs as one supervised process, `docker compose -f <file> up`
(crates/riku-plugin-container/src/main.rs:189), shown as a single `web.1` row.
Its memory and CPU are the compose client's, read from `/proc/<pid>` of that one
process (crates/riku-supervisor/src/stats/resources.rs), not the containers'.
A crashed service is invisible while `compose up` keeps running, restart and
stop act on the whole stack, and scaling would start a second `compose up`
fighting the first. The owner runs real compose apps (a private compose app on
edge) and wants each service managed as well as a Procfile worker, plus
real scaling with no downtime on deploy.

## Goals

1. Riku works with whatever container engine the server has. No engine
   specific code. The dashboard shows the engine's name and version.
2. Each compose service is a first class unit: its own rows (one per copy),
   real state, memory, CPU, restart count, logs, restart and stop, in the CLI
   and the dashboard.
3. Stateless services scale to N copies on one machine. HTTP copies sit behind
   the server's router as N backends.
4. Deploys and restarts replace copies one at a time with a health check, so a
   scaled app never goes down.
5. The same rolling replacement for Procfile apps when scaled.
6. Existing compose apps migrate on their next deploy, data intact.

## Non goals

- Riku does not run containers itself. No OCI runtime, image store or
  container networking code in riku.
- Managing several servers from one riku (phase 3, separate design). This
  project must not block it: keep per server state on the server.
- Traefik support (later). The routing plan must be renderable by it without
  redesign.
- Multi host scheduling, autoscaling, database replication.

## Anti goals

- Corrupting data by running two copies of a storage service.
- Downtime during a deploy of a scaled app.
- A deploy that leaves no healthy copy when the new version fails.
- Code paths that only work with Docker.
- Silently ignoring a compose feature riku does not support.

## Container engine

- Riku drives engines through the Docker compatible CLI: `pull`, `build`,
  `run -d`, `stop -t`, `kill`, `rm`, `ps --format json`, `inspect`,
  `stats --no-stream --format json`, `logs -f`, `version`, `network`,
  `volume`. Docker, Podman, nerdctl and Finch all speak it. One driver, no
  per engine branches.
- Detection: a setting on the server names the engine binary; otherwise riku
  looks for known engine binaries on PATH in a fixed order. The known list is
  data, not code paths.
- On first use riku checks that the engine answers every command it needs,
  and refuses with the missing command named instead of half working.
- Name and version come from `<engine> version --format json`. Docker reports
  a product name (`Server.Platform.Name`, checked locally); Podman's JSON has
  versions but no product name (podman-version docs); nerdctl's fields are not
  documented. Show the reported name when present, else the binary name, plus
  the reported version.
- Engines may need different flags for the same idea (for example rootless
  Podman). Where that happens the driver probes capability, never the engine
  name.

## Compose files

- Riku reads the compose file itself (today it hands it to `compose up`), so
  each service becomes riku managed containers and works with engines that
  have no compose command.
- Version 1 supports: `services`, `image`, `build` (context, dockerfile, args),
  `command`, `entrypoint`, `environment`, `env_file`, `ports`, `volumes`
  (named and bind, `:ro`), `depends_on` (start order, and
  `condition: service_healthy`), `healthcheck`, `restart`, `stop_grace_period`,
  `stop_signal`, `labels`, `networks` (default network only), `working_dir`,
  `user`. Any other key is a deploy error naming the key and service. The
  existing COMPOSE_FILE support and `RIKU_GHCR_*` registry login stay.
- A single Dockerfile app is a compose app with one service named `app`.
- Containers are named `<app>-<service>-<n>`, carry labels identifying app,
  service and copy, and join one network per app so services reach each other
  by service name.
- Existing image watch and per service pull
  (crates/riku-supervisor/src/daemon/image_watch.rs,
  crates/riku-deploy/src/service_update.rs) keep working on the new model.

## Scaling

- Every service starts at 1 copy. Scale is set per service:
  `riku ps <app> --scale <service>=N`, and from the dashboard.
- Storage services cannot scale. A service that mounts any writable volume or
  bind mount is refused with a message saying why. Read only mounts do not
  count.
- A scaled service that publishes a non HTTP port is refused: routers only
  balance HTTP.
- HTTP services: each copy gets its own port on 127.0.0.1 chosen by riku. The
  router config lists every copy as a backend. Adding or removing a copy
  updates the router.
- Workers (no published port): copies run side by side, nothing routed.
- Procfile apps follow the same model: each web copy gets its own port and is
  a backend. Today the router has a single upstream per app (nginx
  `templates/nginx_https_only.conf.tera:1-3`, Caddy `upstream_port` in
  crates/riku-caddy/src/site.rs:32).

## Routing

- One routing plan per app: domains, HTTPS settings and a list of backends.
  The nginx generator, the Caddy site writer and router plugins
  (`Router::Plugin`) each render it. Traefik later renders the same plan.
- A router reload must not drop in flight requests (nginx reload and Caddy
  config reload are graceful; a plugin router must say whether it is).

## Rolling replacement

Used by deploy, redeploy and restart of any app with a running service.

1. Start the new copy beside the old ones, on its own port.
2. Wait for health: the service's compose `healthcheck` if it has one; else
   the service's health URL if configured (compose label, for example
   `riku.health: /healthz`, or an app ENV setting for Procfile apps) until it
   returns 2xx; else until its port accepts connections. Each wait has a time
   limit.
3. If the new copy never becomes healthy: remove it, stop the deploy with an
   error, leave the old copies serving untouched.
4. Drain one old copy: remove it from the router and reload, so no new
   requests reach it while in flight ones finish.
5. Send the service's stop signal (SIGTERM unless `stop_signal` says
   otherwise) and wait up to `stop_grace_period` (default 30 s). Force kill
   only after that.
6. Repeat until every copy runs the new version.

Riku cannot see inside a container to know it is idle. It relies on the
standard contract that a service finishes its current work on SIGTERM; the
docs say so. A service with one copy still briefly runs two during a deploy;
for a storage service (never scaled) the old copy stops first, then the new
one starts, with downtime the docs state plainly.

## Migration

- On the first deploy of an existing compose app with the new riku: riku
  stops the old `compose up` stack (containers and its network only, never
  volumes) and starts the services as riku managed containers.
- Named volumes keep their data: riku must reuse the exact volume names the
  compose project created (compose prefixes them with the project name), so
  the migration maps each volume to its existing name. A test pins this.
- The old supervised `compose up` worker config is removed.
- `riku doctor` reports an app still on the old model.

## Dashboard and CLI

- One row per copy: `<service>.<n>`, state from the engine (running,
  restarting, exited with code, unhealthy), memory and CPU from the engine's
  stats, restart count, container id in place of PID, image and tag.
- Per service: restart, stop, scale, logs (filterable in the existing logs
  drawer). Whole app: restart, stop, redeploy act on all services with rolling
  replacement where it applies.
- Header shows the engine name and version.
- `riku ps <app>` and `riku logs <app> <service>` show the same data.
- The state API gains the fields above; the redesigned page renders them.
  Existing fields keep their meaning for Procfile workers.

## Building on the laptop

`rk deploy <app> --build` (the `--build` flag name is free: no existing
command or flag uses it, checked in cli.rs and cmds.rs) builds on the
laptop and deploys the result through the same rolling replacement.

- Riku asks the server its OS and architecture first and builds for that
  platform. A laptop on arm64 building for an amd64 server is the common
  case; the image or app must come out for the server's platform.
- Container apps: the local engine builds every service image.
  - If a registry is configured for the app, the laptop pushes and the server
    pulls. Both sides skip layers they already have, so a code only change
    moves a few MB. This is the snappy path. Existing ghcr login settings are
    reused.
  - Without a registry, the image is streamed over the same ssh connection,
    zstd compressed, and loaded by the server's engine. The whole image moves
    every time; the docs point big images at the registry path.
- Procfile apps, and any server without a container engine: the laptop's
  engine runs the app's runtime plugin build inside a container matching the
  server's OS and architecture, so native dependencies (Python wheels with C
  code, Node native modules) come out right. The built app directory is then
  copied to the server with riku's own sync (below) and run by the
  supervisor as a normal Procfile app, skipping the server side build. The
  server needs only riku and ssh: no engine, no compilers, no rsync.
- The build must happen at the exact path the app will live at on the
  server, because some build outputs are not relocatable (a Python venv
  records its own path). A test per bundled runtime proves the copied output
  runs.

### Riku's own file sync

rsync is not assumed present (macOS ships openrsync, protocol 29, against
Debian's rsync 3.2.7 protocol 32; checked 2026-10-30). Riku is on both ends,
so it syncs itself over ssh:

1. `rk` lists every file of the built directory: path, size, content hash.
2. `riku` on the server compares and replies with the files it needs and the
   files to delete.
3. `rk` sends only those, as one zstd compressed tar stream on the same
   connection.
4. The server writes them into a fresh copy of the app directory and swaps it
   in only after the whole transfer is verified.

It is file level, not block level: a changed file moves whole. A compiled
binary that changes every build moves every deploy, compressed. The size
cost of this code in the binaries is measured when built.

## Ports and workers (owner decisions 2026-10-30)

1. Published ports bind to 127.0.0.1 by default, so a database's
   `5432:5432` is no longer reachable from outside the server. A service is
   exposed on all interfaces only when its compose file opts in with the label
   `riku.public: "true"`. The routed HTTP port is the published port of the
   service that has the app's domains; a `riku.http: "<port>"` label picks it
   when that is ambiguous, and an ambiguous service without the label is a
   deploy error. Nothing on a private compose app needs outside access other
   than through the router (owner).
2. Services that are not routed (workers, schedulers) are replaced stop
   first during a rolling deploy: the old copy stops, then the new one
   starts, with no overlap, so scheduled jobs never fire twice. Only routed
   web copies overlap. A few seconds' pause in background work during a
   deploy is accepted (owner).

## Builder questions settled by the designer

- Riku's supervisor owns container lifecycle: containers run with the
  engine's restart policy off, the supervisor restarts crashed copies, counts
  restarts and restarts copies after a reboot. This avoids two restarters
  racing a rolling replacement. The compose `restart` key maps to the
  supervisor's restart policy.
- Container logic moves into core crates (a container driver crate used by
  deploy, supervisor, dashboard and both binaries). The container runtime
  plugin's start command is retired for compose and single image apps.
- Compose files: top level `version`, `volumes`, `networks` and `x-*` keys,
  `${VAR}` and `${VAR:-default}` interpolation and the project `.env` file are
  supported. Unknown keys inside a service are errors.
- If migration's new start fails, riku restarts the old compose stack so the
  app is never left down.
- The router plugin protocol changes from one upstream port to a list of
  backends and gets a protocol version; riku has one owner, so installed
  router plugins are updated with it.
- WSGI socket Procfile apps get one socket per copy.
- Draining needs a confirmed reload, so riku reloads the router
  synchronously and checks the result before stopping a copy.

## Later phases

- Phase 3: one `rk` managing and deploying to several servers. Separate
  design, asked interactively.
- Traefik router.

## Decisions taken by the owner

- Engine agnostic through detection, no built in Docker or Podman code;
  engine name on the dashboard.
- Routers: nginx and Caddy now, Traefik later.
- Storage services never scale.
- Rolling replacement one copy at a time; a copy is stopped only after it is
  drained and given its grace period.
- Health: compose healthcheck, else optional health URL, else port open.
- Procfile apps get the same rolling replacement.
- Existing compose apps migrate on their next deploy.
- Build on the laptop with `rk deploy <app> --build`; registry when
  configured, else a compressed ssh stream; riku's own file sync instead of
  rsync; Procfile apps can be built locally for servers without an engine.
- This project comes after the `rk` client split
  (docs/superpowers/specs/2026-10-30-rk-client-design.md).

## Calls made by the designer

- Docker compatible CLI as the engine contract; capability probing over name
  checks.
- Supported compose key list; unknown keys are errors.
- Container naming, labels, one network per app.
- Writable mount means storage; non HTTP published port blocks scaling.
- 30 s default grace period; health wait limits.
- Health URL as a compose label `riku.health` and an ENV setting for Procfile
  apps.
- Storage services restart stop first, then start.

## Left to the builder

- Module layout per .claude/CLAUDE.md layering.
- Exact JSON shapes of engine output parsing per command, verified against
  each engine's docs and, where available, a real engine in Docker.
- Port allocation scheme for copies.
- Exact time limits for health waits.
