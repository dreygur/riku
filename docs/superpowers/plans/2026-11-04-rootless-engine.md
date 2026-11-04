# Rootless container engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The container engine runs rootless as the deploy user on systemd (docker, podman) and OpenRC (podman), riku is one root owned binary the deploy user cannot change, and the deploy user can no longer reach root through router config, while the base footprint the README promises stays as it is.

**Architecture:** riku is installed once at `/usr/local/bin/riku`, root owned, and every unit, hook and forced command names that path. `Engine` carries an env computed from the running uid and `settings.toml`, applied by every spawn, so the supervisor, git push deploys and `rk` commands see the same engine. Root `riku init` detects the init system by capability and runs one row of data per init system. A root `riku router-helper` subcommand of the same binary takes route JSON over a unix socket, renders it with the existing nginx and Caddy renderers into `/etc/riku/`, checks and reloads as root, and replies. Named volumes move once through `sudo riku migrate-volumes`; listed apps refuse to deploy until then.

**Tech Stack:** Rust workspace, `riku-container` engine driver, `riku-nginx`, `riku-caddy`, `serde`/`serde_json`, `nix` (already a workspace dependency), `anyhow`, `tracing`, systemd, OpenRC.

**Spec:** `docs/superpowers/specs/2026-11-04-rootless-engine-design.md`, addendum to `docs/superpowers/specs/2026-10-30-container-services-design.md`. Ledger: `.superpowers/sdd/2026-11-04-rootless-engine/progress.md` (this machine only).

**Not in this plan:** rootless docker on OpenRC (deferred until users ask), Procfile worker cgroup delegation, installing any engine or helper package, a real self update (`riku update` is a stub today and stays one, now root only).

## Global Constraints

- Follow `.claude/CLAUDE.md` in full. Rule 16: no `?` in new code or any function touched. Rule 12: files over about 200 lines split by concern; tests in sibling `_tests.rs` files. Rule 21: Read/Edit/Write for files, no `cat`/`sed -n`/heredocs/python, Bash only for git, cargo, docker, scripts. Every dispatch states rule 21.
- Rule 8: every new module that crosses a trust boundary opens with a `//!` comment stating who it trusts, what it validates and what runs as root. Each task names that boundary under **Security model**; the reviewer checks the comment exists and matches the code.
- Owner style (dreygur): helpers above callers, constants at the top, no section comments, enums over boolean parameters, test doubles named `mock_*`, no dashes in prose, comments, docs or commit messages, function names under 30 characters, braces always.
- Errors: `anyhow::Result`.
- Confidential names: never write the owner's real server, organisation or project names anywhere. Neutral names only (`edge`, `edge.example`, `myapp`). The controller searches every diff before each commit.
- No AI attribution in commits. Commit with `git commit -F <file> --no-verify` when a hook would add one. Claude agents only, never agy.
- Engine access only through `riku-container`'s `Engine`. Capability probing, never engine or distribution name checks; engine names appear in messages only.
- Containers stay an add-on: no engine installed means no rootless step, no engine check and no engine line from init or doctor.
- One binary: `/usr/local/bin/riku`, root owned. No second copy, no path under the deploy user's home in any unit, hook, forced command or relaunch.
- `riku router-helper`, `riku migrate-volumes` and `riku update` are `Placement::Server` in `crates/riku-args/src/placement.rs` and refuse to run unless euid is 0.
- Rootless never ships without the router helper. Tasks 6 to 10 land before Task 13, and Task 13's mode write refuses unless the helper answers `status`. The branch is not merged to main before Task 21 passes.
- Footprint check (F), the last step before every commit from Task 2 on: run `benches/footprint.sh` (Task 1). The binary must stay within the limit in the ledger below. Tasks that add or change a resident process (7, 10, 13, 14, 15) also run its idle RSS part. A failed F stops the task and goes to the owner.

## Footprint ledger

Filled by Task 1, read by every later task. Sizes in bytes, RSS in KiB, all on one Linux x86_64 host.

| Measure | main | branch at Task 1 | Limit |
|---|---|---|---|
| `target/release/riku` size | | | main size plus the allowance |
| Binary growth allowance (owner decision) | | | 10 percent of main's size, rounded up to 0.5 MiB |
| Idle RSS, supervisor | | | |
| Idle RSS, nginx master and workers | | | |
| Base footprint (sum of the two RSS rows) | | | 61440 KiB (60 MB, README) |
| Disk, the one binary | | | 50 MB (README) |

## Review Focus

1. A deploy user that writes `~/.riku/nginx/x.conf` with `access_log /etc/cron.d/x;` gets no root file, before and after the include upgrade. Tests in Task 8 and Task 10; real host in Task 21.
2. The helper trusts nothing in a request: unknown fields, directive breaking strings, non loopback TCP, paths outside the app, mismatched TLS pairs and a peer uid other than the deploy user or root are refused. Tests in Tasks 6 and 7.
3. A refused or failed reload leaves the previous files live. Test in Task 8 (`check_failure_restores_files`).
4. No app starts on an empty volume while its data sits in the rootful store. Tests in Task 11 and Task 16.
5. Installing the new binary without rerunning init changes nothing for a rootful install. Test in Task 2 (`absent_mode_keeps_today`).
6. Moving the binary never leaves a host where `git push`, `rk` or the supervisor point at a missing file. Test in Task 5 (`failed_move_keeps_old_binary`).

---

### Task 1: Footprint gate

**Files:**
- Create: `benches/footprint.sh`
- Modify: `benches/README.md` (how to run it), this plan's ledger

`benches/benchmark.sh` reports whole MB by integer division, reads `which riku` rather than a given build, takes only the first `riku supervisor` pid and never measures nginx, so it cannot hold a limit measured in hundreds of KiB.

- [ ] **Step 1: Write** `benches/footprint.sh <riku binary>`: prints the binary size in bytes (`stat`), then starts `riku supervisor` on an empty temp `RIKU_ROOT` with nginx running from the distribution package with its default `worker_processes`, waits 60 s, and prints the RSS in KiB of the supervisor and the sum over the nginx master and workers (`ps -o rss=`). Stops what it started. Prints the nginx version and worker count beside the numbers.
- [ ] **Step 2: Build** both trees with `cargo build --release` on the Linux host: main in a worktree under the scratch directory, this branch in place. Record the commits.
- [ ] **Step 3: Run** the script three times per tree; record the median of each measure in the ledger.
- [ ] **Step 4: Fill the limits.** Binary limit: main's size plus 10 percent of it, rounded up to 0.5 MiB (owner decision). Base footprint limit: 60 MB from the README.
- [ ] **Step 5: Gate.** If the branch's binary is already above its limit, its base footprint is above 60 MB, or the binary does not fit the README's 50 MB disk figure, stop here and report the numbers to the owner. Do not start Task 2.
- [ ] **Step 6: Commit** `Measure the footprint before rootless work` (script, README, ledger).

### Task 2: Engine env and settings keys

**Files:**
- Create: `crates/riku-container/src/engine/env.rs`, `env_tests.rs`
- Modify: `crates/riku-container/src/engine/identity.rs` (`Engine` gains the env), `bounded.rs`, `streamed.rs`, `login.rs`, `events.rs`, `logs.rs` (apply it), `fake.rs` (record each spawn's env), `crates/riku-config/src/settings.rs`, `settings_tests.rs`, `crates/riku-deploy/src/containers/engine_open.rs`

**Interfaces:**
- Produces: `pub enum EngineMode { Rootless, Rootful }`; `EngineSettings` gains `mode: Option<EngineMode>` (`None`: init has not run since the upgrade, owner decision) and `host: Option<String>` (`engine.mode`, `engine.host`). A bad value names the key and the file.
- Produces: `pub fn engine_env(mode: Option<EngineMode>, host: Option<&str>, uid: u32) -> Result<Vec<(String, String)>>`. Rootless: `XDG_RUNTIME_DIR=/run/user/<uid>`, refused unless it exists, is owned by `uid` and has mode 0700; `DBUS_SESSION_BUS_ADDRESS` when `$XDG_RUNTIME_DIR/bus` exists. Any mode: `DOCKER_HOST` from `host`. `None` and `Rootful`: nothing else.

**Security model:** `env.rs` doc comment: the env comes from the uid and `settings.toml` only, never from the caller's environment, which differs between sshd, the supervisor and init; the deploy user can edit `settings.toml`, and setting rootful gains it nothing without a socket it cannot reach.

- [ ] **Step 1: Failing tests:** `runtime_dir_wrong_mode_is_refused` (0755; error names the path and the mode), `host_becomes_docker_host`, `absent_mode_keeps_today` (env empty, Review Focus 5), `every_spawn_carries_env` (fake engine: one call per spawn path, each recorded env holds the value), `unknown_mode_is_refused`.
- [ ] **Step 2: Run** `cargo test -q -p riku-container env` and `cargo test -q -p riku-config settings`. Expected: FAIL.
- [ ] **Step 3: Implement.** `open_engine` builds the env from `settings.toml` and `getuid()`.
- [ ] **Step 4: Run** `cargo test -q --workspace`, `cargo clippy --workspace -q`, F. Expected: PASS, no warnings, within limit.
- [ ] **Step 5: Commit** `Give every engine call one computed env`.

### Task 3: Probe rootless through info

**Files:**
- Create: `crates/riku-container/src/engine/info.rs`, `info_tests.rs`, fixtures `info_docker_rootless.json`, `info_podman_rootless.json`, `info_docker_rootful.json`
- Modify: `crates/riku-container/src/engine/probe.rs` (`info` joins `REQUIRED_COMMANDS`), `identity.rs`, `crates/riku-deploy/src/containers/engine_open.rs`

**Interfaces:**
- Produces: `Engine::is_rootless(&self) -> Result<bool>` from `info --format json`: docker `SecurityOptions` holding `name=rootless`, podman `host.security.rootless`. Parsed as data, no engine name.
- `open_engine` in `Rootless` mode refuses an engine that is not rootless, naming the mode, the socket it reached and `sudo riku init`.

**Security model:** `info.rs` doc comment: the field is trusted only as the engine's own report; a rootful engine reached in rootless mode is refused, never used.

- [ ] **Step 1: Failing tests:** `docker_rootless_shape_parses`, `podman_rootless_shape_parses`, `rootful_engine_refused_in_rootless_mode`. Fixture shapes follow upstream docs and are marked unverified in the test file until Task 21 replaces them with captures.
- [ ] **Step 2: Run** `cargo test -q -p riku-container info`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Refuse a rootful engine in rootless mode`.

### Task 4: Init system detection

**Files:**
- Create: `crates/riku-config/src/host_file.rs`, `host_file_tests.rs`, `crates/riku-cli/src/setup/init_system.rs`, `init_system_tests.rs`
- Modify: `crates/riku-config/src/lib.rs`, `crates/riku-cli/src/setup/root_init.rs`

**Interfaces:**
- Produces: `pub enum InitSystem { Systemd, OpenRc, Other }`; `pub fn detect_init(root: &Path) -> InitSystem`: systemd when `<root>/run/systemd/system` is a directory, OpenRC when `openrc-run` is on `PATH` and `<root>/run/openrc` is a directory, else `Other`.
- Produces: `/var/lib/riku/host.toml` (root owned, 0644) with `init = "systemd" | "openrc" | "other"`; `pub fn read_host(path: &Path) -> Result<Option<HostFacts>>` for doctor.

**Security model:** `host_file.rs` doc comment: written only by root, read by anyone; a missing file means init has not run, not a default.

- [ ] **Step 1: Failing tests:** `systemd_dir_means_systemd`, `openrc_needs_both_signals` (binary present, no `/run/openrc`: `Other`), `host_file_round_trips`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli init_system` and `cargo test -q -p riku-config host_file`. Expected: FAIL.
- [ ] **Step 3: Implement**; root init writes the file on every run.
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Detect the init system by capability`.

### Task 5: One root owned binary

Opus review: every way the deploy user reaches riku changes path. Runs on every host, with or without a router or engine.

**Files:**
- Create: `crates/riku-cli/src/setup/binary_move.rs`, `binary_move_tests.rs`
- Modify: `crates/riku-cli/src/setup/binary.rs` (`INSTALL_DIR` becomes the absolute `/usr/local/bin`; stage and rename kept; the shell rc `PATH` lines removed), `init.rs` (non root init requires `/usr/local/bin/riku`, else names `sudo riku init`), `root_init.rs`, `system_service.rs`, `user_service.rs`, `deploy_keys.rs`, `git_hook.rs`, `guidance.rs`, `crates/riku-cli/src/apps/create.rs`, `crates/riku-cli/src/git/repo.rs` (hook templates), `crates/riku-util/src/authorized_keys.rs` (`migrate_text` also rewrites the binary path), `crates/riku-deploy/src/supervisor_ctl.rs` (relaunch never consults `~/.local/bin`), `crates/riku-remote/src/remote/dashboard/remote_script.rs`, `scripts/dashboard-tunnel.sh`, `scripts/install.sh` (server install as root only), `crates/riku-cli/src/apps/control.rs` and `crates/riku-args/src/placement.rs` (`update` root only, `Server`), `crates/riku-args/src/client_text.rs`, `crates/riku-cli/src/doctor/checks.rs`

**Interfaces:**
- Root init installs the running binary to `/usr/local/bin/riku` (root owned, 0755) before anything else.
- `pub fn move_install(user: &DeployUser) -> Result<()>`, idempotent: as the deploy user, rewrite each forced command in `authorized_keys` and every `post-receive` hook under `~/.riku/repos/`; as root, rewrite the system unit (and the user unit when present), `daemon-reload`, restart the supervisor; only then remove `~/.local/bin/riku` as the deploy user. Any failure before the removal returns the error with the old binary still in place.
- `rk update` refuses on the laptop with "run on the server: sudo riku update". `riku update` as non root refuses the same way.
- doctor: `/usr/local/bin/riku` root owned and not group or world writable; no forced command, unit or hook naming another path; a leftover `~/.local/bin/riku` reported.

**Security model:** `binary_move.rs` doc comment: root writes only `/usr/local/bin/riku` and the system units; files in the deploy user's tree are rewritten as that user; root never executes or trusts a file from that tree, and the old binary is removed only after every reference points away from it.

- [ ] **Step 1: Failing tests:** `forced_command_path_rewritten` (old line with `~/.local/bin/riku` becomes `/usr/local/bin/riku`, the rest of the line unchanged), `hooks_name_root_binary` (every hook template, no `$HOME/.local/bin`), `failed_move_keeps_old_binary` (Review Focus 6: unit rewrite fails; old file still there, keys unchanged), `move_runs_without_router` (temp root, no nginx, no engine: move completes), `update_refuses_non_root`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli binary_move update` and `cargo test -q -p riku-util authorized_keys`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, `./tests/deploy-smoke/quick-test.sh myapp node`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Install riku once as a root owned binary`.

### Task 6: Route schema and builder

Opus review: this is the helper's input boundary.

**Files:**
- Create: crate `crates/riku-route/` (`Cargo.toml`, `src/lib.rs`, `src/schema.rs`, `src/schema_tests.rs`, `src/check.rs`, `src/check_tests.rs`), `crates/riku-deploy/src/router/route_build.rs`, `route_build_tests.rs`
- Modify: workspace `Cargo.toml`, `crates/riku-deploy/Cargo.toml`

**Interfaces:**
- Produces: `pub struct Route` (spec "Route schema (v1)"), `#[serde(deny_unknown_fields)]`, and `pub fn check_route(app: &AppName, route: &Route, riku_root: &Path) -> Result<()>`: directive breaking rule of `riku-nginx` `host.rs`, riku-util `Domain` and `Backend` rules, loopback TCP only, unix paths inside the riku root, `static` and `document_root` absolute, no `..`, inside the app directory, TLS PEM parsed as one chain and one matching key, snippet through `snippet_grammar` and `ALLOWED_DIRECTIVES`, snippet refused for Caddy.
- Produces: `pub fn build_route(app: &AppName, env: &HashMap<String, String>, paths: &RikuPaths) -> Result<Route>` mapping the env keys the spec lists; any other key the nginx generator reads is mapped or refused by name. `RIKU_NGINX_CACHE_PATH` is refused with a message saying the helper picks the path.

**Security model:** `riku-route` crate doc: every field is untrusted; `check_route` is the only gate and the helper calls it again on receipt. The deploy side calls it for an early error only.

- [ ] **Step 1: Failing tests:** `unknown_field_is_refused`, `directive_break_in_domain_is_refused`, `public_tcp_backend_is_refused`, `static_outside_app_is_refused`, `mismatched_tls_pair_is_refused`, `filtered_snippet_outside_allowlist_is_refused`, `env_maps_to_route` (one ENV with every mapped key gives the expected route).
- [ ] **Step 2: Run** `cargo test -q -p riku-route` and `cargo test -q -p riku-deploy route_build`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Define the route the router helper accepts`.

### Task 7: Helper socket, protocol and subcommand

Opus review: root process facing the deploy user.

**Files:**
- Create: `crates/riku-route/src/protocol.rs`, `protocol_tests.rs`, `src/server.rs`, `server_tests.rs`, `src/listen.rs` (socket activation or own socket), `crates/riku-deploy/src/router/helper_client.rs`, `helper_client_tests.rs`, `crates/riku-cli/src/router_helper.rs`
- Modify: `crates/riku-args/src/cli.rs`, `crates/riku-args/src/placement.rs` (`router-helper` and `router` as `Server`), `crates/riku-cli/src/dispatch/mod.rs`

**Interfaces:**
- Produces: `pub enum Request { Apply { app, route }, Remove { app }, Status }` with `v: 1`; `pub enum Reply { Ok { reloaded: bool }, Failed { stage: Stage, error: String }, Status { .. } }`, `pub enum Stage { Schema, Render, Check, Reload }`.
- Server: one request per connection, one connection at a time, request at most 1 MiB then EOF, `SO_PEERCRED` through `nix` refusing any uid but the deploy user's and 0, exits after 60 s idle when socket activated (`LISTEN_FDS`/`LISTEN_PID`), otherwise creates `/run/riku/router.sock` itself (`/run/riku` root owned 0750 with the deploy group, socket 0660) and stays running.
- Client: `pub fn send(request: &Request) -> Result<Reply>`, giving up after 60 s.
- `riku router-helper` runs the server; it applies nothing yet (`Apply` replies `Failed { stage: Render }` until Task 8).

**Security model:** `server.rs` doc comment: runs as root; the peer uid check and the socket's group are both required; input is bounded in size and time; one request is served at a time so concurrent deploys queue in the backlog.

- [ ] **Step 1: Failing tests:** `oversize_request_is_refused`, `other_uid_is_refused` (peer check as a pure function over a credential value), `status_round_trips` (server on a temp socket, client sends `Status`), `non_root_refuses_to_start`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route protocol server` and `cargo test -q -p riku-deploy helper_client`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS (the helper is resident on OpenRC: record its idle RSS in the ledger). Expected: PASS, within limit.
- [ ] **Step 5: Commit** `Add the router helper socket and protocol`.

### Task 8: nginx through the helper

Opus review: Review Focus 1 and 3.

**Files:**
- Create: `crates/riku-route/src/nginx_apply.rs`, `nginx_apply_tests.rs`
- Modify: `crates/riku-nginx/src/context.rs` and `template.rs` (render from a `Route`; cache path chosen by the caller), `crates/riku-deploy/src/router.rs` and `router/reload.rs` (send `Apply`/`Remove` when the helper is installed)

**Interfaces:**
- Produces: `fn apply_nginx(app: &AppName, route: &Route) -> Result<Reply>`: `check_route`, render, write `/etc/riku/nginx/<app>.conf` and `/etc/riku/certs/<app>/{crt,key}` (key 0600) through write then rename, cache at `/var/cache/riku/nginx/<app>`, `nginx -t` as root, reload; on `check` or `reload` failure restore the previous files before replying.
- Deploy side: `router::apply` sends `Apply` and maps `Failed` to an error carrying the stage and the router's own message. `ok` replaces the unit timestamp wait of `router/reload/nginx_unit.rs` when the helper is in use.

**Security model:** `nginx_apply.rs` doc comment: root writes only under `/etc/riku` and `/var/cache/riku`, paths built from a checked `AppName`, never from request text; TLS material is written from PEM text, never read from a path.

- [ ] **Step 1: Failing tests** (mock `nginx` on a temp `PATH`, temp `/etc/riku` root): `render_matches_generator` (same input gives today's generator output, cache path aside), `check_failure_restores_files`, `access_log_snippet_never_written`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route nginx_apply`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; with Docker, render and `nginx -t` in `nginx:1.27`; F. Expected: PASS.
- [ ] **Step 5: Commit** `Render and reload nginx in the router helper`.

### Task 9: Caddy through the helper

**Files:**
- Create: `crates/riku-route/src/caddy_apply.rs`, `caddy_apply_tests.rs`
- Modify: `crates/riku-caddy/src/site.rs`, `lib.rs` (stale doc comment about a path unit), `crates/riku-cli/src/setup/caddy_host.rs` (import `/etc/riku/caddy/*.caddy`, admin on `unix//run/caddy/admin.sock`, `caddy` leaves the deploy group, `g+x` grants undone; refuse with the lines to add when a global block cannot be merged)

**Interfaces:**
- Produces: `fn apply_caddy(app: &AppName, route: &Route) -> Result<Reply>`: write `/etc/riku/caddy/<app>.caddy`, `caddy reload --config /etc/caddy/Caddyfile --address unix//run/caddy/admin.sock` as root, restore on failure.

**Security model:** `caddy_apply.rs` doc comment: the admin API moves off localhost TCP to a socket only `caddy` and root reach, so no local process controls routes.

- [ ] **Step 1: Failing tests:** `caddy_reload_failure_restores`, `snippet_for_caddy_is_refused`, `unmergeable_global_block_refused`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route caddy_apply` and `cargo test -q -p riku-cli caddy_host`. Expected: FAIL.
- [ ] **Step 3: Implement.** Admin socket syntax and the `--address` flag are unverified; Task 21 confirms them.
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Reload Caddy through the router helper`.

### Task 10: Install the helper and upgrade from the include

Opus review: the upgrade must never leave a host without a working router.

**Files:**
- Create: `crates/riku-cli/src/setup/helper_install.rs`, `helper_install_tests.rs`, `crates/riku-cli/src/router_republish.rs`
- Modify: `crates/riku-cli/src/setup/root_init.rs` (helper right after the binary, on every host with a router), `router_units.rs` (remove `riku-nginx.path` and `riku-nginx-reload.service`), `nginx_host.rs` and `crates/riku-nginx/src/host.rs` (include `/etc/riku/nginx/*.conf`), `crates/riku-deploy/src/router/host_guard.rs` (refuse while the old include is in place)

**Interfaces:**
- init writes `riku-router-helper.socket` and `.service` running `/usr/local/bin/riku router-helper` on systemd, or `/etc/init.d/riku-router-helper` on OpenRC; on other init systems it prints the command for the operator to start by hand (owner decision). It starts the helper, runs `riku router republish` as the deploy user, and only when every app applied rewrites the include, checks with `nginx -t` as root and reloads; on failure restores the old include and stops.
- `riku router republish`: sends `Apply` for every app.

**Security model:** `helper_install.rs` doc comment: the helper runs only the root owned `/usr/local/bin/riku` (Task 5); root never executes a file from the deploy user's tree.

- [ ] **Step 1: Failing tests:** `include_kept_until_all_apply` (one app refused: include unchanged), `failed_check_restores_include`, `old_include_refuses_deploy`, `units_render_per_init` (systemd socket unit carries `SocketMode=0660` and the deploy group; OpenRC script starts before `riku`; `Other` prints the start command).
- [ ] **Step 2: Run** `cargo test -q -p riku-cli helper_install` and `cargo test -q -p riku-deploy host_guard`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; F with idle RSS. Expected: PASS, within limit.
- [ ] **Step 5: Commit** `Install the router helper and move nginx off the user include`.

### Task 11: Volume move list and deploy refusal

Opus review: Review Focus 4.

**Files:**
- Create: `crates/riku-deploy/src/containers/volume_moves.rs`, `volume_moves_tests.rs`
- Modify: `crates/riku-deploy/src/containers/deploy.rs`, `control.rs`, `copy_restart.rs`

**Interfaces:**
- Produces: `pub fn list_moves(engine: &Engine, paths: &RikuPaths) -> Result<VolumeMoves>`: named volumes mounted by containers labelled `riku.app`, plus `<project>_<key>` per compose app's declared keys; `pub fn write_moves` to `/var/lib/riku/volume-moves.toml` (root owned 0644) and `pub fn pending_for(app: &AppName) -> Result<Vec<String>>`.
- Deploy, restart and the supervisor's copy restart refuse for a listed app, naming `sudo riku migrate-volumes <app>`.

**Security model:** `volume_moves.rs` doc comment: the list is written by root and only read by the deploy user; a listed app never starts, so no copy runs on an empty volume.

- [ ] **Step 1: Failing tests:** `labelled_and_declared_volumes_listed`, `listed_app_refuses_deploy`, `listed_app_refuses_copy_restart`.
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy volume_moves`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Refuse to start apps whose volumes have not moved`.

### Task 12: Port refusal below 1024

**Files:**
- Modify: `crates/riku-deploy/src/containers/spec.rs`, `spec_tests.rs`, `deploy.rs`

**Interfaces:**
- In `Rootless` mode, a non routed host port below `/proc/sys/net/ipv4/ip_unprivileged_port_start` is refused before anything stops, naming the service, the port and the sysctl. Routed copies are untouched (ephemeral ports).

**Security model:** none new; the sysctl is read, never written.

- [ ] **Step 1: Failing tests:** `public_port_80_refused_rootless`, `routed_port_unaffected` (sysctl path injected for the test).
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy spec`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Refuse low host ports in rootless mode`.

### Task 13: systemd rootless init

Opus review: root steps acting for the deploy user.

**Files:**
- Create: `crates/riku-cli/src/setup/rootless/mod.rs`, `checks.rs`, `subids.rs`, `systemd_steps.rs`, sibling `_tests.rs`
- Modify: `crates/riku-cli/src/setup/root_init.rs`, `deploy_user.rs`

**Interfaces:**
- All checks before any change: no engine means skip this whole task's steps silently; subuid and subgid range (add `<user>:<start>:65536` with a start overlapping no entry, `usermod --add-subuids/--add-subgids` when present, else an atomic write), `newuidmap`/`newgidmap`, `unshare --user true` as the deploy user, `pasta` or `slirp4netns`, kernel 5.13 or `fuse-overlayfs` plus `/dev/fuse`, cgroup v2, no app on a legacy worker.
- systemd: `loginctl enable-linger`, `user@.service.d/riku-delegate.conf` (`Delegate=cpu cpuset io memory pids`), docker's `dockerd-rootless-setuptool.sh install` as the deploy user and its user `docker.service`; podman nothing more.
- Write `engine.mode` and `engine.host` through `DeployUser`, refused unless the helper answers `status` (Task 7). Verify as the deploy user: rootless reported, a `--memory 64m` container shows the limit in `stats`. Then `write_moves` (Task 11).
- Fallback: OpenRC with docker only, `Other`, or owner `rootful`: write `rootful` and print the one line reason (grant text in Task 18).

**Security model:** `rootless/mod.rs` doc comment: root edits only `/etc/subuid`, `/etc/subgid` and the systemd drop in; everything in the deploy user's tree is written as that user; riku never adds a group.

- [ ] **Step 1: Failing tests:** `subuid_start_avoids_overlap`, `no_engine_runs_no_step` (no output line mentions an engine), `mode_refused_without_helper`, `legacy_app_blocks_rootless`, `docker_on_openrc_falls_back`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli rootless`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS (lingering user manager recorded separately as add-on cost). Expected: PASS.
- [ ] **Step 5: Commit** `Set up a rootless engine on systemd hosts`.

### Task 14: OpenRC podman init

**Files:**
- Create: `crates/riku-cli/src/setup/rootless/openrc_steps.rs`, `openrc_steps_tests.rs`, templates for `riku-session`
- Modify: `crates/riku-cli/src/setup/rootless/mod.rs`

**Interfaces:**
- Requires `rc_cgroup_mode="unified"` in `/etc/rc.conf`, refusing with the line. Writes `/etc/init.d/riku-session` (`checkpath -d -m 0700 -o <user>:<group> /run/user/<uid>`, `riku-user` cgroup with `+cpu +memory +pids +io`, chowned per the kernel delegation rule, leaf `riku-user/supervisor`), `rc-update add` and `rc-service start`.

**Security model:** `openrc_steps.rs` doc comment: root creates and delegates exactly one cgroup subtree and one runtime directory; it refuses an existing directory with another owner or mode.

- [ ] **Step 1: Failing tests:** `legacy_cgroup_mode_refused`, `session_script_renders` (checkpath line and leaf creation present).
- [ ] **Step 2: Run** `cargo test -q -p riku-cli openrc_steps`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS. Expected: PASS.
- [ ] **Step 5: Commit** `Set up rootless podman on OpenRC hosts`.

### Task 15: Supervisor under the user manager and supervise-daemon

**Files:**
- Modify: `crates/riku-cli/src/setup/user_service.rs`, `system_service.rs`, `systemd.rs`, `rootless/openrc_steps.rs`, `guidance.rs`, sibling tests

**Interfaces:**
- systemd rootless: `~/.config/systemd/user/riku.service` written by the deploy user step with `ExecStart=/usr/local/bin/riku supervisor`, `systemctl --user --machine=<user>@ enable --now riku` by root, `After=docker.service` for docker, no `NoNewPrivileges` and no mount sandboxing; `MemoryMax` and `CPUQuota` kept until Task 21 decides. System `riku.service` disabled on switch, re-enabled on a return to rootful.
- OpenRC: `/etc/init.d/riku`, `supervise-daemon`, `command=/usr/local/bin/riku`, `command_user=<user>`, `need riku-session riku-router-helper`, `start_post` writes its pid to `riku-user/supervisor/cgroup.procs`.

**Security model:** unit and script doc comments state why `NoNewPrivileges` is dropped (setuid `newuidmap`) and that limits come from the delegated cgroup.

- [ ] **Step 1: Failing tests:** `rootless_unit_drops_no_new_privileges`, `switch_disables_system_unit`, `openrc_script_runs_as_user`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli user_service system_service openrc`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS. Expected: PASS.
- [ ] **Step 5: Commit** `Run the supervisor as the deploy user in rootless mode`.

### Task 16: migrate-volumes

Opus review: the data safety boundary.

**Files:**
- Create: `crates/riku-cli/src/migrate_volumes/mod.rs`, `copy.rs`, `uid_shift.rs`, sibling `_tests.rs`
- Modify: `crates/riku-args/src/cli.rs`, `placement.rs`, `crates/riku-cli/src/dispatch/mod.rs`, `crates/riku-container/src/engine/` (volume create with labels, `image inspect` presence)

**Interfaces:**
- `sudo riku migrate-volumes [<app>...]`, per app: helper image `busybox@sha256:<digest>` (digest pinned in source when built) present or pulled through the rootless engine, else stop before touching anything with the `<engine> load` message; refuse a rootful daemon with `userns-remap`; refuse files with a uid outside the subuid range; stop the app's rootful containers; create each rootless volume with the same name and labels; stream a tar read by root into `run --rm -i --network none -v <vol>:/dst <image> tar -x -p -C /dst` as the deploy user; count and size check; remove the app from the list. Never touches a rootful volume. Rerun is a no op.

**Security model:** `migrate_volumes/mod.rs` doc comment: root reads rootful volume data and nothing else; writes happen inside a networkless container as the deploy user; rootful volumes are read only to this command.

- [ ] **Step 1: Failing tests** (fake engine): `uid_outside_range_refused` (uid shift edges 0, 1, last, first outside), `offline_stops_before_stop`, `never_removes_rootful_volume` (recorded argv), `rerun_is_noop`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli migrate_volumes`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Move named volumes into the rootless store`.

### Task 17: Image import on the driver, legacy plugin retired

**Files:**
- Modify: `crates/riku-deploy/src/container_runtime.rs`, `crates/riku-container/src/engine/` (`save` and `load`, not in `REQUIRED_COMMANDS`), workspace `Cargo.toml` and the install path for `crates/riku-plugin-container` (no longer shipped or installed), `crates/riku-cli/src/setup/` plugin install

**Interfaces:**
- Local build and save through `Engine`; the remote side runs a riku subcommand through the forced command instead of `ssh <host> "<engine> load"`, so it gets the engine env. Legacy worker detection and migration in `crates/riku-deploy/src/containers/` stay.

**Security model:** the remote subcommand reads an image stream on stdin only and passes nothing from the client to argv.

- [ ] **Step 1: Failing tests:** `import_uses_engine_env` (fake engine records env on `load`), `no_bare_engine_literal` (grep test over `container_runtime.rs` for `"docker"`/`"podman"` outside messages).
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy container_runtime`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F (binary should shrink or hold). Expected: PASS.
- [ ] **Step 5: Commit** `Import images through the engine driver`.

### Task 18: Rootful fallback grant message

**Files:**
- Create: `crates/riku-cli/src/setup/rootful_grant.rs`, `rootful_grant_tests.rs`
- Modify: `crates/riku-cli/src/setup/rootless/mod.rs`, `guidance.rs`

**Interfaces:**
- `pub fn grant_text(engine: &Engine, user: &str, reason: FallbackReason) -> String`: docker `usermod -aG docker <user>`, then restart the supervisor; podman, nerdctl, finch no grant and a message to use docker or a supported init system; always followed by the root equivalence sentence from the spec. `pub enum FallbackReason { Owner, OtherInit, DockerOnOpenRc }`. riku never runs the grant.

**Security model:** doc comment: text only, nothing executed; the warning is not optional.

- [ ] **Step 1: Failing tests:** `docker_grant_carries_warning`, `docker_on_openrc_names_podman`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli rootful_grant`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Print the rootful grant with its warning`.

### Task 19: doctor

**Files:**
- Create: `crates/riku-cli/src/doctor/rootless_check.rs`, `rootless_check_tests.rs`, `helper_check.rs`, `helper_check_tests.rs`
- Modify: `crates/riku-cli/src/doctor/mod.rs`, `container_check.rs`, `router_check.rs`, `checks.rs`

**Interfaces:**
- No engine: only the binary, helper and init system lines (binary lines from Task 5). With an engine, every line in the spec's doctor section, rootful as a warning with `grant_text`, `engine.mode` absent as "run sudo riku init". Helper: socket answers `status`; no router config the deploy user can write is loaded; leftover `~/.riku/nginx` files listed as unused.

**Security model:** doctor runs as the deploy user and only reads; it calls the helper's `status` and nothing else.

- [ ] **Step 1: Failing tests:** `no_engine_prints_no_engine_line`, `rootful_shows_grant`, `helper_down_fails`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli doctor`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Report rootless and router helper state in doctor`.

### Task 20: Docs and CHANGELOG

**Files:**
- Modify: `docs/docs/runtimes.md`, `nginx.md`, `caddy.md`, `systemd.md`, `settings.md`, `doctor.md`, `installation.md`, `cli.md`, `README.md` (install path, footprint figures from the ledger, add-on costs stated apart), `CHANGELOG.md`, `docs/superpowers/ROADMAP.md`

- [ ] **Step 1: Docs:** the one binary at `/usr/local/bin/riku` and that updates need root; rootless mode per init system and engine, OpenRC docker deferred, the grant and its warning, `engine.mode` and `engine.host`, the router helper and `/etc/riku`, starting the helper by hand on other init systems, Caddy's admin socket, `migrate-volumes`, low port refusal, footprint with the engine's cost stated as the add-on's.
- [ ] **Step 2: CHANGELOG**, under Unreleased. From the 3a ledger, unless 3a Task 16 already wrote them: run `sudo riku init` after upgrade; a stray `.conf` in `~/.riku/nginx` refuses deploys; `RIKU_NGINX_INCLUDE_FILE` is grammar plus allowlist and old snippets may be refused; an app `nginx.conf` is ignored; filter plugins adding non allowlisted directives fail deploys; container app detection (a compose file or a root `Dockerfile`/`Containerfile` deploys as riku containers, owner decision); port host IP form refused, use the `riku.public` label; `destroy --force`; the logs glob change. From this plan: riku moves from `~/.local/bin` to a root owned `/usr/local/bin/riku`, and `riku update` needs root; rootless by default when an engine is present; root router helper and `/etc/riku`; `RIKU_NGINX_CACHE_PATH` removed; Caddy admin on a unix socket; legacy container plugin retired; `migrate-volumes`; host ports below the unprivileged start refused.
- [ ] **Step 3: Run** `NO_MKDOCS_2_WARNING=1 uvx --with mkdocs-material --with mkdocs-minify-plugin mkdocs build --strict -d <scratch>/site`. Expected: no warnings.
- [ ] **Step 4:** ROADMAP status lines with commits.
- [ ] **Step 5: Commit** `Document the rootless engine and router helper`.

### Task 21: Real host check

Three hosts: systemd with docker, systemd with podman, OpenRC (Alpine VM) with podman. Plus one systemd host with neither router nor engine, one OpenRC host with docker only, and one runit or sysvinit VM. Results go in the ledger; each failure is a bug fixed before Task 22, never a skipped item.

- [ ] Footprint: F on the no engine host within the ledger limits; helper idle RSS on OpenRC recorded; lingering user manager recorded as add-on cost.
- [ ] Binary move from a `~/.local/bin/riku` install, on a host with a router and on the host with neither: `git push`, `rk` commands and the supervisor keep working; the old file is gone; `/usr/local/bin` is on the deploy user's `PATH` over ssh and sudo (unverified outside Debian and Alpine); `rk update` and non root `riku update` refuse with the root message.
- [ ] Init from clean on each of the three hosts; refusals on cgroup v1, missing network helper, missing `newuidmap`.
- [ ] No engine host: init and doctor print nothing about engines.
- [ ] OpenRC docker only and runit or sysvinit: init sets rootful, prints the grant, the warning and the reason, runs nothing; on runit or sysvinit init prints the helper start command and deploys work once the operator starts it.
- [ ] Unverified in the spec, each confirmed or the design changed: docker `SecurityOptions` `name=rootless` and podman `host.security.rootless` (captured JSON replaces the Task 3 fixtures); podman's systemd cgroup manager uses the user bus; kernel 5.13 versus 5.11 for overlay; `nginx -s reload` as the deploy user fails with EPERM on a root master; `Delegate=` needed for `--cpus`; `systemctl --user --machine=<user>@` needs systemd 248; `/run` is a tmpfs on Alpine; podman falls back to cgroupfs without a bus and its containers sit under `riku-user`; user unit mount sandboxing needs an extra user namespace; Caddy `admin unix//...` syntax and `caddy reload --address unix//...`.
- [ ] `riku-session` recreates `/run/user/<uid>` after reboot; the OpenRC supervisor sits in `riku-user/supervisor` and is respawned after `kill -9`.
- [ ] Deploy through git push and through `rk` in a session without `XDG_RUNTIME_DIR`.
- [ ] A supervisor restart leaves containers running; `--memory` shows in `stats`; containers are outside the supervisor unit's cgroup (decides `MemoryMax`/`CPUQuota`, Task 15).
- [ ] `migrate-volumes` on a postgres volume: data intact, owners shifted, rootful volume unchanged, rerun a no op, offline run stops before stopping containers.
- [ ] `riku.public` port 80 refused with the sysctl named.
- [ ] A deploy user `.conf` with `access_log /etc/cron.d/x;` is never loaded; a filter adding `access_log` is refused; the Caddy admin port on localhost is closed.
- [ ] Upgrade of a rootful install: new binary alone changes nothing; `sudo riku init` moves the binary and the routes without a failed request in a `curl` loop.

### Task 22: Final review

- [ ] One Opus review of the whole branch against the spec, Review Focus and rule 8 doc comments; fix findings, rerun `cargo test -q --workspace`, `cargo clippy --workspace -q` and F.
- [ ] Confidential name search over the branch diff. Ask the owner before push.

## Run order

Task 1 first and alone; nothing starts if its gate fails. Then 2, 3, 4 and 5 (independent, own worktrees). Then 6, 7, 8, 9, 10 in order (the router helper; 10 needs 5). Then 11 and 12 (independent). Then 13 (needs 2, 3, 4, 7, 11), 14, 15 in order. 16 and 17 after 13, in parallel. 18 after 13, 19 after 18, 20 after 19, 21 after all, 22 last. Opus reviews for 5, 6, 7, 8, 10, 11, 13, 16 and the final review; Sonnet for the rest. At most 4 agents at a time.
