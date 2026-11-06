# Container integration (phase 3a, server side) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Riku runs every compose service as its own supervised containers, scales stateless services and Procfile web copies behind the router, replaces copies one at a time on deploy, and migrates existing compose apps with their data.

**Architecture:** The deploy process owns change: it builds or pulls images, starts copies through the phase 2 engine driver, waits for health, drains through a confirmed router reload and stops old copies. One rolling algorithm serves two runtimes (containers and Procfile workers) through a trait with exactly those two implementors. The supervisor owns watching: it adopts running containers by label, restarts dead ones through the `ControlActions` hook the binary injects, follows their logs into the existing log files and puts engine stats into `stats.json`, so the state API, `riku ps`, `riku logs` and the dashboard read one source.

**Tech Stack:** Rust workspace, `riku-container` (phase 2 driver), `anyhow`, `serde`/`serde_json`, `tracing`, systemd, nginx, Caddy.

**Spec:** `docs/superpowers/specs/2026-10-30-container-services-design.md`. Phase 2 ledger with rulings and deferred findings: `.superpowers/sdd/2026-10-30-container-foundations/progress.md` (this machine only).

**Not in this plan (phase 3b, own plan later):** `rk deploy --build`, the server platform query, image streaming, Procfile builds in a container, riku-sync wiring (ssh entry point, process deadline, `ReceiveLimits` setting, refusal logging, `.sync-new`/`.sync-old` hidden from `riku apps`).

## Global Constraints

- Follow `.claude/CLAUDE.md` in full. Rule 16: no `?` in new code or any function touched. Rule 12: files over about 200 lines split by concern; tests in sibling `_tests.rs` files. Rule 21: Read/Edit/Write for files, no `cat`/`sed -n`/heredocs/python, Bash only for git, cargo, docker, scripts.
- Owner style (dreygur): a function's helpers sit above it, constants at the top of the file, no section comments, enums over boolean parameters, test doubles named `mock_*` (the phase 2 `engine::fake` module keeps its name), no dashes in prose, comments, docs or commit messages, function names under 30 characters, braces always.
- Errors: `anyhow::Result` per `.claude/CLAUDE.md` (it wins over the style skill's `thiserror`).
- Confidential names: never write the owner's real server, organisation or project names anywhere. Neutral names only (`edge`, `edge.example`, `myapp`, `shop`). Every subagent is told this; the controller searches every diff before each commit.
- No AI attribution in commits. Commit with `git commit -F <file> --no-verify` when a hook would add one.
- No `Debug` print or `tracing` field may show env values, build args, label values or command arguments of a service (phase 2 deferred: `RawProject` and `ServiceDef` derive `Debug` with secrets). Log names and counts only.
- Engine access only through `riku-container`'s `Engine`. No `docker` literal outside `KNOWN_ENGINES`. Capability probing, never engine name checks.
- Owner decisions 2026-10-30 (this session): two plans (3a here, 3b later); Caddy reload through `caddy reload` on the admin endpoint, path unit dropped; nginx reload confirmed by watching `riku-nginx-reload.service`; supervisor adopts running containers on start.
- Spec values: stop grace default 30 s (`DEFAULT_STOP_GRACE`); published ports bind 127.0.0.1 unless `riku.public: "true"`; routed port from `riku.http` when ambiguous; non routed services replace stop first; storage services never scale; container restart policy at the engine is always off.
- Fixed by this plan (owner informed 2026-10-30): web scale above 1 routes to one copy only; restart counts reset to 0 in stats; router reload results ignored; custom `RIKU_NGINX_SOCKET` not where the worker listens; health `interval` never read. Each fix starts with a failing test.

## Decisions taken by the planner

- Copy slots: a new copy takes the lowest index not used by a live copy of that service, so a one copy service alternates `web.1` and `web.2` across deploys. No engine rename command is needed.
- Host ports: `pick_free_port()` binds `127.0.0.1:0`, reads the port, closes the socket. Stored on the container as label `riku.port`, and in the worker TOML env as `PORT` for Procfile copies. Router backends are always rebuilt from those two sources, never from `RIKU_UPSTREAM_PORT`.
- Restart policy travels as label `riku.restart` (`no`, `always`, `unless-stopped`, `on-failure`, `on-failure:N`), so the supervisor never parses compose.
- Health wait limits: `HEALTHCHECK_WAIT = 120 s` for a compose `healthcheck`, `PROBE_WAIT = 60 s` for a health URL or open port, polled every `PROBE_INTERVAL = 500 ms`, each HTTP probe with `PROBE_TIMEOUT = 2 s`.
- Reload confirmation limit `RELOAD_WAIT = 30 s`.
- Image tags for built services: `riku-<app>-<service>`. Running copies keep their image by id, so moving the tag never touches them.
- Single Dockerfile apps: the service `app` gets `PortBinding { host_ip: Loopback, host_port: None, container_port: 8000, protocol: Tcp }` and env `PORT=8000` (`SINGLE_IMAGE_PORT`), so apps that read `$PORT` keep working.
- Migration accepts a short outage: compose's containers stop before riku's start, because they share volumes and host ports. The docs say so.

## Review Focus

1. A deploy whose new copy never gets healthy: old copies keep serving and the router config is unchanged. Test in Task 6 (`failed_copy_leaves_old_serving`) and Task 7 (container end to end with a mock engine).
2. A router reload that fails or never confirms: no old copy is stopped. Test in Task 6 (`unconfirmed_reload_keeps_old_copy`).
3. The supervisor restarts riku while a deploy is replacing copies: the dead copy restart is skipped while the deploy lock is held, and adoption never stops a copy. Test in Task 9 (`busy_deploy_skips_restart`).
4. Migration of a compose app with named volumes and a declared project name: volumes reused by exact name, refused when the name belongs to another app or the volume lacks the compose project label, old stack restored on failure. Tests in Task 11.
5. Two apps whose names make the same container name (`a-b` service `c`, `a` service `b-c`): the second run is refused with a message naming the other app. Test in Task 4.

---

### Task 1: Confirmed router reload

**Files:**
- Create: `crates/riku-deploy/src/router/reload.rs`, `crates/riku-deploy/src/router/reload_tests.rs`
- Modify: `crates/riku-nginx/src/lib.rs:70-95` (`reload_nginx`), `crates/riku-nginx/src/template.rs:189-211` (drop the implicit reload), `crates/riku-deploy/src/router.rs`, `crates/riku-deploy/src/router/caddy.rs`, `crates/riku-cli/src/setup/router_units.rs` (no Caddy units), `crates/riku-cli/src/doctor/router_check.rs`

**Interfaces:**
- Produces: `pub fn reload(router: &Router, paths: &RikuPaths) -> Result<()>` in `router/reload.rs`, returning only after the router confirmed the new config, else an error naming the router and cause.
- Produces: `pub fn apply(app: &str, env: &HashMap<String, String>, paths: &RikuPaths, backends: &[Backend]) -> Result<()>` in `router.rs`: render, write, validate, `reload`. Replaces the private single backend `route_plan`. `configure` becomes `apply` with `current_backends` (Task 2).
- Produces: `pub fn reload_nginx() -> Result<()>` (was `-> bool`).

How each router confirms:
- nginx: read `ExecMainExitTimestampMonotonic` of `riku-nginx-reload.service` with `systemctl show -p ExecMainExitTimestampMonotonic,ExecMainStatus,LoadState --value`, write the config, then poll every `PROBE_INTERVAL` until the timestamp is newer and `ExecMainStatus` is `0`, within `RELOAD_WAIT`. When `LoadState` is `not-found` (no systemd units, as on a container test host), run `nginx -s reload` and require exit 0.
- Caddy: `caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile` (`SYSTEM_CADDYFILE`), exit 0 required, stderr in the error. Setup stops installing `riku-caddy.path` and `riku-caddy-reload.service`, and removes them when present.
- Plugin: the existing synchronous `reload` verb.

- [ ] **Step 1: Write failing tests** in `reload_tests.rs` with a mock `systemctl`/`caddy` on a temp `PATH` (scripts written by the test):
  - `nginx_reload_waits_for_newer_unit_run`: the mock reports an old timestamp twice, then a newer one with status 0; `reload` returns Ok.
  - `nginx_reload_fails_on_unit_error`: newer timestamp with status 1; the error contains `riku-nginx-reload.service`.
  - `nginx_reload_times_out`: the timestamp never changes; error contains `did not confirm` (test sets the wait through a `#[cfg(test)]` constant of 1 s).
  - `caddy_reload_reports_stderr`: mock `caddy` exits 1 printing `admin endpoint disabled`; the error contains that text.
  - In `crates/riku-nginx`: `reload_nginx_error_is_returned` (mock `nginx` exits 1).
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy router::reload` and `cargo test -q -p riku-nginx reload`. Expected: FAIL (functions missing).
- [ ] **Step 3: Implement** `reload`, `apply`, `reload_nginx() -> Result<()>`; `install_nginx_symlink` no longer reloads (callers reload once through `apply`); every former `reload_nginx()` caller (`template.rs:210`, `lib.rs:121`, `riku-supervisor/src/daemon/mod.rs:263`) handles the `Result` (supervisor logs a warning, the others propagate).
- [ ] **Step 4: Doctor**: `reload` writes its last error to `<riku_root>/router-reload.err` and removes the file on success. `router_check.rs` fails with that text when the file exists, and warns when old `riku-caddy.path` units remain.
- [ ] **Step 5: Run** `cargo test -q -p riku-deploy -p riku-nginx -p riku-cli` and `cargo clippy --workspace -q`. Expected: PASS, no warnings.
- [ ] **Step 6: Docs**: `docs/docs/routers.md` (or the router page that exists) says reloads are confirmed, Caddy needs its admin endpoint on (the default, `localhost:2019`), and any local user can reach that endpoint, which is Caddy's default and not changed by riku.
- [ ] **Step 7: Commit** `Confirm router reloads before returning`.

### Task 2: Every Procfile web copy is a backend

**Files:**
- Create: `crates/riku-deploy/src/router/backends.rs`, `backends_tests.rs`
- Modify: `crates/riku-deploy/src/workers/web_env.rs:36-195` (per copy socket, no `persist_route_env` port overwrite), `crates/riku-deploy/src/workers/pinned.rs`, `crates/riku-nginx/src/backends.rs:47-120`

**Interfaces:**
- Consumes: `apply` (Task 1).
- Produces: `pub fn current_backends(app: &str, env: &HashMap<String, String>, paths: &RikuPaths, engine: Option<&Engine>) -> Result<Vec<Backend>>`. Procfile apps: one backend per enabled `<app>-web-<n>.toml` (TCP from its `PORT` and `RIKU_BIND_ADDRESS`, or its socket for WSGI kinds). Container apps (engine `Some`): one `Backend::loopback(port)` per running copy of the routed service, from `ContainerInfo.host_port` (Task 3). Empty list is an error naming the app.
- Produces: `pub fn copy_socket(paths: &RikuPaths, app: &str, copy: CopyIndex) -> PathBuf` = `<nginx_root>/<app>.<n>.sock`.

- [ ] **Step 1: Failing tests:**
  - `scaled_web_lists_every_copy`: three web TOMLs with ports 5001..5003; `current_backends` returns all three in copy order. (Bug: today only the last copy is routed.)
  - `wsgi_copies_get_own_socket`: two wsgi copies give `unix:<nginx_root>/myapp.1.sock` and `.2.sock`.
  - `custom_socket_is_where_worker_listens`: `RIKU_NGINX_SOCKET=/srv/s.sock`, scale 1: the worker env `SOCKET`/`UWSGI_SOCKET` equals `/srv/s.sock`. (Bug 4.)
  - `custom_socket_refuses_scale`: same with web scale 2 is refused, message names `RIKU_NGINX_SOCKET`, like `check_pinned_port`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy backends`. Expected: FAIL.
- [ ] **Step 3: Implement**. `RIKU_UPSTREAM_PORT` is no longer written; `upstream_port_number` stays only for reading old ENV files in `riku doctor`. `spawn_app` calls `router::apply` with `current_backends`.
- [ ] **Step 4: Run** `cargo test -q --workspace` (nginx template tests from cf89ff3 must keep passing) and, with Docker available, render a scaled config and run `nginx -t` in `nginx:1.27`.
- [ ] **Step 5: Commit** `Route every web copy of a Procfile app`.

### Task 3: Engine driver additions

Opus review: this task builds engine argv.

**Files:**
- Modify: `crates/riku-container/src/types/names.rs` (labels), `engine/inspect.rs`, `engine/inspect_parse.rs`, `engine/resources.rs`, `engine/control.rs`, `engine/probe.rs`, `compose/service_steps.rs:41` (refuse the new labels in compose), engine tests and fixtures.

**Interfaces:**
- Produces labels: `labels::PORT = "riku.port"`, `labels::RESTART = "riku.restart"`. Compose files setting either are refused like `riku.app`.
- Produces: `ContainerState::Starting` (health status `starting`).
- Produces: `ContainerInfo` gains `app: Option<String>`, `host_port: Option<u16>` (from `riku.port`, junk is `None` plus a `tracing::warn` naming the container, not a batch error), `restart: Option<String>` (raw `riku.restart`), `image_id: String`.
- Produces `Engine` methods:
  - `owner_of(&self, name: &ContainerName) -> Result<Option<String>>`: `riku.app` of the container with that exact name, `None` when absent, `Some("")` when it has no label.
  - `list_project(&self, project: &str) -> Result<Vec<ContainerId>>`: all states, label `com.docker.compose.project=<project>`.
  - `volume_labels(&self, name: &str) -> Result<BTreeMap<String, String>>`.
  - `remove_network(&self, name: &str) -> Result<()>`: absent network is Ok.
  - `image_id(&self, image: &str) -> Result<Option<String>>`.
  - `login(&self, registry: &str, user: &str, token: &str) -> Result<()>`: token on stdin only, never argv.
  - `REQUIRED_COMMANDS` gains `login`.

- [ ] **Step 1: Failing tests** against the existing `fake` engine and captured fixtures: one per method, plus `starting_health_is_starting`, `junk_port_label_is_none`, `login_token_never_in_argv` (assert the recorded argv), `compose_refuses_riku_port_label`, `project_filter_is_exact` (`list_project("shop")` argv uses `label=com.docker.compose.project=shop`).
- [ ] **Step 2: Run** `cargo test -q -p riku-container`. Expected: FAIL.
- [ ] **Step 3: Implement.** Project and volume names go through the same engine safe check as `ContainerName` before reaching argv.
- [ ] **Step 4: Run** `cargo test -q -p riku-container`; with Docker, check `owner_of`, `list_project`, `volume_labels` against a real engine and store the captured JSON as fixtures with stock values (no host detail, per the A2/A4 rulings).
- [ ] **Step 5: Commit** `Add engine calls needed to run and migrate apps`.

### Task 4: Start one container copy

Opus review: names, mounts and env cross a trust boundary.

**Files:**
- Create: `crates/riku-deploy/src/containers/mod.rs`, `spec.rs`, `start.rs`, `images.rs`, `ports.rs`, sibling `_tests.rs` files.
- Modify: `crates/riku-deploy/Cargo.toml` (add `riku-container`), `crates/riku-container/src/compose/paths.rs` (make the bind source symlink walk `pub fn check_bind_source(app_path: &Path, source: &Path) -> Result<()>`).

**Interfaces:**
- Consumes: Task 3 engine methods and labels.
- Produces:
  - `pub struct AppName(String)` with `TryFrom<&str>` through `validate_app_name` plus `check_app_name`. Every function here takes `&AppName`.
  - `pub fn pick_free_port() -> Result<u16>` in `ports.rs`.
  - `pub fn copy_spec(app: &AppName, project: &ComposeProject, service: &ServiceDef, copy: CopyIndex, host_port: Option<u16>, env: &BTreeMap<String, String>) -> Result<RunSpec>`. Sets labels `riku.app`, `riku.service`, `riku.copy`, `riku.port` (when routed), `riku.restart`; network `NetworkName::for_app`; alias the service name; the routed binding's host port replaced by `host_port`; mounts from `project.engine_mounts`.
  - `pub fn prepare_images(engine: &Engine, app: &AppName, project: &ComposeProject, env: &BTreeMap<String, String>) -> Result<()>` in `images.rs`: `login` when `RIKU_GHCR_USERNAME`/`RIKU_GHCR_TOKEN` are set (registry `ghcr.io`), `pull` for `image` services, `build` tagged `riku-<app>-<service>` for `build` services.
  - `pub fn start_copy(engine: &Engine, spec: &RunSpec, app_path: &Path, scratch: &Path) -> Result<ContainerId>` in `start.rs`: refuses when `owner_of(spec.name)` is another app (message names both apps), walks every bind source again with `check_bind_source`, `ensure_network`, `run`.

- [ ] **Step 1: Failing tests:**
  - `name_owned_by_other_app_is_refused` (Review Focus 5): fake engine reports `a-b-c-1` owned by `a`; starting app `a-b` service `c` copy 1 fails, error contains `a` and `a-b`.
  - `bind_through_new_symlink_is_refused`: project parsed, then `data` replaced by a symlink to `/`; `start_copy` fails naming `data`.
  - `routed_copy_gets_its_own_port`: `copy_spec` with host port 5123 has one binding `127.0.0.1:5123 -> 80` and label `riku.port=5123`.
  - `restart_label_maps_policy`: `on-failure` with 3 retries gives `riku.restart=on-failure:3`.
  - `token_only_reaches_login`: `prepare_images` with a ghcr token; recorded argv never contains it.
  - `pick_free_port` returns a port that can be bound right after.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy containers`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q -p riku-deploy -p riku-container`. Expected: PASS.
- [ ] **Step 5: Commit** `Start one riku managed container copy`.

### Task 5: Wait for health

**Files:**
- Create: `crates/riku-deploy/src/rolling/health.rs`, `health_tests.rs`

**Interfaces:**
- Produces: `pub enum HealthTarget { Engine(ContainerId), Url { port: u16, path: String }, Port(u16) }`.
- Produces: `pub fn health_target(service: &ServiceDef, id: &ContainerId, host_port: Option<u16>) -> Option<HealthTarget>`: compose `healthcheck` (not `Disabled`) gives `Engine`; else `riku.health` label with a routed port gives `Url`; else a routed port gives `Port`; a worker with neither gives `None` (started is enough).
- Produces: `pub fn procfile_target(env: &HashMap<String, String>, port: u16) -> HealthTarget`: `RIKU_HEALTH_CHECK_URL` gives `Url`, else `Port`.
- Produces: `pub fn wait_healthy(target: &HealthTarget, engine: Option<&Engine>) -> Result<()>` using `HEALTHCHECK_WAIT`, `PROBE_WAIT`, `PROBE_INTERVAL`, `PROBE_TIMEOUT`. `Engine` passes on `Running`, fails at once on `Unhealthy` or `Exited`; `Url` needs a 2xx; `Port` needs a TCP connect.

- [ ] **Step 1: Failing tests:** `url_needs_2xx` (local listener answers 503 then 200), `exited_container_fails_fast` (fake engine, error names the exit code), `closed_port_times_out` (test limit 1 s), `health_label_beats_port`, `disabled_healthcheck_falls_back_to_port`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy rolling::health`. Expected: FAIL.
- [ ] **Step 3: Implement** with the blocking `reqwest` client already in the workspace.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Wait for a new copy to become healthy`.

### Task 6: Rolling replacement

**Files:**
- Create: `crates/riku-deploy/src/rolling/mod.rs`, `replace.rs`, `replace_tests.rs`

**Interfaces:**
- Consumes: `HealthTarget`, `wait_healthy` (Task 5); router `apply` (Task 1).
- Produces:
  ```rust
  pub trait CopyRuntime {
      fn live(&self) -> Result<Vec<LiveCopy>>;
      fn start(&mut self, copy: CopyIndex) -> Result<LiveCopy>;
      fn wait_healthy(&self, copy: &LiveCopy) -> Result<()>;
      fn stop(&mut self, copy: &LiveCopy) -> Result<()>;
      fn discard(&mut self, copy: &LiveCopy);
  }
  pub struct LiveCopy { pub copy: CopyIndex, pub backend: Option<Backend>, pub handle: CopyHandle }
  pub enum CopyHandle { Container(ContainerId), Process(String) }
  pub trait RouteSink { fn publish(&mut self, backends: &[Backend]) -> Result<()>; }
  pub fn replace_all(runtime: &mut impl CopyRuntime, routes: &mut impl RouteSink, order: ReplaceOrder, want: u32) -> Result<()>;
  ```
  Implementors: `ContainerRuntime` (Task 7), `ProcessRuntime` (Task 8) and the test mocks. `RouteSink` for production wraps `router::apply`; services without a routed port use a sink that does nothing.
- Algorithm for `Overlap`, per old copy: start at the lowest free index, wait healthy (on failure `discard` it and return the error, routes untouched), publish backends with the new copy added, publish backends without one old copy (the drain; error stops here with the old copy still running), stop the old copy (signal and grace live in the runtime). For `StopFirst`: stop old, start new, wait healthy; on failure the error says the service is down. Then grow or shrink to `want`: extra copies are started and added, surplus copies drained and stopped, highest index first.

- [ ] **Step 1: Failing tests** with `mock_runtime` and `mock_routes` that record calls:
  - `failed_copy_leaves_old_serving` (Review Focus 1): new copy unhealthy; recorded calls end with `discard`; no `publish`, no `stop`.
  - `unconfirmed_reload_keeps_old_copy` (Review Focus 2): the drain `publish` fails; no `stop` recorded.
  - `overlap_replaces_one_at_a_time`: 3 copies; the call log alternates start, healthy, publish, publish, stop three times, and at every publish at least 3 backends are listed until the end.
  - `stop_first_never_overlaps`: worker service; each `start` comes after the matching `stop`.
  - `scale_down_drains_before_stop`: want 1 from 3; each surplus copy is published away before its `stop`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy rolling::replace`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests 20 times with `--test-threads 1` and default threads. Expected: PASS every time.
- [ ] **Step 5: Commit** `Replace copies one at a time`.

### Task 7: Deploy container apps through riku

**Files:**
- Create: `crates/riku-deploy/src/containers/runtime.rs` (`ContainerRuntime`), `deploy.rs`, `control.rs`, `_tests.rs` files.
- Modify: `crates/riku-deploy/src/lib.rs:55-260` (branch before runtime plugin detection; move steps out so the file drops under 200 lines), `crates/riku-plugin-container/src/main.rs:192-210` (`start` for compose and single image apps exits 1 with `riku runs container apps itself; redeploy`), `crates/riku-cli/src/apps/process/ps_scale.rs`, `crates/riku-cli/src/apps/control.rs`, `crates/riku-args/src/cli.rs:196-209`.

**Interfaces:**
- Consumes: Tasks 2 to 6.
- Produces:
  - `pub fn is_container_app(app_path: &Path, env: &HashMap<String, String>) -> Result<bool>`: compose file found, or `Dockerfile`/`Containerfile` at the root, and `RIKU_RUNTIME` unset or `container`.
  - `pub fn deploy_containers(app: &AppName, paths: &RikuPaths, env: &HashMap<String, String>) -> Result<()>`: `read_project` (or `single_image_project` with `SINGLE_IMAGE_PORT`), `check_scale` per service against `SCALING`, `prepare_images`, then services in start order: `replace_all` with `replace_order`, waiting for `DependsCondition::Healthy` dependencies to be healthy before their dependents start. Hooks, release and post deploy run as for Procfile apps.
  - `pub fn scale_service(app: &AppName, paths: &RikuPaths, service: &ServiceName, copies: u32) -> Result<()>`: no rebuild, grows or shrinks through `replace_all` with the current image.
  - `pub fn restart_service(app, paths, service: Option<&ServiceName>) -> Result<()>` and `pub fn stop_service(app, paths, service: Option<&ServiceName>) -> Result<()>`.
  - `pub fn destroy_containers(app: &AppName, paths: &RikuPaths) -> Result<()>`, called by `riku destroy` for container apps: stops and removes every copy and the app network, keeps volumes, and says so in its output.
  - `pub fn restart_copy(app: &AppName, paths: &RikuPaths, service: &ServiceName, copy: CopyIndex) -> Result<RestartOutcome>` with `pub enum RestartOutcome { Restarted, Busy }`: takes the deploy lock non blocking, `Busy` when held; removes the dead container and starts the same slot.
  - CLI: `riku restart <app> [service]` and `riku stop <app> [service]` (optional positional `service`, refused for Procfile apps with a message); `riku ps <app> --scale svc=N` calls `scale_service` for container apps.

- [ ] **Step 1: Failing tests** (fake engine, temp riku root, mock router sink):
  - `deploy_starts_services_in_order`: `db` then `web` (web `depends_on: db: condition: service_healthy`); `web` starts only after `db` reports `Running`.
  - `redeploy_keeps_old_copy_on_failure`: second deploy's new web copy exits; old container still listed, router file unchanged.
  - `scale_refuses_storage_service`: `ps --scale db=2` with a writable volume fails naming the volume.
  - `restart_copy_reports_busy`: deploy lock held; returns `Busy`, no engine call.
  - `plugin_start_refuses_compose`: the container plugin's `start` exits 1 with the message.
  - `destroy_keeps_volumes`: recorded argv removes containers and the network, never a volume.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy containers` and `cargo test -q -p riku-plugin-container`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; with Docker, deploy a two service compose app (`web` on `nginx:alpine`, `worker` on `busybox`) on a local riku root and confirm `web.1` serves, then redeploy and confirm no failed request in a `curl` loop during the replacement.
- [ ] **Step 5: Commit** `Deploy compose and Dockerfile apps as riku containers`.

### Task 8: Rolling replacement for Procfile web copies

**Files:**
- Create: `crates/riku-deploy/src/workers/process_runtime.rs`, `process_runtime_tests.rs`
- Modify: `crates/riku-deploy/src/workers.rs:65-203` (web kinds go through `replace_all`; other kinds keep the current rewrite, which is stop first already)

**Interfaces:**
- Consumes: `CopyRuntime`, `replace_all` (Task 6), `procfile_target` (Task 5), `current_backends` (Task 2).
- Produces: `ProcessRuntime` implementing `CopyRuntime`: `start` writes `<app>-web-<n>.toml` for the free slot (supervisor spawns it on the file event), `stop` removes the enabled symlink and waits until the supervisor's `stats.json` shows the process gone or `grace_period` plus 5 s passed, `live` reads the enabled TOMLs.

- [ ] **Step 1: Failing tests:** `web_copy_overlaps_on_deploy` (temp root, a mock supervisor thread that reacts to file events by opening the copy's port): during the deploy two web TOMLs exist at once and the router never has zero backends; `pinned_port_keeps_stop_first` (pinned `PORT` cannot overlap, so web uses `StopFirst` and the commit body says why).
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy process_runtime`. Expected: FAIL.
- [ ] **Step 3: Implement.** The supervisor canary path (`deploy_generation`) stays for `restart --hot`; deploys no longer modify an existing web TOML, so it is not triggered by them.
- [ ] **Step 4: Run** `cargo test -q --workspace` and the deploy smoke test `./tests/deploy-smoke/quick-test.sh myapp node` with `web=2`.
- [ ] **Step 5: Commit** `Replace Procfile web copies one at a time`.

### Task 9: Supervisor watches containers

**Files:**
- Create: `crates/riku-supervisor/src/containers/mod.rs`, `watch.rs`, `restart_policy.rs`, `log_follow.rs`, `stats.rs`, sibling `_tests.rs`
- Modify: `crates/riku-supervisor/Cargo.toml` (add `riku-container`), `daemon/mod.rs:299` (call the watch from periodic maintenance every 5 s; split `daemon/mod.rs` by concern while there, it is 411 lines), `health/actions.rs:19-31` and its binary implementor (`crates/riku-cli/src/control_actions.rs`), `stats/types.rs:8`.

**Interfaces:**
- Consumes: `restart_copy` and `RestartOutcome` (Task 7) through `ControlActions::restart_copy(&self, paths: &RikuPaths, app: &str, service: &str, copy: u32) -> Result<RestartOutcome>`; `RestartOutcome` moves to `riku-supervisor` (`health/actions.rs`) so the supervisor does not depend on riku-deploy, and riku-deploy re-exports it.
- Produces: `pub enum RestartPolicyLabel { No, Always, UnlessStopped, OnFailure(Option<u32>) }` with `TryFrom<&str>`; `fn should_restart(policy, exit_code: i32, restarts: u32) -> bool`.
- Produces: `ProcessStats.container: Option<ContainerFacts>` with `pub struct ContainerFacts { pub id: String, pub image: String, pub state: String, pub exit_code: Option<i32>, pub cpu_percent: f64 }`; rows keyed `<app>-<service>-<n>`, `kind` = service, `ordinal` = copy, `pid` = None, `memory_bytes` from engine stats.
- Behaviour: on start no copy is stopped (adoption) and one listing finds every riku container; after that the supervisor follows `<engine> events` (container start, die, health status) through one long running process, so it reacts to a crash without polling, and polls engine stats every 30 s (`CONTAINER_STATS_INTERVAL`) for memory and CPU. Engines without `events` (capability probe) fall back to a listing every 30 s. It starts a log follower per running copy (`logs_command(follow, tail Some(0))` appending to `{log_root}/{app}/{service}.{n}.log`; tail 0 on adoption so lines are not repeated), restarts exited copies per policy with the existing backoff `min(60, 2^min(n,6))` s, counts restarts in memory per container name. Owner decision 2026-11-01: restart counts mean restarts since the last deploy, so a new container id (containers) or a rewritten worker TOML (Procfile copies) starts at 0.

- [ ] **Step 1: Failing tests:** `adoption_never_stops_copies` (fake engine with two running copies, a fresh supervisor tick records no stop/kill/remove), `busy_deploy_skips_restart` (Review Focus 3: `restart_copy` returns `Busy`, restart count unchanged, retried next tick), `on_failure_respects_limit`, `exit_zero_not_restarted_on_failure`, `stats_rows_carry_engine_state`, `die_event_triggers_restart` (mock engine streams one die event; restart happens without a listing tick), `no_events_falls_back_to_polling`, `deploy_resets_restart_count` (new container id or rewritten TOML starts at 0).
- [ ] **Step 2: Run** `cargo test -q -p riku-supervisor containers`. Expected: FAIL.
- [ ] **Step 3: Implement.** Engine errors in a tick are logged and the tick continues with other apps.
- [ ] **Step 4: Run** `cargo test -q --workspace`, 20 runs of the new tests in both thread modes.
- [ ] **Step 5: Commit** `Watch, restart and log riku containers`.

### Task 10: Supervisor fixes found while planning

**Files:**
- Modify: `crates/riku-supervisor/src/stats/manager.rs:31,83`, `crates/riku-supervisor/src/process/health_check.rs:113-205`, sibling tests.

- [ ] **Step 1: Failing tests:** `restart_count_survives_respawn` (mark restarting, register again; `restart_count` is 1, not 0), `health_check_honours_interval` (interval 30 s; two ticks 1 s apart run one probe).
- [ ] **Step 2: Run** `cargo test -q -p riku-supervisor restart_count health_check_honours`. Expected: FAIL.
- [ ] **Step 3: Fix:** `register_process` keeps `restart_count` and `last_restart_at` of an existing entry; health checks record `last_checked: Instant` per process and skip until `interval` passed.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Keep restart counts and honour health intervals`.

### Task 11: Migrate compose apps

Opus review: volume adoption is the data safety boundary.

**Files:**
- Create: `crates/riku-deploy/src/containers/migrate.rs`, `migrate_tests.rs`, `crates/riku-cli/src/doctor/container_check.rs`
- Modify: `crates/riku-deploy/src/containers/deploy.rs` (call migration first), `crates/riku-cli/src/doctor/mod.rs:123`

**Interfaces:**
- Consumes: `list_project`, `volume_labels`, `remove_network` (Task 3), `deploy_containers` (Task 7).
- Produces: `pub fn is_legacy_worker(config: &WorkerConfig) -> bool`: the command is one of the two shapes the container plugin printed (`<compose> -f <file> up`, `<rt> run --rm -p $PORT:$PORT riku-<app>`).
- Produces: `pub fn confirm_project_name(project: &mut ComposeProject, app: &AppName, paths: &RikuPaths, engine: &Engine) -> Result<()>`: for a `Declared` name, refuse when the normalised name equals another riku app's normalised name (`Shop`, `sh.op`, `shop` all compare as `shop`), or when any existing volume `<project>_<key>` lacks `com.docker.compose.project=<project>`; only then `confirm_declared_name`.
- Produces: `pub fn migrate(app: &AppName, paths: &RikuPaths, project: &mut ComposeProject, engine: &Engine) -> Result<()>`: move the legacy worker TOMLs to `<name>.toml.migrating`, wait for the supervisor to stop them, stop and remove containers from `list_project` (grace per service), remove network `<project>_default`, never a volume, then `deploy_containers`. On failure restore the TOMLs (old stack comes back) and return the error. On success delete them.
- Doctor: `container_check` reports engine name and version or why it is missing, and each app still on a legacy worker.

- [ ] **Step 1: Failing tests:**
  - `migration_reuses_volume_names` (spec pin): project `myapp`, volume key `data`; the new `db` copy mounts `myapp_data`.
  - `declared_name_of_other_app_refused`: app `myapp` with compose `name: Shop` while app `shop` exists.
  - `unlabelled_volume_refused`: `shop_data` exists without the compose label.
  - `failed_start_restores_old_stack`: new `web` exits; `.toml.migrating` files are renamed back and no volume removal was recorded.
  - `migration_never_removes_volumes`: the recorded argv has no `volume rm` in any path.
  - `doctor_reports_legacy_app`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy migrate` and `cargo test -q -p riku-cli container_check`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; with Docker, migrate a compose app started by the old plugin (`compose up` with a named volume holding a file) and confirm the file is still there after migration.
- [ ] **Step 5: Docs:** `docs/docs/containers.md` (or the existing compose page) states the migration outage and what is kept.
- [ ] **Step 6: Commit** `Migrate compose apps to riku containers`.

### Task 12: Image watch on the new model

**Files:**
- Modify: `crates/riku-deploy/src/service_update.rs`, `crates/riku-supervisor/src/daemon/image_watch.rs` (unchanged trigger), tests.

**Interfaces:**
- Consumes: `image_id` (Task 3), `replace_all` (Task 6).
- Produces: `pull_service` for container apps pulls the service image and runs `replace_all` for that service only when `image_id` differs from the running copies' `image_id`; legacy apps keep the plugin path until migrated.

- [ ] **Step 1: Failing tests:** `same_digest_does_nothing`, `new_digest_replaces_service`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy service_update`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Replace a watched service only when its image changed`.

### Task 13: State API fields

**Files:**
- Modify: `crates/riku-cli/src/apps/dump_state.rs:55-208` (split: types and building), tests.

**Interfaces:**
- Consumes: `ContainerFacts` (Task 9).
- Produces: `StateDump.engine: Option<EngineInfo { name, version }>` (from `Engine::open` on the configured or detected engine, `None` when absent); `WorkerStateEntry` gains `unit: UnitKind` (`process` | `container`), `container_id: Option<String>` (first 12 chars), `image: Option<String>`, `state_detail: Option<String>` (`exited 137`, `unhealthy`, `restarting`), `cpu_percent: Option<f64>`. Existing fields keep their meaning for Procfile workers.

- [ ] **Step 1: Failing tests:** `container_row_has_engine_fields`, `process_row_unchanged` (exact JSON for a Procfile worker equals today's plus `unit: "process"` and nulls), `engine_missing_is_null`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli dump_state`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Report container rows and engine in the state API`.

Tasks 14 and 15 run in parallel after Task 13.

### Task 14: Dashboard rows and service actions

**Files:**
- Modify: `crates/riku-dashboard/src/index.html` (`createCard` 716, `workerRow` 944, header), `crates/riku-dashboard/src/mutations.rs:20-60`, tests in `crates/riku-dashboard`.

**Interfaces:**
- Consumes: Task 13 fields; `restart_service`, `stop_service`, `scale_service` (Task 7).
- Produces routes: `POST /api/apps/{app}/services/{service}/restart`, `.../stop`, `.../scale` (body `{"copies": n}`), all behind `authorize_mutation`; service names through `ServiceName::try_from` before use.
- Page: header shows `engine.name engine.version`; container rows show container id where PID was, image, state detail, CPU %; one action group per service (restart, stop, scale stepper, logs opening the drawer filtered to that service). The `memory_limit_bytes` read the API never sends is left as is (not this task).

- [ ] **Step 1: Failing tests:** route tests `service_restart_needs_token`, `bad_service_name_is_400`, `scale_calls_scale_service`.
- [ ] **Step 2: Run** `cargo test -q -p riku-dashboard`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests; open the dashboard against the Task 7 Docker setup and check the rows and actions in a browser.
- [ ] **Step 5: Commit** `Show container copies and service actions in the dashboard`.

### Task 15: CLI rows and logs

**Files:**
- Modify: `crates/riku-cli/src/apps/process/ps_show.rs`, `ps_all.rs` (202 lines, split), `crates/riku-cli/src/apps/logs.rs`, tests.

**Interfaces:**
- Consumes: Task 13 fields.
- Produces: `riku ps <app>` columns for container rows: `service.n`, state detail, memory, CPU %, restarts, container id, image. `riku logs <app> <service>` works through the existing `{service}.{n}.log` files the supervisor writes (Task 9); an unknown service name lists the known ones.

- [ ] **Step 1: Failing tests:** `ps_shows_container_columns`, `logs_unknown_service_lists_known`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli ps_ logs_`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `Show container copies in riku ps and logs`.

### Task 16: Docs, changelog, roadmap

**Files:**
- Modify: `docs/docs/` pages for containers, scaling, routers, CLI; `CHANGELOG.md`; `docs/superpowers/ROADMAP.md`.

- [ ] **Step 1:** Document scaling rules, rolling replacement and the SIGTERM contract, storage service downtime, the `riku.*` labels, migration, `restart`/`stop` per service, confirmed reloads and the Caddy admin endpoint.
- [ ] **Step 2: Run** `NO_MKDOCS_2_WARNING=1 uvx --with mkdocs-material --with mkdocs-minify-plugin mkdocs build --strict -d <scratch>/site`. Expected: no warnings.
- [ ] **Step 3:** ROADMAP: phase 3a status lines with commits, phase 3b entry naming its duties (moved from the phase 3 list).
- [ ] **Step 4: Commit** `Document container integration`.

### Task 17: Server settings file and engine time limits

Owner decisions 2026-11-01.

**Files:**
- Create: `crates/riku-config/src/settings.rs`, `settings_tests.rs`
- Modify: `crates/riku-config/src/lib.rs` (`settings_file` path on `RikuPaths`), `crates/riku-container/src/engine/bounded.rs` and the engine call sites, `crates/riku-cli/src/doctor/` (report an unreadable settings file), docs.

**Interfaces:**
- Produces: `pub struct ServerSettings { pub engine: EngineSettings }`, `pub struct EngineSettings { pub call_timeout: Duration, pub stop_extra: Duration, pub pull_timeout: Duration, pub build_timeout: Duration }` with defaults 30 s, 30 s, 30 min, 30 min; `pub fn load(paths: &RikuPaths) -> Result<ServerSettings>` reading `~/.riku/settings.toml` (`[engine]` table, keys `call_timeout`, `stop_extra`, `pull_timeout`, `build_timeout` as humantime strings such as `"30s"`, `"30m"`). Missing file or key gives the default; an unknown key or a bad value is an error naming the key and the file.
- `Engine` takes its limits from `EngineSettings` (set when opened): `run`, `remove`, `stats`, `list`, `inspect`, `owner_of`, `volume_labels`, `remove_network`, `image_id`, `login` use `call_timeout`; `stop` uses the grace period plus `stop_extra`; `pull` and `build` use their own limits and stream output to the caller's log as they run.
- ROUTER and CONTAINER_ENGINE stay in their own files (moved later with a `riku doctor --fix` migration).

- [ ] **Step 1: Failing tests:** `missing_settings_file_gives_defaults`, `unknown_engine_key_is_refused` (error names key and file), `stop_waits_grace_plus_extra` (mock engine sleeping past grace plus a short cfg(test) extra is killed), `pull_limit_applies` (mock pull sleeping past a short limit fails naming pull).
- [ ] **Step 2: Run** `cargo test -q -p riku-config settings` and `cargo test -q -p riku-container bounded`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`. Expected: PASS.
- [ ] **Step 5: Docs:** `docs/docs/` page for server settings (create `settings.md`, link from mkdocs nav).
- [ ] **Step 6: Commit** `Add server settings file with engine time limits`.

## Run order

Tasks 1, 3 and 10 have no dependencies on each other and run at the same time, each in its own worktree. Then 2 (needs 1 and 3), 4 (needs 3), 5, 6, 7, 8 and 9 in order, 11 and 12 (need 7), 13 (needs 9), then 14 and 15 in parallel, then 16. Task 17 (added 2026-11-01) runs beside Task 7 in its own worktree; it touches riku-config, riku-container and doctor only. Reviews: Opus for 3, 4, 11 and the final branch review; Sonnet for the rest. At most 4 agents at a time.
