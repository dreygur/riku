# Rootless container engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The container engine runs rootless as the deploy user on systemd (docker, podman) and OpenRC (podman), riku is one root owned binary the deploy user cannot change, and the deploy user can no longer reach root through router config, while the footprint the README states stays as it is.

**Architecture:** riku is installed once at `/usr/local/bin/riku`, root owned, and every unit, hook and forced command names that path. `Engine` carries an env computed from the running uid and `settings.toml`, applied by every spawn, so the supervisor, git push deploys and `rk` commands see the same engine. Root `riku init` detects the init system by capability and runs one row of data per init system. A root `riku router-helper` subcommand of the same binary takes route JSON over a unix socket, renders it with the existing nginx and Caddy renderers into root owned files under `/etc/riku`, checks and reloads as root, and replies.

**Tech Stack:** Rust workspace, `riku-container` engine driver, `riku-nginx`, `riku-caddy`, `riku-util` (`RoutePlan`, `Domain`, `Backend`), `serde`/`serde_json`, `nix`, `anyhow`, `tracing`, systemd, OpenRC.

**Spec:** `docs/superpowers/specs/2026-11-04-rootless-engine-design.md`, addendum to `docs/superpowers/specs/2026-10-30-container-services-design.md`. Ledger: `.superpowers/sdd/2026-11-04-rootless-engine/progress.md` (this machine only).

**Refreshed** 2026-11-05 against `feat/container-integration` at ffd7909 (phase 3a done, PR #73). Starts from that branch or from main once PR #73 merges.

**Not in this plan:** rootless docker on OpenRC (deferred until users ask), Procfile worker cgroup delegation, installing any engine or helper package, router plugins, a self update.

riku has one user, so there is no migration and no backward compatibility; reinstall and redeploy.

## Status of the 2026-11-04 plan

The earlier plan had 20 tasks. Against the code at ffd7909:

| Old task | Status | Where it went |
|---|---|---|
| 1 Footprint gate | partly: baseline measured in 3a (optz-runtime-report.md, check-5-and-benchmark-report.md); `benches/footprint.sh` does not exist | Task 1, shrunk |
| 2 Engine env and settings keys | still needed | Task 2 |
| 3 Probe rootless through info | still needed | Task 3 |
| 4 Init system detection | still needed; `/var/lib/riku` already exists (var_root.rs:43-72) | Task 4 |
| 5 One root owned binary | still needed; the `riku update` part is done (6a5307a) | Task 5 |
| 6 Route schema and builder | still needed; builds on `RoutePlan` (crates/riku-util/src/route_plan.rs:17) | Task 6 |
| 7 Helper socket and protocol | still needed | Task 7 |
| 8 nginx through the helper | still needed | Task 8 |
| 9 Caddy through the helper | partly: `caddy` no longer in the deploy group, no home grants, sites in `/etc/riku/caddy` (c087210) | Task 9, shrunk |
| 10 Install the helper | still needed; the include is already root owned with stray check and catch all (4294a66, f3fee94), the reload unit has no start limit (6e79b09) | Task 10 |
| 11 Port refusal below 1024 | still needed | Task 11 |
| 12 systemd rootless init | still needed | Task 12 |
| 13 OpenRC podman init | still needed; now also recreates `/run/riku-sockets` | Task 13 |
| 14 Supervisor under the user manager | still needed | Task 14 |
| 15 Image import on the driver, legacy plugin retired | mostly done: plugin deleted (bb4aa56), compose migration and legacy paths removed (1ec2024, 030e7aa); `container_runtime.rs` remains | dropped; see Open questions 1 |
| 16 Rootful grant message | still needed | Task 15 |
| 17 doctor | still needed | Task 16 |
| 18 Docs and CHANGELOG | still needed; the 3a CHANGELOG items were written in 07d39f5 | Task 17, shrunk |
| 19 Real host check | still needed | Task 18 |
| 20 Final review | still needed | Task 19 |

## Global Constraints

- Follow `.claude/CLAUDE.md` in full. Rule 16: no `?` in new code or any function touched. Rule 12: files over about 200 lines split by concern; tests in sibling `_tests.rs` files. Rule 21: Read/Edit/Write for files, no `cat`/`sed -n`/heredocs/python, Bash only for git, cargo, docker, scripts. Every dispatch states rule 21.
- Rule 8: every new module that crosses a trust boundary opens with a `//!` comment stating who it trusts, what it validates and what runs as root. Each task names that boundary under **Security model**; the reviewer checks the comment exists and matches the code.
- Owner style (dreygur): helpers above callers, constants at the top, no section comments, enums over boolean parameters, test doubles named `mock_*`, no dashes in prose, comments, docs or commit messages, function names under 30 characters, braces always.
- Owner process rules: Claude agents only, never agy; smallest fix; one review round per task.
- Errors: `anyhow::Result`.
- Confidential names: never write the owner's real server, organisation or project names anywhere. Neutral names only (`edge`, `edge.example`, `myapp`). The controller searches every diff before each commit.
- No AI attribution in commits. Commit with `git commit -F <file> --no-verify` when a hook would add one.
- Engine access only through `riku-container`'s `Engine`. Capability probing, never engine or distribution name checks; engine names appear in messages only.
- Containers stay an add-on: no engine installed means no rootless step, no engine check and no engine line from init or doctor.
- One binary: `/usr/local/bin/riku`, root owned. No path under the deploy user's home in any unit, hook, forced command or relaunch.
- Reuse what 3a built rather than adding parallel pieces: `check_write_dir` (crates/riku-cli/src/setup/nginx_host.rs:96) for every root write, `socket_dir::prepare` (crates/riku-util/src/socket_dir.rs) for shared directories, the riku-config constants `SOCKET_ROOT`, `SOCKET_GROUP`, `CADDY_SITE_ROOT`, `VAR_ROOT`, `ACME_WEBROOT`, `STATIC_ROOT` (crates/riku-config/src/lib.rs:56-77), and `DeployUser` for anything written in the deploy user's tree.
- `riku router-helper` is `Placement::Server` in `crates/riku-args/src/placement.rs` and refuses to run unless euid is 0.
- Rootless never ships without the router helper. Tasks 6 to 10 land before Task 12, and Task 12's mode write refuses unless the helper answers `status`. The branch is not merged to main before Task 18 passes.
- Footprint check (F), the last step before every commit from Task 2 on: run `benches/footprint.sh` (Task 1) on a release build. The binary must stay within the ledger limit. Tasks that add or change a resident process (7, 10, 12, 13, 14) also run its idle RSS part. A failed F stops the task and goes to the owner.

## Footprint ledger

Sizes in bytes, RSS in KiB, all on the 4 core x86_64 Linux test server, release profile opt-level "z" (Cargo.toml:54), stripped.

| Measure | Baseline | Source | Limit |
|---|---|---|---|
| stripped `riku` | 9,198,584 | optz-runtime-report.md, c69aa1f | 10,247,160 (baseline plus 10 percent, 919,859, rounded up to 0.5 MiB: 1,048,576) |
| Supervisor idle RSS, no apps | 8,756 (median of 3, 79491da, binary 9,213,048 B) | `benches/footprint.sh` in a `debian:12` container on the test server, no cgroup delegation | 9,632 (baseline plus 10 percent, Open question 2) |
| Supervisor plus nginx 1.22.1 (master and 4 workers), idle, no apps | 21,724 (median of 3, same run) | same | 23,897 (baseline plus 10 percent, Open question 2) |
| Binary plus riku state on disk, no apps | about 9.3 MB | README.md:55-56 | the binary limit plus state; README says about 10 MB (Open question 3) |
| Router helper idle RSS (OpenRC and other init systems) | 7,068 (median of 3, Task 7, binary 9,329,080 B) | `riku router-helper` as root in a `debian:12` container on the test server, 5 s after start | counts toward the base |
| Lingering user manager (rootless systemd hosts) | none yet | Task 12 | add-on cost, reported apart |

## Review Focus

1. A deploy user that writes a `.conf` with `access_log /etc/cron.d/x;` anywhere it can write gets no root file. Tests in Tasks 8 and 10; real host in Task 18.
2. The helper trusts nothing in a request: unknown fields, directive breaking strings, non loopback TCP, mismatched TLS pairs and a peer uid other than the deploy user or root are refused, and no request field becomes a path. Tests in Tasks 6 and 7.
3. A refused or failed reload leaves the previous files live. Test in Task 8 (`check_failure_restores_files`).

## Open questions

These need the owner. Each has the plan's recommendation; the plan proceeds on it unless the owner says otherwise.

1. `riku container` (`build`, `export`, remote `deploy` and `check-remote`) is a laptop side command (`Placement::Local`, placement.rs:27) that runs bare `docker`/`podman` (container_runtime.rs:18-31) and `ssh <host> "<engine> load -i <file>"` (container_runtime.rs:128-160). Through a riku deploy key that ssh command meets the forced command and is expected to fail (not run). Old Task 15 would move it onto the driver and a server subcommand. Recommendation: leave it out of this plan and let phase 3b's image streaming (`rk deploy --build`) replace it, deleting it there.
2. The old RSS limit (60 MB) came from a README line that no longer exists; the README now says about 10 MB supervisor and about 22 MB with nginx. Recommendation: the same rule as the binary, at most 10 percent over Task 1's measurement for both RSS rows, so the README figures stay true.
3. With the full allowance the binary reaches 10.25 MB, above README.md:51's "about 10 MB disk for Riku itself". Recommendation: keep the cap and let Task 17 restate the README figure from the measured size.

---

### Task 1: Footprint gate (S)

Status: done. The start commit's binary is 9,213,048 B, under the limit, and the RSS rows above hold its medians.

**Files:**
- Create: `benches/footprint.sh`
- Modify: `benches/README.md` (how to run it), this plan's ledger

`benches/benchmark.sh` reports whole MB by integer division (lines 35, 46), reads `which riku` rather than a given build (line 43), takes only the first matching pid (line 31) and never measures nginx, so it cannot hold a limit measured in KiB.

- [ ] **Step 1: Write** `benches/footprint.sh <riku binary>`: prints the binary size in bytes (`stat`), then starts `riku supervisor` on an empty temp `RIKU_ROOT` with nginx from the distribution package and its default `worker_processes`, waits 60 s, and prints the RSS in KiB of the supervisor and the sum over the nginx master and workers (`ps -o rss=`). Stops what it started. Prints the nginx version and worker count beside the numbers.
- [ ] **Step 2: Run** it three times on the Linux test server against a release build of the start commit; record the commit and the medians in the ledger's RSS rows, replacing the opt-level 3 figures.
- [ ] **Step 3: Gate.** If the start commit's binary is above 10,247,160 B, stop and report to the owner. Do not start Task 2.
- [ ] **Step 4: Commit** `Add a footprint script with KiB precision` (script, README, ledger).

### Task 2: Engine env and settings keys (M)

Status: done, binary 9,218,408 B. Was: no spawn sets any variable (bounded.rs:163, streamed.rs:34, login.rs:30, events.rs:39); `[engine]` knows only timeouts (crates/riku-config/src/settings.rs:86-101).

**Files:**
- Create: `crates/riku-container/src/engine/env.rs`, `env_tests.rs`
- Modify: `crates/riku-container/src/engine/identity.rs` (`Engine` gains the env), `bounded.rs`, `streamed.rs`, `login.rs`, `events.rs`, `probe.rs` (the probes take a program path today, lines 20, 26, 44, so they take the env too), `fake.rs` (record each spawn's env), `crates/riku-config/src/settings.rs`, `settings_tests.rs`, `crates/riku-deploy/src/containers/engine_open.rs`

**Interfaces:**
- Produces: `pub enum EngineMode { Rootless, Rootful }`; `EngineSettings` gains `mode: EngineMode` (absent means `Rootless`) and `host: Option<String>`. A bad value names the key and the file.
- Produces: `pub fn engine_env(mode: EngineMode, host: Option<&str>, uid: u32) -> Result<Vec<(String, String)>>`. Rootless: `XDG_RUNTIME_DIR=/run/user/<uid>`, refused unless it exists, is owned by `uid` and has mode 0700; `DBUS_SESSION_BUS_ADDRESS` when `$XDG_RUNTIME_DIR/bus` exists. Any mode: `DOCKER_HOST` from `host`. Rootful: nothing else.

**Security model:** `env.rs` doc comment: the env comes from the uid and `settings.toml` only, never from the caller's environment, which differs between sshd, the supervisor and init; the deploy user can edit `settings.toml`, and setting rootful gains it nothing without a socket it cannot reach.

- [ ] **Step 1: Failing tests:** `runtime_dir_wrong_mode_is_refused` (0755; error names the path and the mode), `host_becomes_docker_host`, `absent_mode_means_rootless`, `every_spawn_carries_env` (fake engine: one call per spawn path, probes included), `unknown_mode_is_refused`.
- [ ] **Step 2: Run** `cargo test -q -p riku-container env` and `cargo test -q -p riku-config settings`. Expected: FAIL.
- [ ] **Step 3: Implement.** `open_engine` builds the env from `settings.toml` and `getuid()`.
- [ ] **Step 4: Run** `cargo test -q --workspace`, `cargo clippy --workspace -q`, F. Expected: PASS, no warnings, within limit.
- [ ] **Step 5: Commit** `Give every engine call one computed env`.

### Task 3: Probe rootless through info (S)

Status: done, `REQUIRED_COMMANDS` now has 17 entries with `info`. The Docker rootful shape matches a real Docker 29.1.3; the rootless Docker and Podman fixtures stay unverified until Task 18.

**Files:**
- Create: `crates/riku-container/src/engine/info.rs`, `info_tests.rs`, fixtures `info_docker_rootless.json`, `info_podman_rootless.json`, `info_docker_rootful.json`
- Modify: `crates/riku-container/src/engine/probe.rs` (`info` joins REQUIRED_COMMANDS; the parallel probe from ffd7909 stays), `identity.rs`, `crates/riku-deploy/src/containers/engine_open.rs`

**Interfaces:**
- Produces: `Engine::is_rootless(&self) -> Result<bool>` from `info --format json`: docker `SecurityOptions` holding `name=rootless`, podman `host.security.rootless`. Parsed as data, no engine name.
- `open_engine` in `Rootless` mode refuses an engine that is not rootless, naming the mode, the socket it reached and `sudo riku init`.

**Security model:** `info.rs` doc comment: the field is trusted only as the engine's own report; a rootful engine reached in rootless mode is refused, never used.

- [ ] **Step 1: Failing tests:** `docker_rootless_shape_parses`, `podman_rootless_shape_parses`, `rootful_engine_refused_in_rootless_mode`. Fixture shapes follow upstream docs and are marked unverified until Task 18 replaces them with captures.
- [ ] **Step 2: Run** `cargo test -q -p riku-container info`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Refuse a rootful engine in rootless mode`.

### Task 4: Init system detection (S)

Status: done. Was: `/var/lib/riku` is already created root owned 0755 with a checked parent (crates/riku-cli/src/setup/var_root.rs:43-72), so `host.toml` goes there with no new directory code.

**Files:**
- Create: `crates/riku-config/src/host_file.rs`, `host_file_tests.rs`, `crates/riku-cli/src/setup/init_system.rs`, `init_system_tests.rs`
- Modify: `crates/riku-config/src/lib.rs` (a `HOST_FILE` constant beside `VAR_ROOT`), `crates/riku-cli/src/setup/var_root.rs` or `root_init.rs` (write it after `setup_var_root`)

**Interfaces:**
- Produces: `pub enum InitSystem { Systemd, OpenRc, Other }`; `pub fn detect_init(root: &Path) -> InitSystem`: systemd when `<root>/run/systemd/system` is a directory, OpenRC when `openrc-run` is on `PATH` and `<root>/run/openrc` is a directory, else `Other`.
- Produces: `/var/lib/riku/host.toml` (root owned, 0644) with `init = "systemd" | "openrc" | "other"`, written with `check_write_dir` on `/var/lib/riku`; `pub fn read_host(path: &Path) -> Result<Option<HostFacts>>` for doctor.

**Security model:** `host_file.rs` doc comment: written only by root into a root owned directory, read by anyone; a missing file means init has not run, not a default.

- [ ] **Step 1: Failing tests:** `systemd_dir_means_systemd`, `openrc_needs_both_signals` (binary present, no `/run/openrc`: `Other`), `host_file_round_trips`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli init_system` and `cargo test -q -p riku-config host_file`. Expected: FAIL.
- [ ] **Step 3: Implement**; root init writes the file on every run.
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Detect the init system by capability`.

### Task 5: One root owned binary (L)

Status: done, binary 9,222,136 B. Was: Opus review: every way the deploy user reaches riku changes path. Runs on every host, with or without a router or engine.

Today root init stages a root owned temp copy (root_init.rs:46-65) and runs `riku init --user-steps` from it as the deploy user (root_init.rs:81-94), and that child installs `~/.local/bin/riku` by copy and rename (binary.rs:47-91, from init.rs:73). This task makes root install `/usr/local/bin/riku` with the same copy and rename, run the user steps from there, and drop both the temp stage and the user side install.

**Files:**
- Modify: `crates/riku-cli/src/setup/binary.rs`, `binary_tests.rs` (target `/usr/local/bin/riku`; `replace_binary` kept with the stage file in `/usr/local/bin`; `print_path_note`, `try_add_to_shell_config`, `append_once`, `get_user_install_directory`, `INSTALL_DIR` removed), `root_init.rs` (install first, `stage_binary` removed, user steps run `/usr/local/bin/riku`), `init.rs` (user steps install nothing; non root init requires `/usr/local/bin/riku`, else names `sudo riku init`), `system_service.rs` (line 151), `user_service.rs` (lines 9, 26), `deploy_keys.rs` (line 96), `git_hook.rs` (line 33), `guidance.rs` (line 34), `crates/riku-cli/src/apps/create.rs` (line 55), `crates/riku-cli/src/git/repo.rs` (lines 20-25), `crates/riku-deploy/src/supervisor_ctl.rs` (lines 36-63), `crates/riku-remote/src/remote/dashboard/remote_script.rs` (line 32), `scripts/dashboard-tunnel.sh` (line 23), `scripts/install.sh` (server install as root only; `--rk` unchanged), `crates/riku-cli/src/doctor/checks.rs` (lines 85-119), README files in the touched directories

**Interfaces:**
- Root init installs the running binary to `/usr/local/bin/riku` (root owned, 0755, parent checked with `check_write_dir`) before anything else. Every unit, hook and forced command it writes names that path.
- doctor: `/usr/local/bin/riku` root owned and not group or world writable; no forced command, unit or hook naming another path.

**Security model:** `binary.rs` doc comment: root writes only `/usr/local/bin/riku`; root never executes or trusts a file from the deploy user's tree.

- [ ] **Step 1: Failing tests:** `forced_command_names_root_binary`, `hooks_name_root_binary` (every hook template, no `$HOME/.local/bin`), `install_runs_without_router` (temp root, no nginx, no engine), `relaunch_skips_home_binary`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli binary` and `cargo test -q -p riku-deploy supervisor_ctl`. Expected: FAIL.
- [ ] **Step 3: Implement.** Grep the tree for `.local/bin` afterwards; only `rk` install text may keep it.
- [ ] **Step 4: Run** `cargo test -q --workspace`, `./tests/deploy-smoke/quick-test.sh myapp node`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Install riku once as a root owned binary`.

### Task 6: Route schema and builder (M)

Status: done, binary 9,221,816 B. Gate is `check_nginx_route` and `check_caddy_route`; `build_route` takes the router. Was: Opus review: this is the helper's input boundary.

**Files:**
- Create: crate `crates/riku-route/` (`Cargo.toml`, `src/lib.rs`, `src/schema.rs`, `src/schema_tests.rs`, `src/check.rs`, `src/check_tests.rs`), `crates/riku-deploy/src/router/route_build.rs`, `route_build_tests.rs`
- Modify: workspace `Cargo.toml`, `crates/riku-deploy/Cargo.toml`, `crates/riku-util/src/route_plan.rs` (serde for `HttpsPolicy` if the route embeds it)

**Interfaces:**
- Produces: `pub struct Route` (spec "Route schema (v1)"), `#[serde(deny_unknown_fields)]`, built around `RoutePlan`'s domains, policy and backends, with `Unix` backends replaced by copy numbers. No field is a path.
- Produces: `pub fn check_route(app: &AppName, route: &Route) -> Result<()>`: directive breaking rule of `riku-nginx` `host.rs:20`, `Domain` rules, loopback TCP only, TLS PEM parsed as one chain and one matching key, snippet through `snippet_grammar`, `ALLOWED_DIRECTIVES` and `snippet_limits`, snippet refused for Caddy.
- Produces: `pub fn build_route(app: &AppName, env: &HashMap<String, String>, paths: &RikuPaths) -> Result<Route>` mapping the env keys the spec lists; any other key the nginx generator reads is mapped or refused by name. `RIKU_NGINX_CACHE_PATH` is refused with a message saying the helper picks the path. TLS text is read from the acme.sh output the deploy user already has (crates/riku-nginx/src/ssl.rs:30-152).

**Security model:** `riku-route` crate doc: every field is untrusted; `check_route` is the only gate and the helper calls it again on receipt. The deploy side calls it for an early error only.

- [ ] **Step 1: Failing tests:** `unknown_field_is_refused`, `directive_break_in_domain_is_refused`, `public_tcp_backend_is_refused`, `mismatched_tls_pair_is_refused`, `filtered_snippet_outside_allowlist_is_refused`, `env_maps_to_route` (one ENV with every mapped key gives the expected route).
- [ ] **Step 2: Run** `cargo test -q -p riku-route` and `cargo test -q -p riku-deploy route_build`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Define the route the router helper accepts`.

### Task 7: Helper socket, protocol and subcommand (M)

Status: done, binary 9,329,080 B. Linux tests pass as root (fork and setuid foreign peer tests) and as a plain user. Deferred: the Status reply carries only the version; router and installed apps go to Task 16. Apply does not run the gate yet; Task 8 must call `check_nginx_route` or `check_caddy_route` there. Opus review: root process facing the deploy user.

**Files:**
- Create: `crates/riku-route/src/protocol.rs`, `protocol_tests.rs`, `src/server.rs`, `server_tests.rs`, `src/listen.rs` (socket activation or own socket), `crates/riku-deploy/src/router/helper_client.rs`, `helper_client_tests.rs`, `crates/riku-cli/src/router_helper.rs`
- Modify: `crates/riku-args/src/cli.rs`, `crates/riku-args/src/placement.rs` (`router-helper` as `Server`), `crates/riku-cli/src/dispatch/mod.rs`, `crates/riku-config/src/lib.rs` (`ROUTER_SOCKET = "/run/riku-router.sock"`)

**Interfaces:**
- Produces: `pub enum Request { Apply { app, route }, Remove { app }, Status }` with `v: 1`; `pub enum Reply { Ok { reloaded: bool }, Failed { stage: Stage, error: String }, Status { .. } }`, `pub enum Stage { Schema, Render, Check, Reload }`.
- Server: one request per connection, one connection at a time, request at most 1 MiB then EOF, `SO_PEERCRED` through `nix` refusing any uid but the deploy user's and 0, exits after 60 s idle when socket activated (`LISTEN_FDS`/`LISTEN_PID`), otherwise creates `/run/riku-router.sock` itself (root, the deploy user's primary group, 0660) and stays running.
- Client: `pub fn send(request: &Request) -> Result<Reply>`, giving up after 60 s.
- `riku router-helper` runs the server; `Apply` replies `Failed { stage: Render }` until Task 8.

**Security model:** `server.rs` doc comment: runs as root; the peer uid check and the socket's group are both required; the deploy user's primary group holds only the deploy user (the nginx worker uses `riku-sockets`, 42d11f4; `caddy` joins nothing, c087210); input is bounded in size and time; one request is served at a time.

- [ ] **Step 1: Failing tests:** `oversize_request_is_refused`, `other_uid_is_refused` (peer check as a pure function over a credential value), `status_round_trips` (server on a temp socket), `non_root_refuses_to_start`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route protocol server` and `cargo test -q -p riku-deploy helper_client`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS (record the helper's idle RSS in the ledger). Expected: PASS, within limit.
- [ ] **Step 5: Commit** `Add the router helper socket and protocol`.

### Task 8: nginx through the helper (L)

Status: done, binary 9,383,376 B; Docker e2e passes through a real helper. Was: root side done (binary 9,364,480 B): the helper renders, checks, installs and restores nginx sites; every site kind passes real `nginx -t` 1.22.1 on Linux. Deploy side still needed: `router.rs` and `router/reload.rs` send Apply and Remove, `nginx_unit.rs` and the direct reload go, the stray check in `host_check.rs` goes, destroy sends Remove, and the old ENV path in riku-nginx is removed. Opus review: Review Focus 1 and 3.

Owner decisions 2026-11-05: the total cache cap is a `settings.toml` setting with a 10 GB default (today a 100 GB constant `TOTAL_CACHE_GB`); root init fetches the Cloudflare ranges into `/etc/riku/cloudflare.conf` and doctor warns when the file is old; the helper never fetches. `catch_all` renders only with cache prefixes and not wsgi, as before.

Carried from the Task 6 review:
- Call `ensure_ssl_certificates` before `build_route`; it runs in riku-nginx/src/context.rs:180 today, so a first https only deploy would otherwise build no TLS.
- `apply_nginx` calls `check_nginx_route` and writes only the normalized PEM `check_tls` returns.
- Cache: `keys_zone` is sized by `size_gb` (nginx_common.conf.tera:25), so 100 means a 100 GiB shared zone, and no `max_size` bounds the disk. Use a small fixed zone, render `max_size`, cap the total.
- acme.sh renews as the deploy user with no reload hook, so helper held certificates go stale until the next deploy. Own the renewal path here.
- The Cloudflare template includes `/etc/nginx/cloudflare.conf`, while cloudflare.rs:14 writes `<riku_root>/cloudflare.conf`. Never include the deploy owned file (Review Focus 1), and the helper does no network fetch.
- Task 7 enforces the 1 MiB request limit before parsing; `check.rs` has no list or string caps of its own.

Today the deploy user writes `<nginx root>/<app>.conf.new`, runs `nginx -t` and renames (crates/riku-nginx/src/install.rs:1-7), keeps TLS files in the same tree (ssl.rs:35-36), and asks for a reload through the trigger file (crates/riku-deploy/src/router/reload/nginx_unit.rs). All of that moves into the helper.

**Files:**
- Create: `crates/riku-route/src/nginx_apply.rs`, `nginx_apply_tests.rs`
- Modify: `crates/riku-nginx/src/context.rs`, `context_cache.rs`, `template.rs`, `static_site.rs` (render from a `Route`; socket, static, ACME and cache paths passed in by the caller), `install.rs` (writes into a directory the caller names), `crates/riku-deploy/src/router.rs`, `router/reload.rs` (send `Apply`/`Remove`; nginx_unit.rs and the direct nginx reload removed), `crates/riku-nginx/src/host_check.rs` (stray check removed; the helper owns the directory)

**Interfaces:**
- Produces: `fn apply_nginx(app: &AppName, route: &Route) -> Result<Reply>`: `check_route`, render, write `/etc/riku/nginx/<app>.conf` and `/etc/riku/certs/<app>/{fullchain.pem,key.pem}` (directory 0700, key 0600) through write then rename, cache at `/var/cache/riku/nginx/<app>`, `nginx -t` as root, `systemctl reload nginx` or `nginx -s reload` as root; on `check` or `reload` failure restore the previous files before replying.
- Deploy side: `router::apply` sends `Apply` and maps `Failed` to an error carrying the stage and the router's own message. Static publishing to `/var/lib/riku/static/<app>` (117b33c) stays on the deploy side.

**Security model:** `nginx_apply.rs` doc comment: root writes only under `/etc/riku` and `/var/cache/riku`, every path built from a checked `AppName` and riku-config constants, never from request text; TLS material is written from PEM text, never read from a path.

- [ ] **Step 1: Failing tests** (mock `nginx` on a temp `PATH`, temp `/etc/riku` root): `render_matches_generator` (same input gives today's generator output, paths aside), `check_failure_restores_files`, `access_log_snippet_never_written`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route nginx_apply`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; with Docker, render and `nginx -t` in `nginx:1.27`; F. Expected: PASS.
- [ ] **Step 5: Commit** `Render and reload nginx in the router helper`.

### Task 9: Caddy through the helper (M)

Status: partly done. c087210 already moved the sites to `/etc/riku/caddy` and took `caddy` out of the deploy group. Left: root ownership of that directory, the admin socket and the reload.

**Files:**
- Create: `crates/riku-route/src/caddy_apply.rs`, `caddy_apply_tests.rs`
- Modify: `crates/riku-cli/src/setup/caddy_host.rs` (`prepare_site_root` owner becomes root, group `caddy` and 2750 kept; admin on `unix//run/caddy/admin.sock`, refusing with the lines to add when a global block cannot be merged), `crates/riku-caddy/src/lib.rs` (stale doc at lines 1-5), `crates/riku-deploy/src/router/caddy.rs` and `router/reload.rs` (send `Apply`; the deploy user's `caddy reload` at reload.rs:39-57 removed), `crates/riku-cli/src/setup/system_service.rs` (`CADDY_SITE_ROOT` leaves `ReadWritePaths`, line 52)

**Interfaces:**
- Produces: `fn apply_caddy(app: &AppName, route: &Route) -> Result<Reply>`: write `/etc/riku/caddy/<app>.caddy` (0640) with the existing `riku-caddy` renderer, `caddy reload --config /etc/caddy/Caddyfile --address unix//run/caddy/admin.sock` as root, restore on failure.

**Security model:** `caddy_apply.rs` doc comment: the admin API moves off localhost TCP to a socket only `caddy` and root reach, so no local process controls routes.

- [ ] **Step 1: Failing tests:** `caddy_reload_failure_restores`, `snippet_for_caddy_is_refused`, `unmergeable_global_block_refused`.
- [ ] **Step 2: Run** `cargo test -q -p riku-route caddy_apply` and `cargo test -q -p riku-cli caddy_host`. Expected: FAIL.
- [ ] **Step 3: Implement.** Admin socket syntax and the `--address` flag are unverified; Task 18 confirms them.
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Reload Caddy through the router helper`.

### Task 10: Install the helper (M)

Status: still needed. Opus review: root installs units and the nginx include. The include is already root owned and carries the catch all (nginx_host.rs:55-72, f3fee94); only its target changes.

**Files:**
- Create: `crates/riku-cli/src/setup/helper_install.rs`, `helper_install_tests.rs`
- Modify: `crates/riku-cli/src/setup/root_init.rs` (helper right after the router host, on every host with a router), `router_units.rs` (nginx path and reload units removed; `other_path_units` disables them on hosts that have them), `system_service.rs` (`reload_trigger` removed, lines 72-77), `nginx_host.rs` and `crates/riku-nginx/src/host.rs` (include `/etc/riku/nginx/*.conf`; `/etc/riku/nginx` created root owned 0755 under the checked `/etc/riku`), `crates/riku-config/src/lib.rs` (`nginx_reload_trigger` and the `nginx_root` include use removed)

**Interfaces:**
- init writes `riku-router-helper.socket` and `.service` running `/usr/local/bin/riku router-helper` on systemd, or `/etc/init.d/riku-router-helper` on OpenRC; on other init systems it prints the command for the operator to start by hand. It starts the helper and writes the include as `include /etc/riku/nginx/*.conf;`.

**Security model:** `helper_install.rs` doc comment: the helper runs only the root owned `/usr/local/bin/riku` (Task 5); root never executes a file from the deploy user's tree.

- [ ] **Step 1: Failing tests:** `include_names_etc_riku`, `units_render_per_init` (systemd socket unit carries `SocketMode=0660` and the deploy group; OpenRC script starts before `riku`; `Other` prints the start command).
- [ ] **Step 2: Run** `cargo test -q -p riku-cli helper_install`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`; F with idle RSS. Expected: PASS, within limit.
- [ ] **Step 5: Commit** `Install the router helper and move nginx off the user include`.

### Task 11: Port refusal below 1024 (S)

Status: still needed.

**Files:**
- Modify: `crates/riku-deploy/src/containers/spec.rs`, `spec_tests.rs`, the container deploy entry that runs before any copy stops

**Interfaces:**
- In `Rootless` mode, a non routed host port below `/proc/sys/net/ipv4/ip_unprivileged_port_start` is refused before anything stops, naming the service, the port and the sysctl. Routed copies are untouched (ephemeral ports, ports.rs:12-23).

**Security model:** none new; the sysctl is read, never written.

- [ ] **Step 1: Failing tests:** `public_port_80_refused_rootless`, `routed_port_unaffected` (sysctl path injected).
- [ ] **Step 2: Run** `cargo test -q -p riku-deploy spec`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Refuse low host ports in rootless mode`.

### Task 12: systemd rootless init (L)

Status: still needed. Opus review: root steps acting for the deploy user.

**Files:**
- Create: `crates/riku-cli/src/setup/rootless/mod.rs`, `checks.rs`, `subids.rs`, `systemd_steps.rs`, sibling `_tests.rs`
- Modify: `crates/riku-cli/src/setup/root_init.rs`, `deploy_user.rs`

**Interfaces:**
- All checks before any change: no engine means skip every step silently; subuid and subgid range (add `<user>:<start>:65536` with a start overlapping no entry, `usermod --add-subuids/--add-subgids` when present, else an atomic write), `newuidmap`/`newgidmap`, `unshare --user true` as the deploy user, `pasta` or `slirp4netns`, kernel 5.13 or `fuse-overlayfs` plus `/dev/fuse`, cgroup v2.
- systemd: `loginctl enable-linger`, `user@.service.d/riku-delegate.conf` (`Delegate=cpu cpuset io memory pids`), docker's `dockerd-rootless-setuptool.sh install` as the deploy user and its user `docker.service`; podman nothing more. Restart `user@<uid>.service` after the `riku-sockets` group change so the user manager carries it.
- Write `engine.mode` and `engine.host` through `DeployUser`, refused unless the helper answers `status` (Task 7). Verify as the deploy user: rootless reported, a `--memory 64m` container shows the limit in `stats`.
- Fallback: OpenRC with docker only, `Other`, or owner `rootful`: write `rootful` and print the one line reason (grant text in Task 15).

**Security model:** `rootless/mod.rs` doc comment: root edits only `/etc/subuid`, `/etc/subgid` and the systemd drop in; everything in the deploy user's tree is written as that user; riku never adds the deploy user to an engine group.

- [ ] **Step 1: Failing tests:** `subuid_start_avoids_overlap`, `no_engine_runs_no_step` (no output line mentions an engine), `mode_refused_without_helper`, `docker_on_openrc_falls_back`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli rootless`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS (lingering user manager recorded as add-on cost). Expected: PASS.
- [ ] **Step 5: Commit** `Set up a rootless engine on systemd hosts`.

### Task 13: OpenRC podman init (M)

Status: still needed. On systemd the tmpfiles entry recreates `/run/riku-sockets` at boot (socket_group.rs:29-31, 95); OpenRC has no such entry, so `riku-session` takes that job too.

**Files:**
- Create: `crates/riku-cli/src/setup/rootless/openrc_steps.rs`, `openrc_steps_tests.rs`, the `riku-session` template
- Modify: `crates/riku-cli/src/setup/rootless/mod.rs`, `socket_group.rs` (no tmpfiles on OpenRC)

**Interfaces:**
- Requires `rc_cgroup_mode="unified"` in `/etc/rc.conf`, refusing with the line. Writes `/etc/init.d/riku-session` (`checkpath -d -m 0700 -o <user>:<group> /run/user/<uid>`, `checkpath -d -m 2750 -o <user>:riku-sockets /run/riku-sockets`, `riku-user` cgroup with `+cpu +memory +pids +io` chowned per the kernel delegation rule, leaf `riku-user/supervisor`), `rc-update add` and `rc-service start`.

**Security model:** `openrc_steps.rs` doc comment: root creates and delegates exactly one cgroup subtree and two runtime directories; it refuses an existing directory with another owner or mode.

- [ ] **Step 1: Failing tests:** `legacy_cgroup_mode_refused`, `session_script_renders` (both checkpath lines and leaf creation present).
- [ ] **Step 2: Run** `cargo test -q -p riku-cli openrc_steps`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS. Expected: PASS.
- [ ] **Step 5: Commit** `Set up rootless podman on OpenRC hosts`.

### Task 14: Supervisor under the user manager and supervise-daemon (M)

Status: still needed. The user unit exists for non root installs (user_service.rs:12-97) but is never used by a root init; the system unit is the only path today (system_service.rs:142-177).

**Files:**
- Modify: `crates/riku-cli/src/setup/user_service.rs`, `system_service.rs`, `systemd.rs`, `rootless/openrc_steps.rs`, `guidance.rs`, sibling tests

**Interfaces:**
- systemd rootless: `~/.config/systemd/user/riku.service` written by the deploy user step with `ExecStart=/usr/local/bin/riku supervisor`, `systemctl --user --machine=<user>@ enable --now riku` by root, `After=docker.service` for docker, no `NoNewPrivileges` and no mount sandboxing; `MemoryMax` and `CPUQuota` kept until Task 18 decides. System `riku.service` disabled on switch, re-enabled on a return to rootful.
- OpenRC: `/etc/init.d/riku`, `supervise-daemon`, `command=/usr/local/bin/riku`, `command_user=<user>`, `need riku-session riku-router-helper`, `start_post` writes its pid to `riku-user/supervisor/cgroup.procs`.

**Security model:** unit and script doc comments state why `NoNewPrivileges` is dropped (setuid `newuidmap`) and that limits come from the delegated cgroup.

- [ ] **Step 1: Failing tests:** `rootless_unit_drops_no_new_privileges`, `switch_disables_system_unit`, `openrc_script_runs_as_user`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli user_service system_service openrc`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F with idle RSS. Expected: PASS.
- [ ] **Step 5: Commit** `Run the supervisor as the deploy user in rootless mode`.

### Task 15: Rootful fallback grant message (S)

Status: still needed.

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

### Task 16: doctor (M)

Status: still needed. doctor today has `container_check.rs`, `router_check.rs`, `settings_check.rs` and `checks.rs`; none reports engine mode, the helper or the init system.

Owner decisions 2026-11-05, part of this task:
- No automatic supervisor restart on upgrade. doctor fails when the running supervisor's `/proc/<pid>/exe` is not `/usr/local/bin/riku` or ends in `(deleted)`, and root init prints `sudo systemctl restart riku` (or the OpenRC equivalent) when it replaced the binary. Revisit only if Task 18 shows workers survive a supervisor restart.
- `riku doctor --fix`, as the deploy user, rewrites riku-marked `authorized_keys` lines (`RIKU_KEY_MARKER`) and `repos/*/hooks/post-receive` that name another riku path so they name `/usr/local/bin/riku`, as phase 1 did for the old forced command keys. Atomic writes, own files only, no root. Tests: a stale key and a stale hook are rewritten, a non riku key line is left byte for byte.

**Files:**
- Create: `crates/riku-cli/src/doctor/rootless_check.rs`, `rootless_check_tests.rs`, `helper_check.rs`, `helper_check_tests.rs`
- Modify: `crates/riku-cli/src/doctor/mod.rs`, `container_check.rs`, `router_check.rs` (the missing include and stray conf reports change to the `/etc/riku` layout), `checks.rs` (`systemd_service` at lines 121-151 learns the user unit and OpenRC)

**Interfaces:**
- No engine: only the binary, helper and init system lines (binary lines from Task 5). With an engine, every line in the spec's doctor section, rootful as a warning with `grant_text`. Helper: socket answers `status`; no file the router loads is writable by the deploy user.

**Security model:** doctor runs as the deploy user and only reads; it calls the helper's `status` and nothing else.

- [ ] **Step 1: Failing tests:** `no_engine_prints_no_engine_line`, `rootful_shows_grant`, `helper_down_fails`.
- [ ] **Step 2: Run** `cargo test -q -p riku-cli doctor`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -q --workspace`, F. Expected: PASS.
- [ ] **Step 5: Commit** `Report rootless and router helper state in doctor`.

### Task 17: Docs and CHANGELOG (M)

Status: still needed for this plan's changes. The 3a CHANGELOG items (include, snippet allowlist, container detection, ports form, logs glob) were written in 07d39f5 and later 3a commits.

**Files:**
- Modify: `docs/docs/runtimes.md`, `nginx.md`, `caddy.md`, `systemd.md`, `settings.md`, `doctor.md`, `installation.md`, `cli.md`, `README.md` (install path, footprint figures from the ledger, add-on costs stated apart), `CHANGELOG.md`, `docs/superpowers/ROADMAP.md`, per directory READMEs the tasks touched

- [ ] **Step 1: Docs:** the one binary at `/usr/local/bin/riku` and that updates need root; rootless mode per init system and engine, OpenRC docker deferred, the grant and its warning, `engine.mode` and `engine.host`, the router helper and `/etc/riku`, starting the helper by hand on other init systems, Caddy's admin socket, low port refusal, footprint with the engine's cost stated as the add-on's.
- [ ] **Step 2: CHANGELOG**, under Unreleased: riku lives at `/usr/local/bin/riku`, root owned; rootless by default when an engine is present; root router helper and `/etc/riku`; `RIKU_NGINX_CACHE_PATH` removed; Caddy admin on a unix socket; host ports below the unprivileged start refused; reinstall and redeploy after upgrading.
- [ ] **Step 3: Run** `NO_MKDOCS_2_WARNING=1 uvx --with mkdocs-material --with mkdocs-minify-plugin mkdocs build --strict -d <scratch>/site`. Expected: no warnings.
- [ ] **Step 4:** ROADMAP status lines with commits.
- [ ] **Step 5: Commit** `Document the rootless engine and router helper`.

### Task 18: Real host check (L)

Status: still needed. Three hosts: systemd with docker, systemd with podman, OpenRC (Alpine VM) with podman. Plus one systemd host with neither router nor engine, one OpenRC host with docker only, and one runit or sysvinit VM. Results go in the ledger; each failure is a bug fixed before Task 19, never a skipped item. Container, volume and image names use a fresh prefix and are removed afterwards, as in the 3a checks.

- [ ] Footprint: F on the no engine host within the ledger limits; helper idle RSS on OpenRC recorded; lingering user manager recorded as add-on cost.
- [ ] One binary, on a host with a router and on the host with neither: `git push`, `rk` commands and the supervisor run `/usr/local/bin/riku`; `/usr/local/bin` is on the deploy user's `PATH` over ssh and sudo.
- [ ] Init from clean on each of the three hosts; refusals on cgroup v1, missing network helper, missing `newuidmap`.
- [ ] No engine host: init and doctor print nothing about engines.
- [ ] OpenRC docker only and runit or sysvinit: init sets rootful, prints the grant, the warning and the reason, runs nothing; on runit or sysvinit init prints the helper start command and deploys work once the operator starts it.
- [ ] Unverified in the spec, each confirmed or the design changed: docker `SecurityOptions` `name=rootless` and podman `host.security.rootless` (captured JSON replaces the Task 3 fixtures); podman's systemd cgroup manager uses the user bus; kernel 5.13 versus 5.11 for overlay; `Delegate=` needed for `--cpus`; `systemctl --user --machine=<user>@` needs systemd 248; the user manager picks up `riku-sockets` after a restart; podman falls back to cgroupfs without a bus and its containers sit under `riku-user`; user unit mount sandboxing needs an extra user namespace; Caddy `admin unix//...` syntax and `caddy reload --address unix//...`.
- [ ] `riku-session` recreates `/run/user/<uid>` and `/run/riku-sockets` after reboot; the OpenRC supervisor sits in `riku-user/supervisor` and is respawned after `kill -9`.
- [ ] Deploy through git push and through `rk` in a session without `XDG_RUNTIME_DIR`; a uWSGI app answers through nginx under the user manager.
- [ ] A supervisor restart leaves containers running; `--memory` shows in `stats`; containers are outside the supervisor unit's cgroup (decides `MemoryMax`/`CPUQuota`, Task 14).
- [ ] `riku.public` port 80 refused with the sysctl named.
- [ ] A deploy user `.conf` with `access_log /etc/cron.d/x;` is never loaded; a filter adding `access_log` is refused; the Caddy admin port on localhost is closed.

### Task 19: Final review (S)

- [ ] One Opus review of the whole branch against the spec, Review Focus and rule 8 doc comments; fix findings in one round, rerun `cargo test -q --workspace`, `cargo clippy --workspace -q` and F.
- [ ] Confidential name search over the branch diff. Ask the owner before push.

## Run order

Task 1 first and alone; nothing starts if its gate fails. Then 2, 3, 4 and 5 (independent, own worktrees). Then 6, 7, 8, 9, 10 in order (the router helper; 10 needs 5). Then 11. Then 12 (needs 2, 3, 4, 7), 13, 14 in order. 15 after 12, 16 after 15, 17 after 16, 18 after all, 19 last. Opus reviews for 5, 6, 7, 8, 10, 12 and the final review; Sonnet for the rest. At most 4 agents at a time.
