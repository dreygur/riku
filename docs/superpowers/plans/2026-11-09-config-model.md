# Config Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Problem:** Riku's own settings live as `RIKU_*` lines in each app's `envs/<app>/ENV` and in loose server files (`<riku root>/ROUTER`, `<riku root>/CONTAINER_ENGINE`). A developer cannot commit app settings next to the code, and riku settings mix with app secrets.

**Goal:** First the two fixes from the spec section "Before both projects: two small fixes" (runtime plugins installed as bundles are detected; the plugin docs match the code). Then riku settings move into `settings.toml` (server) and an optional `riku.toml` (per app), with a `riku doctor --fix` migration.

**Architecture:** One loader module, `riku_config::app_config`, returns an app's merged env (`load_env`) and process list (`load_processes`). Every place that today reads `envs/<app>/ENV` or the Procfile for riku keys calls the loader instead. The env map is built from three layers, later wins: the repo `riku.toml` (in the checkout at `<app_root>/<app>/riku.toml`), then legacy `RIKU_*` table keys still in `ENV`, then the server override at `<env_root>/<app>/riku.toml`. App keys in ENV that are not riku keys are included as today. A static key table maps each `riku.toml` key to the `RIKU_*` env key consumers read today. This table is a transitional adapter: Project 2 adds typed fields to `AppConfig` instead of new env keys. Router and container engine program move into `settings.toml`; the old files are still read until doctor migrates them.

**Tech Stack:** Rust workspace, `toml = "1"` (riku-config already depends on it). No serde; tables are walked by hand as `settings.rs` does. New crate edge: riku-config depends on riku-util (riku-util has no riku dependencies, so no cycle).

**Spec:** `docs/superpowers/specs/2026-11-09-developer-experience-design.md`, the two small fixes and Project 1. The server override is edited with `riku settings set` as recorded in the spec section "Calls made by Claude"; `riku config set` keeps setting app env only.

## Non goals

- Stripping riku keys from the app process. Today riku keys reach the app through `LIVE_ENV`; this plan keeps that, so nothing regresses, but the leak is not closed here.
- `[routing] mode`, `port`, fallback domain: Project 2.

## Out of scope versus spec

The spec moves all server wide `RIKU_*` values into settings.toml. This plan moves only the router name and the container engine program. Deferred: `RIKU_PLUGIN_TIMEOUT` (process env with 9 call sites, some without `RikuPaths`), `RIKU_ACME_ROOT`, `RIKU_DASHBOARD_*`, `RIKU_HEALTH_PORT`, `RIKU_NS_*`, `RIKU_ISOLATION_ROOT`, fallback domain.

## Alternatives

- Typed `AppSettings` passed to every consumer: cleanest end state, but changes every consumer in one step. Rejected for now; the loader makes it a later, local change.
- Migrating consumer by consumer: two config paths coexist for a long time. Rejected.

## Risks

- A repo that already has an unrelated `riku.toml` now fails to deploy. The error names the file and says to rename it.
- Downgrading riku after `riku doctor --fix` is unsupported: old binaries refuse unknown settings.toml sections (`settings.rs:113`) and no longer find keys moved out of ENV. The release note says so. Doctor writes `ENV.bak` before editing an ENV.
- `settings::save` patches the parsed table, so operator comments in settings.toml are lost on save. Documented in `settings.md`.

## Global Constraints

- `.claude/CLAUDE.md` rules apply: no `?` (explicit `match`/`return Err`), tests in sibling `_tests.rs` via `#[path]`, files under about 200 lines, doc comments on public items, no emoji, no em dashes.
- Read `docs/CODEMAP.md` first; any commit that adds, moves or changes a module contract updates it.
- Native file tools only (Read, Edit, Write; Grep/Glob or shell grep for search). No python, perl or other interpreters.
- Commits: imperative subject under 50 characters, no AI attribution, `git commit -F <file> --no-verify`.
- Confidential: never write the owner's server, organisation or project names. Use `prod`, `edge.example`, `myapp`.
- Per task: `cargo check --workspace` and `cargo test -p <touched crates>`. Full suite, clippy and the deploy smoke tests run once in the final gate.
- Plugin protocol env (`RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`, `RIKU_HOOK`, `RIKU_PLUGIN_*`, `RIKU_ADDON_*`, `RIKU_DETECTED_RUNTIME`) does not change.
- An upgraded server that has not run `riku doctor --fix` and has no `riku.toml` deploys with the same env map and routes as before.
- Parse errors name file, key and line, and never echo the value.
- Files riku writes that may hold secrets keep the original file mode (0600 when new); new directories 0700.

## Review Focus

1. A Caddy server upgrades without running doctor: the router stays Caddy (`ROUTER` file still read). Task 3.
2. `riku.toml` with a misspelled key (`domain`) or a wrong type (`https_only = "yes"`): push refused naming file, key and line. Task 4.
3. A secret or a server only key (`secrets.ghcr_token`, `limits.max_memory_mb`) in the repo `riku.toml`: refused. Allowed in the server override. Task 4.
4. A repo value built to escape (`document_root = "../../etc"`, a domain containing `;` or a newline): refused at parse. Task 4.
5. `riku.toml` sets `domains` and `[cron]`: the router config contains the domain and the cron job is scheduled. Task 6.
6. `riku doctor --fix` run twice: second run changes nothing. Task 8.

---

### Task 1: Detect scaffolded runtime bundles

**Files:**
- Modify: `crates/riku-plugins/src/runtime.rs:62-108` (`discover`)
- Test: `crates/riku-plugins/src/runtime_tests.rs`

**Interfaces:**
- Consumes: `crate::bundles::find_bundles(plugin_root) -> Vec<(PathBuf, PluginManifest)>` (`bundles.rs:12`). Manifests are already validated on load: `entry` cannot be absolute or contain `..` (`crates/riku-bundle/src/manifest.rs:165-200`); `PluginManifest::entry_path(bundle_dir)` (line 205) resolves it.
- Produces: `discover` also returns `RuntimePlugin { name: manifest.name, path: manifest.entry_path(bundle_dir) }` for each runtime bundle whose entry is an executable file. Plain file plugins unchanged; result sorted by name. When a bundle name equals a plain file plugin name, the plain file wins and a warning names both paths.

- [ ] **Step 1: Failing tests:** `discovers_runtime_bundle_from_scaffold` (manifest `name="mynode"`, `version="0.1.0"`, `type="runtime"`, `api=1`, `entry="bin/mynode"`, executable `bin/mynode`; one plugin named `mynode`, path ends `mynode/bin/mynode`), `skips_non_runtime_bundle` (`type="addon"`, empty), `plain_file_wins_name_clash` (file `plugins/mynode` and bundle `mynode`; one plugin, path is the file).
- [ ] **Step 2:** `cargo test -p riku-plugins runtime` fails.
- [ ] **Step 3:** Implement in `discover` after the file scan. New code uses no `?`.
- [ ] **Step 4:** `cargo test -p riku-plugins` passes.
- [ ] **Step 5:** Commit `Detect runtime plugins installed as bundles`.

### Task 2: Make the plugin docs match the code

**Files:**
- Modify: `docs/docs/plugins.md`, `PLUGIN_PROTOCOL.md:274-279`, `docs/docs/plugin-bundles.md:86` (runtime bundles now detected)

- [ ] **Step 1:** Fix each claim, reading the code line first:
  - `plugins.md:128`: a post build failure aborts the deploy (`crates/riku-plugins/src/manager.rs:145`).
  - `plugins.md:30-31`: no `container` plugin; java and clojure are opt in via `--plugins` (`crates/riku-cli/src/apps/install_plugins.rs:16-22`).
  - `plugins.md:145-197`: remove the section claiming riku runs unknown subcommands as server plugins (`riku hello`); the CLI refuses unknown subcommands.
  - `plugins.md` sections around 270-280 (`riku open`) and from 366 through the tunnel section at 401: remove `riku open`, `riku tunnel`, `riku backup` as client plugins; list the ten commands a client plugin can replace, from `crates/riku-remote/src/routing.rs:12-24`.
  - `plugins.md:327`: `riku plugin list` lists client plugins.
  - `PLUGIN_PROTOCOL.md:274-279`: gate mode runs as observe and cannot veto (`crates/riku-plugins/src/events/bus.rs:71-77`).
- [ ] **Step 2:** `mkdocs build --strict` from the repo root (`mkdocs.yml` is there) if mkdocs is installed; otherwise say so in the report.
- [ ] **Step 3:** Commit `Make the plugin docs match the code`.

### Task 3: Server settings in settings.toml

**Files:**
- Modify: `crates/riku-config/src/settings.rs` (split into `settings/mod.rs`, `settings/engine.rs`, `settings/router.rs` if past 200 lines), `crates/riku-config/src/router.rs` (fix the `load_file` doc at line 39: no supervisor caller), `crates/riku-config/src/container_engine.rs`, `crates/riku-cli/src/setup/init_router.rs:29-68`, `crates/riku-supervisor/src/containers/mod.rs:111-120`, `crates/riku-deploy/src/containers/engine_open.rs:36-40`
- Test: `crates/riku-config/src/settings_tests.rs`, `router_tests.rs`, `container_engine_tests.rs`

**Interfaces:**
- Consumes: existing `settings::load(paths: &RikuPaths) -> Result<ServerSettings>` (`settings.rs:129`).
- Produces:
  - `EngineSettings` gains `program: Option<String>` (TOML `[engine] program = "podman"`). `ServerSettings { engine: EngineSettings, router: Option<String> }` (TOML `[router] name = "caddy"`). `None` means absent. Both lose `Copy`.
  - `Router::load(paths) -> Result<Router>`: `settings.router` when set, else the legacy `ROUTER` file, else nginx. An invalid settings.toml is an error naming the file (never a silent nginx fallback).
  - `container_engine::load(paths) -> Result<Option<String>>`: `settings.engine.program`, else the legacy `CONTAINER_ENGINE` file. The two callers that pair `settings::load` with `container_engine::load` read `settings.engine.program` from the one load.
  - `settings::save_key(paths, section: &str, key: &str, value: Option<&str>) -> Result<()>`: read the file as `toml::Table` (empty when missing), set or remove one key, write it back. Other keys untouched; comments lost.
  - `router::save(paths, &Router)` calls `save_key(paths, "router", "name", ..)`; no longer writes `ROUTER`.
  - `init_router::deploy_router` reads the deploy user's settings.toml through `DeployUser::read_file` before the legacy `ROUTER` file.

- [ ] **Step 1: Failing tests:** `router_from_settings_wins_over_legacy_file`, `legacy_router_file_read_without_settings_key` (Review Focus 1), `invalid_settings_fails_router_load` (error names settings.toml), `engine_program_from_settings`, `unknown_router_key_refused` (names file and key), `save_key_keeps_other_sections`.
- [ ] **Step 2:** `cargo test -p riku-config` fails.
- [ ] **Step 3:** Implement. Unknown sections and keys stay errors.
- [ ] **Step 4:** `cargo check --workspace`; `cargo test -p riku-config -p riku-cli -p riku-deploy -p riku-supervisor` passes.
- [ ] **Step 5:** Commit `Move server settings into settings.toml`.

### Task 4: riku.toml schema and parser

**Files:**
- Create: `crates/riku-config/src/app_settings/mod.rs` (types, `parse`, `load`), `app_settings/keys.rs` (key table), `app_settings/values.rs` (kind and charset checks), `app_settings/processes.rs`
- Modify: `crates/riku-config/Cargo.toml` (add riku-util), `crates/riku-config/src/lib.rs` (`pub mod app_settings;`), `crates/riku-util/src/procfile.rs:31` (make `is_valid_cron_command` `pub`, approved by the owner)
- Test: `crates/riku-config/src/app_settings/app_settings_tests.rs`

**Interfaces:**
- Produces:
  - `pub enum Source { Repo, Server }`
  - `pub struct AppSettings` (private fields)
  - `AppSettings::parse(raw: &str, file: &Path, source: Source) -> Result<AppSettings>`
  - `AppSettings::load(file: &Path, source: Source) -> Result<Option<AppSettings>>` (`None` when the file is missing)
  - `AppSettings::env(&self) -> HashMap<String, String>`: the `RIKU_*` keys this file sets
  - `AppSettings::processes(&self) -> Option<HashMap<String, String>>`: `[processes]` plus `[cron]` in the shape `riku_util::procfile::parse_procfile` returns; `None` when neither section exists
  - `AppSettings::set(&mut self, key: &str, value: &str) -> Result<()>`, `unset(&mut self, key: &str)`, `get(&self, key: &str) -> Option<String>`, `to_toml(&self) -> String`
  - `AppSettings::from_env(env: &HashMap<String, String>) -> (AppSettings, Vec<String>)`: reverse mapping; second value lists the env keys it moved
  - `keys::KEYS: &[Key]`, `Key { section, name, env, kind: Kind, scope: Scope, secret: bool }`, `Kind { Text, Bool, Int, List }`, `Scope { Any, ServerOnly }`, `keys::is_table_env_key(env: &str) -> bool`

- Key table (`section.name`, kind, env key). Every listed key has a Rust or plugin reader today; the implementer greps `crates/`, `plugins/` and `templates/` and drops any without one, noting it:
  - `routing`: `domains` List RIKU_DOMAINS; `https_only` Bool RIKU_HTTPS_ONLY; `document_root` Text RIKU_NGINX_DOCUMENT_ROOT; `static` Bool RIKU_NGINX_STATIC; `static_paths` List RIKU_NGINX_STATIC_PATHS; `allow_git_folders` Bool RIKU_NGINX_ALLOW_GIT_FOLDERS; `bind_address` Text RIKU_BIND_ADDRESS (ServerOnly)
  - `tls`: `acme_email` Text RIKU_ACME_EMAIL; `acme_root_ca` Text RIKU_ACME_ROOT_CA (ServerOnly)
  - `runtime`: `name` Text RIKU_RUNTIME; `version` Text `RIKU_<NAME>_VERSION` (name uppercased, `-` to `_`; refused when `name` is absent); `python_package_manager` Text RIKU_PYTHON_PACKAGE_MANAGER
  - `workers`: `processes` Int RIKU_WORKER_PROCESSES; `timeout` Int RIKU_WORKER_TIMEOUT; `grace_period` Int RIKU_WORKER_GRACE_PERIOD; `max_restarts` Int RIKU_MAX_RESTARTS; `auto_restart` Bool RIKU_AUTO_RESTART
  - `health`: `url` Text RIKU_HEALTH_CHECK_URL; `interval` Int RIKU_HEALTH_CHECK_INTERVAL; `timeout` Int RIKU_HEALTH_CHECK_TIMEOUT; `retries` Int RIKU_HEALTH_CHECK_RETRIES
  - `limits` (all ServerOnly, Int): `max_memory_mb` RIKU_MAX_MEMORY_MB; `max_cpu_seconds` RIKU_MAX_CPU_SECONDS; `max_file_size_mb` RIKU_MAX_FILE_SIZE_MB; `max_open_files` RIKU_MAX_OPEN_FILES; `max_processes` RIKU_MAX_PROCESSES; `isolation_max_memory_mb` RIKU_ISOLATION_MAX_MEMORY_MB; `isolation_max_pids` RIKU_ISOLATION_MAX_PIDS; `isolation_cpu_quota_us` RIKU_ISOLATION_CPU_QUOTA_US; `isolation_cpu_period_us` RIKU_ISOLATION_CPU_PERIOD_US
  - `containers`: `watch_services` List RIKU_WATCH_SERVICES
  - `secrets` (ServerOnly, secret): `ghcr_username` Text RIKU_GHCR_USERNAME; `ghcr_token` Text RIKU_GHCR_TOKEN; `notify_webhook_url` Text RIKU_NOTIFY_WEBHOOK_URL
- Left out: keys riku already refuses (`route_build.rs:33-49` REMOVED_KEYS, and RIKU_NGINX_INCLUDE_FILE at line 45); keys riku writes itself (RIKU_UPSTREAM_PORT, RIKU_NGINX_PORTMAP, RIKU_NGINX_EXTERNAL_PORT, RIKU_NGINX_WSGI, RIKU_NGINX_SOCKET, RIKU_BACKENDS, RIKU_STATIC_ALIASES; see `workers/web_env.rs:80-142`); server and test env (RIKU_ENV, RIKU_ACME_ROOT, RIKU_HEALTH_PORT, RIKU_DASHBOARD_*, RIKU_NS_*, RIKU_ENABLE_CORE_DUMPS, RIKU_ISOLATION_ROOT, RIKU_TEST_DOCKER, RIKU_PLUGIN_TIMEOUT).
- Values: List joins with `,` (`route_build.rs:74` splits on `,` and whitespace). Bool `true` renders `"true"`, `false` omits the key, unless the key's read site parses the value (the implementer checks each Bool key; presence checks like `route_build.rs:103` need the omit rule). Repo Text and List items allow only letters, digits and `. - _ / : @ +`; no whitespace, quotes, `;`, `$`, backtick or control characters. `routing.document_root` from either file is relative and has no `..` component.
- Scope: a ServerOnly key in a `Source::Repo` file is refused, naming the key and saying it belongs in `riku settings set`. `[processes]` and `[cron]` are refused in a `Source::Server` file.
- Processes: `[processes]` keys follow `riku_util::procfile::is_valid_kind` (`procfile.rs:44`); a key starting with `cron` is refused there (use `[cron]`). `[cron] backup = "0 2 * * * ./backup.sh"` becomes kind `cron-backup`, checked with `riku_util::procfile::is_valid_cron_command`.
- `from_env` moves `RIKU_<NAME>_VERSION` only when `RIKU_RUNTIME` names that runtime; otherwise the key stays.

- [ ] **Step 1: Failing tests:** `maps_domains_and_https_to_env` (`domains = ["a.example", "b.example"]` gives `RIKU_DOMAINS = "a.example,b.example"`), `refuses_unknown_key_with_line` (error contains file, `routing.domain`, line; not the value), `refuses_wrong_type`, `refuses_server_only_keys_in_repo` (`secrets.ghcr_token`, `limits.max_memory_mb`) and `allows_server_only_keys_in_server_file` (Review Focus 3), `refuses_escaping_values` (`document_root = "../../etc"`, a domain with `;`, a domain with a newline; Review Focus 4), `runtime_version_uses_runtime_name`, `cron_section_becomes_cron_kind`, `refuses_cron_key_in_processes`, `refuses_processes_in_server_file`, `from_env_round_trips` (table keys only, others untouched).
- [ ] **Step 2:** `cargo test -p riku-config app_settings` fails.
- [ ] **Step 3:** Implement. Parse to `toml::Table`; syntax error lines from `toml::de::Error::span`, schema error lines by locating the key with the spanned API the workspace `toml` 1.x offers.
- [ ] **Step 4:** `cargo check --workspace`; `cargo test -p riku-config -p riku-util` passes.
- [ ] **Step 5:** Commit `Add the riku.toml schema`.

### Task 5: App config loader

**Files:**
- Create: `crates/riku-config/src/app_config.rs`
- Modify: `crates/riku-config/src/lib.rs` (`pub mod app_config;`, `RikuPaths::app_settings_override(app) -> PathBuf` = `env_root/<app>/riku.toml`, `RikuPaths::app_settings_repo(app) -> PathBuf` = `app_root/<app>/riku.toml`)
- Test: `crates/riku-config/src/app_config_tests.rs`

**Interfaces:**
- Consumes: Task 4 `AppSettings::load`, `env`, `processes`, `keys::is_table_env_key`; `riku_util::procfile::parse_procfile`; `riku_util::env::parse_settings`.
- Produces:
  - `pub struct AppEnv { pub env: HashMap<String, String>, pub legacy_keys: Vec<String> }`. `legacy_keys` lists table keys found in ENV (callers that log warn once: run `riku doctor --fix`).
  - `app_config::load_env(paths: &RikuPaths, app: &str) -> Result<AppEnv>`: repo riku.toml env, then ENV table keys, then server override; non table ENV keys copied as today. ENV is parsed into a fresh map; a malformed ENV line is an error naming file and line (fixes the existing partial ENV bug below).
  - `app_config::load_processes(paths: &RikuPaths, app: &str) -> Result<Option<HashMap<String, String>>>`: repo riku.toml `[processes]`/`[cron]` when present, else `parse_procfile` on `app_root/<app>/Procfile`, else `None`. Both present is an error: `both riku.toml [processes] and Procfile define processes for 'myapp'; keep one`. Procfile's "wsgi replaces web" rule (`procfile.rs:113-124`) is not copied: a riku.toml with both `web` and `wsgi` is refused.
  - Env only readers call `load_env` only, so a broken Procfile still fails only the deploy, as today.
- Existing bug fixed here: `riku_util::env::parse_settings` (`env.rs:63-68`) says "ignoring file" on a malformed line but has already inserted the earlier lines into the caller's map, so callers act on a partial ENV. Step 1 starts with the failing test for it.

- [ ] **Step 1: Failing tests:** `malformed_env_line_is_error_not_partial_map`, `repo_settings_reach_env`, `env_file_value_beats_repo_settings` (records `legacy_keys`), `server_override_beats_env_file`, `invalid_repo_settings_is_error`, `riku_toml_processes_replace_procfile`, `both_process_sources_refused`, `refuses_web_and_wsgi_in_riku_toml`, `no_riku_toml_matches_env_and_procfile` (env equals today's ENV parse; processes equal `parse_procfile`).
- [ ] **Step 2:** `cargo test -p riku-config app_config` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p riku-config` passes.
- [ ] **Step 5:** Commit `Add the app config loader`.

### Task 6: Move every reader onto the loader

**Files:**
- Modify, deploy path (load once): `crates/riku-deploy/src/lib.rs:140-160` (`do_deploy` calls `load_env` and `load_processes` once; container branch refuses a riku.toml with `[processes]` or `[cron]`: `riku.toml [processes] is not supported for container apps`), `crates/riku-deploy/src/deploy_env.rs:34-55`, `crates/riku-deploy/src/procfile_deploy.rs:26-39`, `crates/riku-deploy/src/workers.rs:121` (and `create_workers_generic`, re-exported at `lib.rs:57`)
- Modify, env readers (`load_env`): `crates/riku-deploy/src/supervisor_ctl.rs:120` (`read_app_env`, which feeds `router/renew.rs:57` and `workers/web_rollout.rs:53,61`), `crates/riku-deploy/src/teardown.rs:22-27` (destroy must see `runtime.name`), `crates/riku-deploy/src/containers/context.rs`, `crates/riku-supervisor/src/daemon/image_watch.rs:62`, `crates/riku-cli/src/apps/info.rs`, `crates/riku-cli/src/apps/dump_state/builder.rs`
- Modify, process readers: `crates/riku-supervisor/src/daemon/config_watcher.rs:240-246` and `cron_tasks.rs:120`, `crates/riku-cli/src/apps/logs/services.rs:24` (`load_processes`), `crates/riku-cli/src/apps/deploy.rs:69` (a dir with a riku.toml `[processes]` section and no Procfile is accepted)
- Leave as is: ENV writers and editors `apps/config.rs`, `agent/commands/cmd_config.rs`, `workers/web_env.rs`, `env_setup.rs`, `doctor/app_env.rs`, `containers/mock_state.rs`, `riku-plugins/src/addon/service.rs` (writes only), `riku-dashboard/src/appcfg.rs` (the ENV editor; it keeps showing raw ENV); `workers/pinned.rs` (reads only `PORT`); `containers/release.rs:125` (Procfile only); `riku-remote/src/quickstart/` (laptop side scaffold).
- Test: `crates/riku-deploy/src/deploy_env_tests.rs`, `crates/riku-deploy/src/procfile_deploy_tests.rs` (create, add `#[path]` module to `procfile_deploy.rs`), supervisor cron test next to `cron_tasks.rs`

**Interfaces:**
- Consumes: Task 5 `app_config::load_env`, `load_processes`, `AppEnv`.
- Produces:
  - `load_deploy_env(app, paths, app_env: &AppEnv, dlog) -> Result<HashMap<String, String>>`: deploy env built from the loaded env; logs the legacy warning once.
  - `read_workers(app, processes: Option<&HashMap<String, String>>, dlog)` (was `(app, app_path, dlog)`).
  - `create_workers_generic` and `create_workers_with` take `processes: Option<&HashMap<String, String>>` instead of parsing the Procfile at `workers.rs:121`; `release_and_start` passes it through.
  - Supervisor: `load_cron_jobs(&mut self, app: &str, processes: &HashMap<String, String>)`; `load_config_file` builds paths once with `RikuPaths::from_env()` (as `image_watch.rs:22` does) and calls `load_processes`. Cron kinds are iterated sorted so `cron_index` stays stable.
- Error policy: deploy, router renew and web rollout propagate loader errors. `info` and `dump_state` show the error instead of dropping it. `image_watch` logs a warning naming the app and skips it for that tick.

- [ ] **Step 1: Failing tests:** `riku_toml_domains_reach_router_config` (Review Focus 5, through `read_app_env`), `riku_toml_cron_job_is_scheduled` (Review Focus 5, calls `load_cron_jobs` with a map; no process env changed), `riku_toml_worker_gets_worker_config`, `container_app_refuses_riku_toml_processes`, `destroy_uses_riku_toml_runtime`, `broken_procfile_does_not_fail_read_app_env`, `legacy_env_deploys_as_before`.
- [ ] **Step 2:** Run the touched crates' tests; they fail.
- [ ] **Step 3:** Implement. Then `git grep -n '"ENV"\|"Procfile"' crates` and confirm every remaining hit is in the leave as is list or a test.
- [ ] **Step 4:** `cargo check --workspace`; `cargo test -p riku-deploy -p riku-supervisor -p riku-cli -p riku-dashboard -p riku-plugins` passes.
- [ ] **Step 5:** Commit `Read app config through one loader`.

### Task 7: riku settings command, and config set refuses table keys

**Files:**
- Modify: `crates/riku-args/src/cmds.rs:38` (next to `ConfigCmd`), `crates/riku-args/src/cli.rs:104` area (`Settings` variant), `crates/riku-args/src/placement.rs:29` (`Forwarded` arm), `crates/riku-remote/src/routing.rs:12-53` (`get_plugin_command`: `Settings` is not client replaceable), `crates/riku-cli/src/dispatch/mod.rs:109` and `dispatch/app_cmds.rs` (`run_settings`), `crates/riku-cli/src/apps/mod.rs` (declare `settings_cmd`), `crates/riku-cli/src/apps/config.rs:43` (`cmd_config_set` refuses table keys; add a `#[path]` test module), `tests/demo/entrypoint.sh:105` and `crates/riku-deploy/src/deploy_finish.rs:34` (switch `config set RIKU_DOMAINS` to `riku settings set <app> routing.domains=...`)
- Create: `crates/riku-cli/src/apps/settings_cmd.rs`, `settings_cmd_tests.rs`, `crates/riku-cli/src/apps/config_tests.rs`
- Test: `crates/riku-args/src/placement_tests.rs`

**Interfaces:**
- Consumes: Task 4 `AppSettings::{load, parse, set, unset, get, to_toml}`, `keys`; Task 5 `app_settings_override`; `exit_if_invalid(app, &paths.app_root)` as `config.rs:44` uses it.
- Produces: `riku settings show APP`, `riku settings get APP KEY`, `riku settings set APP KEY=VALUE...`, `riku settings unset APP KEY...`. All four act on the server override file only (not the effective merge). `KEY` is `section.name`. List values are comma separated. Arguments split like `cmd_config_set` (`config.rs:51-52`). The result is validated with `Source::Server` before writing, mode 0600, dir 0700. `set` and `unset` redeploy with `do_deploy` (not `update_env_and_redeploy`, which rewrites ENV). `show` prints secret keys as `***`; `get` of a secret prints it. `riku config set myapp RIKU_DOMAINS=x` fails: `RIKU_DOMAINS is a riku setting; use riku settings set myapp routing.domains=x`.

- [ ] **Step 1: Failing tests:** `set_then_get_round_trips`, `set_refuses_unknown_key`, `unset_removes_key`, `show_masks_secrets`, `refuses_invalid_app_name` (`../x`), `settings_is_forwarded_by_rk`, `config_set_refuses_table_key`.
- [ ] **Step 2:** `cargo test -p riku-cli settings_cmd`, `cargo test -p riku-cli config_tests` and `cargo test -p riku-args placement` fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo check --workspace`; both pass.
- [ ] **Step 5:** Commit `Add riku settings for per app overrides`.

### Task 8: doctor migration

**Files:**
- Create: `crates/riku-cli/src/doctor/migrate_settings.rs`, `crates/riku-cli/src/doctor/migrate_settings_tests.rs`
- Modify: `crates/riku-cli/src/doctor/mod.rs:147-152` (run under `fix` as the deploy user, after `redeploy_old_sites` and before `fix_launch_paths`; declare the module), `crates/riku-cli/src/doctor/app_env.rs` (warn on table keys still in ENV), `crates/riku-cli/src/doctor/settings_check.rs` (warn while `ROUTER` or `CONTAINER_ENGINE` exist; fail once when settings.toml is invalid), `crates/riku-deploy/src/containers/mock_state.rs:122-127` if its legacy files break the new tests

**Interfaces:**
- Consumes: Task 3 `settings::load`, `save_key`; Task 4 `AppSettings::{from_env, parse, load, to_toml, env}`; Task 5 `app_settings_override`, `load_env`.
- Produces: `migrate_settings(paths: &RikuPaths) -> Vec<Check>` (`Check` is doctor's ok/warn/fail result):
  - As root: returns `Check::warn("settings migration", "skipped as root: run riku doctor --fix as the deploy user")` and changes nothing, like `redeploy_old_sites` and `fix_launch_paths`.
  - `ROUTER` and `CONTAINER_ENGINE` values move into settings.toml (an existing settings key wins), then the file is removed.
  - Per app: only `RIKU_*` table keys move; legacy renamed keys are not touched (doctor keeps only warning about them, `doctor/app_env.rs:23-26`). A key moves only when its value survives the round trip byte for byte: `AppSettings::parse(to_toml(), Server).env()[k] == env[k]`. Keys that do not (whitespace separated `RIKU_DOMAINS`, `RIKU_NGINX_STATIC=false`, `RIKU_AUTO_RESTART=false`, a non integer Int, a `document_root` with `..`) stay in ENV with a warn `Check` naming the key. Existing override keys win.
  - Order per app: copy `ENV` to `ENV.bak` once, validate the whole new override, write the override, then rewrite ENV without the moved keys. Other ENV lines kept byte for byte, comments included.
  - Writes go to a temp file in the same dir created with the original file's mode (0600 when new), then rename.
  - One `Check` per change; one ok `Check` when nothing changed.

- [ ] **Step 1: Failing tests:** `moves_router_file_into_settings`, `moves_table_keys_out_of_env_keeping_other_lines`, `keeps_lossy_keys_in_env_with_warning` (`RIKU_NGINX_STATIC=false`, `RIKU_DOMAINS="a.example b.example"`), `keeps_env_file_mode`, `skips_as_root` (where root can be mocked the way `fix_launch_paths` tests do), `second_run_changes_nothing` (Review Focus 6), `app_env_unchanged_after_migration` (`load_env` env before equals after).
- [ ] **Step 2:** `cargo test -p riku-cli migrate_settings` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p riku-cli` passes.
- [ ] **Step 5:** Commit `Migrate env settings with riku doctor --fix`.

### Task 9: Docs, map and roadmap

**Files:**
- Modify: `docs/docs/settings.md`, `mkdocs.yml` (nav), `docs/CODEMAP.md`, `docs/superpowers/ROADMAP.md`
- Create: `docs/docs/riku-toml.md`

- [ ] **Step 1:** `riku-toml.md`: what the file is, one full example, the key table by hand from `keys.rs`, server only keys, precedence, Procfile fallback, container app rule, `riku settings`. `settings.md`: `[router] name`, `[engine] program`, comment loss on save, the migration and the downgrade note. Replace instructions that use `riku config set RIKU_*` for table keys.
- [ ] **Step 2:** CODEMAP: `app_settings`, `app_config`, settings changes, `settings_cmd`, `migrate_settings`.
- [ ] **Step 3:** ROADMAP: the two small fixes and Project 1 done with commit hashes.
- [ ] **Step 4:** Commit `Document riku.toml and server settings`.

### Final gate (once, before review)

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`
- [ ] `./tests/deploy-smoke/test-all.sh`
- [ ] Search the branch diff for the confidential names before any push.
