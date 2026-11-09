# Config Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The two small fixes, then riku settings move out of env vars into `settings.toml` (server) and an optional `riku.toml` (per app, repo and server override), with a `riku doctor --fix` migration.

**Architecture:** One key table maps every `riku.toml` key to the `RIKU_*` env key riku reads today. Deploy parses the repo `riku.toml` and the server override, turns them into those env keys and merges them into the deploy env map, so every existing consumer keeps working unchanged. Server values (router, container engine, plugin timeout) become sections of `settings.toml`, with the old files still read until doctor migrates them.

**Tech Stack:** Rust workspace, `toml` (already a dependency of riku-config), `serde` derive where it keeps parsing plain.

**Spec:** `docs/superpowers/specs/2026-11-09-developer-experience-design.md` (step 1 and Project 1 only).

## Global Constraints

- `.claude/CLAUDE.md` rules apply: no `?` (explicit `match`/`return Err`), tests in sibling `_tests.rs` via `#[path]`, files under about 200 lines, doc comments on public items, no emoji, no dashes in prose.
- Native file tools only for reading and editing (Read, Edit, Write; Grep/Glob or shell grep for search). No python, perl or other interpreters.
- Commits: imperative subject under 50 characters, no AI attribution, commit with `git commit -F <file> --no-verify`.
- Confidential: never write the owner's server, organisation or project names. Use `prod`, `edge.example`, `myapp`.
- Per task run only `cargo test -p <touched crate>`. Full suite, clippy and Docker e2e run once before the final review.
- The plugin protocol env (`RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`, `RIKU_HOOK`, `RIKU_PLUGIN_*`, `RIKU_ADDON_*`, `RIKU_DETECTED_RUNTIME`) does not change.
- Breaking changes are allowed, but an upgraded server that has not run `riku doctor --fix` must deploy exactly as before.

## Review Focus

1. A server running Caddy upgrades and has not run doctor: the router must stay Caddy (old `ROUTER` file still honored). Test in Task 3.
2. `riku.toml` with a typo'd key (`domain = ...`) or a wrong type (`https_only = "yes"`): push refused naming file, key and line. Test in Task 4.
3. A secret (`ghcr_token`, `notify_webhook_url`) in the repo `riku.toml`: refused, since the repo file is committed. Allowed in the server override. Test in Task 4.
4. App ENV still holds `RIKU_DOMAINS` and the repo `riku.toml` sets `domains`: the ENV value wins (it is a server side value) and the deploy warns to run doctor. Test in Task 5.
5. `riku doctor --fix` run twice: second run changes nothing and reports ok. Test in Task 7.

---

### Task 1: Detect scaffolded runtime bundles

**Files:**
- Modify: `crates/riku-plugins/src/runtime.rs:62-108` (`discover`)
- Test: `crates/riku-plugins/src/runtime_tests.rs`

**Interfaces:**
- Consumes: `crate::bundles::find_bundles(plugin_root) -> Vec<(PathBuf, PluginManifest)>` (`bundles.rs:12`), `PluginManifest { name, plugin_type, entry, .. }` (`crates/riku-bundle/src/manifest.rs:117`).
- Produces: `discover` also returns `RuntimePlugin { name: manifest.name, path: bundle_dir.join(manifest.entry) }` for every bundle whose `plugin_type` is runtime. Plain file plugins unchanged. Sorted by name as today.

- [ ] **Step 1: Failing test** `discovers_runtime_bundle_from_scaffold`: write `plugins/mynode/riku-plugin.toml` (`name="mynode"`, `type="runtime"`, `api=1`, `entry="bin/mynode"`) and an executable `plugins/mynode/bin/mynode`; assert `discover` returns one plugin named `mynode` whose path ends in `mynode/bin/mynode`. Second test `skips_non_runtime_bundle`: same with `type="addon"`, assert empty.
- [ ] **Step 2:** `cargo test -p riku-plugins runtime` fails on the first test.
- [ ] **Step 3:** In `discover`, after the file scan, append runtime bundles from `find_bundles`. The entry must resolve inside the bundle dir (reuse whatever path check bundles already apply; if none, refuse an `entry` with `..` or an absolute path) and be executable.
- [ ] **Step 4:** `cargo test -p riku-plugins` passes.
- [ ] **Step 5:** Commit `Detect runtime plugins installed as bundles`.

### Task 2: Make plugins.md match the code

**Files:**
- Modify: `docs/docs/plugins.md`, `PLUGIN_PROTOCOL.md:274-279`, `docs/docs/plugin-bundles.md:86` if Task 1 changes what it says

- [ ] **Step 1:** Fix each claim, checking the code line first:
  - `plugins.md:128`: post build failure aborts the deploy (`crates/riku-plugins/src/manager.rs:145`).
  - `plugins.md:30-31`: no `container` plugin; java and clojure are opt in via `--plugins` (`crates/riku-cli/src/apps/install_plugins.rs:16-22`).
  - `plugins.md:145-197`: remove "server side utility plugins" (`riku hello`); the CLI does not accept unknown subcommands.
  - `plugins.md:273-277`, `366-399`: remove `riku open`, `riku tunnel`, and `riku backup` as client plugins; list the ten replaceable commands from `crates/riku-remote/src/routing.rs:12-24`.
  - `plugins.md:327`: `riku plugin list` lists client plugins.
  - `PLUGIN_PROTOCOL.md:274-279`: gate mode currently runs as observe and cannot veto (`crates/riku-plugins/src/events/bus.rs:71-77`).
- [ ] **Step 2:** `mkdocs build --strict` (from `docs/`) passes if mkdocs is installed; if not, say so in the report.
- [ ] **Step 3:** Commit `Make the plugin docs match the code`.

### Task 3: Server settings in settings.toml

**Files:**
- Modify: `crates/riku-config/src/settings.rs` (split into `settings/mod.rs`, `settings/engine.rs`, `settings/server.rs` if it passes 200 lines), `crates/riku-config/src/router.rs`, `crates/riku-config/src/container_engine.rs`, `crates/riku-plugins/src/executor.rs:14-27`, `crates/riku-cli/src/setup/init_router.rs:29-47`
- Test: `crates/riku-config/src/settings_tests.rs`, `router_tests.rs`, `container_engine_tests.rs`

**Interfaces:**
- Produces:
  - `ServerSettings { engine: EngineSettings, router: Option<String>, container_engine: Option<String>, plugin_timeout: Option<Duration> }`. TOML: `[router] name = "caddy"`, `[engine] program = "podman"` (next to the existing timeouts), `[plugins] timeout = "300s"`. `None` means the key is absent.
  - `Router::load(paths)`: `settings.router` when set, else the legacy `ROUTER` file, else nginx. Same for `container_engine::load(paths)` with the legacy `CONTAINER_ENGINE` file.
  - `router::save(paths, &Router)` writes `[router] name` into settings.toml (creating or updating it, other sections kept), no longer the `ROUTER` file.
  - `settings::save(paths, &ServerSettings) -> Result<()>` used by `router::save` and Task 7.
  - `executor::plugin_timeout(paths: &RikuPaths) -> Duration`: settings value, else `RIKU_PLUGIN_TIMEOUT`, else 300s. Update its 10 call sites.

- [ ] **Step 1: Failing tests:** `router_from_settings_wins_over_legacy_file`, `legacy_router_file_still_read_without_settings_key` (Review Focus 1), `engine_program_from_settings`, `unknown_router_key_refused` (error names file and key), `save_router_keeps_engine_section`.
- [ ] **Step 2:** `cargo test -p riku-config` fails.
- [ ] **Step 3:** Implement. Keep the strict parsing: unknown section or key is an error naming the file.
- [ ] **Step 4:** `cargo test -p riku-config -p riku-plugins -p riku-cli` passes.
- [ ] **Step 5:** Commit `Move server settings into settings.toml`.

### Task 4: riku.toml schema and parser

**Files:**
- Create: `crates/riku-config/src/app_settings/mod.rs` (types, `parse`, `env`), `app_settings/keys.rs` (the key table), `app_settings/processes.rs`
- Test: `crates/riku-config/src/app_settings/app_settings_tests.rs`

**Interfaces:**
- Produces:
  - `pub enum Source { Repo, Server }`
  - `pub struct AppSettings` (private fields)
  - `AppSettings::parse(raw: &str, file: &Path, source: Source) -> Result<AppSettings>`
  - `AppSettings::load(file: &Path, source: Source) -> Result<Option<AppSettings>>` (`None` when missing)
  - `AppSettings::env(&self) -> HashMap<String, String>`: the `RIKU_*` keys this file sets
  - `AppSettings::processes(&self) -> Option<HashMap<String, String>>`: `[processes]` plus `[cron]`, in the same shape `parse_procfile` returns; `None` when neither section is present
  - `AppSettings::from_env(env: &HashMap<String, String>) -> (AppSettings, Vec<String>)`: reverse mapping for Task 7; second value is the keys it moved
  - `AppSettings::to_toml(&self) -> String`
  - `keys::KEYS: &[Key]`, `Key { section, name, env, kind: Kind, scope: Scope }`, `Kind { Text, Bool, Int, List }`, `Scope { Any, ServerOnly }`

- Key table. Each row is `section.name` to env key. The implementer confirms each env key is read per app today (grep its read site) and drops any that is not, noting it in the report:
  - `routing`: `domains` (List) RIKU_DOMAINS, `https_only` (Bool) RIKU_HTTPS_ONLY, `bind_address` RIKU_BIND_ADDRESS, `disable_ipv6` (Bool) RIKU_DISABLE_IPV6, `upstream_port` (Int) RIKU_UPSTREAM_PORT, `ipv4_address` RIKU_NGINX_IPV4_ADDRESS, `ipv6_address` RIKU_NGINX_IPV6_ADDRESS, `external_port` (Int) RIKU_NGINX_EXTERNAL_PORT, `portmap` RIKU_NGINX_PORTMAP, `cloudflare_acl` (Bool) RIKU_NGINX_CLOUDFLARE_ACL, `csp` RIKU_NGINX_CSP, `catch_all` RIKU_NGINX_CATCH_ALL, `document_root` RIKU_NGINX_DOCUMENT_ROOT, `static` (Bool) RIKU_NGINX_STATIC, `static_paths` (List) RIKU_NGINX_STATIC_PATHS, `allow_git_folders` (Bool) RIKU_NGINX_ALLOW_GIT_FOLDERS
  - `cache`: `any`, `control`, `expiry`, `path`, `prefixes` (List), `redirects`, `size`, `time` to RIKU_NGINX_CACHE_*
  - `tls`: `acme_email` RIKU_ACME_EMAIL, `acme_root_ca` RIKU_ACME_ROOT_CA
  - `runtime`: `name` RIKU_RUNTIME, `version` to `RIKU_<NAME>_VERSION` (name uppercased, `-` to `_`; refused when `name` is absent), `python_package_manager` RIKU_PYTHON_PACKAGE_MANAGER
  - `workers`: `processes` (Int) RIKU_WORKER_PROCESSES, `timeout` (Int) RIKU_WORKER_TIMEOUT, `grace_period` (Int) RIKU_WORKER_GRACE_PERIOD, `max_restarts` (Int) RIKU_MAX_RESTARTS, `auto_restart` (Bool) RIKU_AUTO_RESTART
  - `health`: `url` RIKU_HEALTH_CHECK_URL, `interval`, `timeout`, `retries` (Int) RIKU_HEALTH_CHECK_*
  - `limits`: `max_memory_mb`, `max_cpu_seconds`, `max_file_size_mb`, `max_open_files`, `max_processes` (Int) RIKU_MAX_*; `isolation_max_memory_mb`, `isolation_max_pids`, `isolation_cpu_quota_us`, `isolation_cpu_period_us` (Int) RIKU_ISOLATION_*
  - `containers`: `watch_services` (List) RIKU_WATCH_SERVICES
  - `secrets` (ServerOnly): `ghcr_username` RIKU_GHCR_USERNAME, `ghcr_token` RIKU_GHCR_TOKEN, `notify_webhook_url` RIKU_NOTIFY_WEBHOOK_URL
- Paths (spec: the repo file never sets a path outside the app): `cache.path` is ServerOnly. `routing.document_root` from the repo file must be relative and stay inside the app dir; check what the nginx consumer already enforces and refuse at parse time either way.
- Left out on purpose: keys riku sets itself (RIKU_NGINX_SOCKET, RIKU_NGINX_WSGI, RIKU_BACKENDS, RIKU_STATIC_ALIASES), RIKU_NGINX_INCLUDE_FILE (refused today, `route_build.rs:45`), RIKU_ACME_ROOT (server path), and server or test env (RIKU_ENV, RIKU_HEALTH_PORT, RIKU_DASHBOARD_*, RIKU_NS_*, RIKU_ENABLE_CORE_DUMPS, RIKU_ISOLATION_ROOT, RIKU_TEST_DOCKER).
- Value rendering: List joins with `,` (`route_build.rs:74` splits on `,` and whitespace). Bool renders as the literal its consumer accepts; check each Bool key's read site and record the literal in the table.
- Processes: `[processes]` keys follow `is_valid_kind` (`crates/riku-util/src/procfile.rs:44`); a key starting with `cron` is refused there (use `[cron]`). `[cron] backup = "0 2 * * * ./backup.sh"` becomes kind `cron-backup`, validated with the same cron check Procfile uses (reuse it, do not copy it).

- [ ] **Step 1: Failing tests:** `maps_domains_and_https_to_env` (`domains = ["a.example", "b.example"]` gives `RIKU_DOMAINS = "a.example,b.example"`), `refuses_unknown_key_with_line` (Review Focus 2, error contains file, `routing.domain` and line), `refuses_wrong_type` (`https_only = "yes"`), `refuses_secret_in_repo_file` and `allows_secret_in_server_file` (Review Focus 3), `runtime_version_uses_runtime_name` (`name = "node"`, `version = "22"` gives `RIKU_NODE_VERSION = "22"`), `cron_section_becomes_cron_kind`, `refuses_cron_key_in_processes`, `from_env_round_trips` (env to AppSettings to env is identity for table keys, other keys untouched).
- [ ] **Step 2:** `cargo test -p riku-config app_settings` fails.
- [ ] **Step 3:** Implement. Parse to `toml::Table` (as `settings.rs` does) so errors can name the key; line numbers come from `toml::de::Error::span` for syntax errors and from locating the key for schema errors (`toml::Spanned` or a `DeTable` with spans, whichever the workspace `toml` version offers).
- [ ] **Step 4:** `cargo test -p riku-config` passes.
- [ ] **Step 5:** Commit `Add the riku.toml schema`.

### Task 5: Deploy reads riku.toml

**Files:**
- Modify: `crates/riku-deploy/src/deploy_env.rs:34-55`, `crates/riku-deploy/src/procfile_deploy.rs:26-39`, `crates/riku-config/src/lib.rs` (add `RikuPaths::app_settings_override(app) -> PathBuf` = `env_root/<app>/riku.toml`)
- Test: `crates/riku-deploy/src/deploy_env_tests.rs`, new `crates/riku-deploy/src/procfile_deploy_tests.rs`

**Interfaces:**
- Consumes: Task 4 `AppSettings::load`, `env`, `processes`.
- Produces:
  - `load_deploy_env(app, paths, dlog)` result: built in defaults, then repo `riku.toml` env, then legacy `RIKU_*` table keys found in `ENV` (warn once: run `riku doctor --fix`), then the server override env. Later wins. App keys from ENV that are not table keys are included as today.
  - `read_workers(app, app_path, settings: Option<&AppSettings>, dlog)`: `riku.toml` processes when present; Procfile otherwise; both present (riku.toml processes and a Procfile with at least one process) is an error: `both riku.toml [processes] and Procfile define processes for 'myapp'; keep one`.
- A riku.toml parse error aborts the deploy with the parser's message, logged like the invalid Procfile case.

- [ ] **Step 1: Failing tests:** `repo_settings_reach_deploy_env`, `env_file_value_beats_repo_settings_and_warns` (Review Focus 4), `server_override_beats_env_file`, `invalid_repo_settings_abort_deploy`, `riku_toml_processes_replace_procfile`, `both_process_sources_refused`, `no_settings_uses_procfile_as_today`.
- [ ] **Step 2:** `cargo test -p riku-deploy deploy_env procfile_deploy` fails.
- [ ] **Step 3:** Implement. Confirm with grep that routing, workers and the supervisor take these keys from the merged deploy env or the `LIVE_ENV` written from it (`env_setup::write_live_env`, `procfile_deploy.rs:110`) and never re-read `ENV` from disk; list any that do and fix them to use the merged map.
- [ ] **Step 4:** `cargo test -p riku-deploy` passes.
- [ ] **Step 5:** Commit `Read riku.toml at deploy`.

### Task 6: riku settings command

**Files:**
- Modify: `crates/riku-args/src/cmds.rs` (next to `ConfigCmd`, line 58), `crates/riku-args/src/cli.rs:102` area, `crates/riku-args/src/placement.rs` (server command, forwarded by rk like `config`), `crates/riku-cli/src/dispatch/app_cmds.rs:27` area
- Create: `crates/riku-cli/src/apps/settings_cmd.rs`
- Test: `crates/riku-cli/src/apps/settings_cmd_tests.rs`, `crates/riku-args/src/placement_tests.rs`

**Interfaces:**
- Produces: `riku settings show APP`, `riku settings get APP KEY`, `riku settings set APP KEY=VALUE...`, `riku settings unset APP KEY...`. `KEY` is `section.name` (`routing.domains`). Values are parsed by the key's `Kind` (List: comma separated). Writes `paths.app_settings_override(app)` with mode 0600, validated by `AppSettings::parse(.., Source::Server)` before writing. Does not redeploy; prints `run: riku restart myapp` (check `cmd_config_set` in `crates/riku-cli/src/apps/config.rs:43` and match whatever it does about redeploy instead, if it redeploys).

- [ ] **Step 1: Failing tests:** `set_then_get_round_trips`, `set_refuses_unknown_key`, `unset_removes_key`, `settings_is_forwarded_by_rk` (placement).
- [ ] **Step 2:** `cargo test -p riku-cli settings_cmd` and `cargo test -p riku-args placement` fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Both pass.
- [ ] **Step 5:** Commit `Add riku settings for per app overrides`.

### Task 7: doctor migration

**Files:**
- Create: `crates/riku-cli/src/doctor/migrate_settings.rs`
- Modify: `crates/riku-cli/src/doctor/mod.rs:147-152` (run under `fix`, as the deploy user like `fix_launch_paths`), `crates/riku-cli/src/doctor/app_env.rs` (warn on table keys still in ENV), `crates/riku-cli/src/doctor/settings_check.rs` (warn while `ROUTER` or `CONTAINER_ENGINE` files exist)
- Test: `crates/riku-cli/src/doctor/migrate_settings_tests.rs`

**Interfaces:**
- Consumes: Task 3 `settings::load/save`, Task 4 `AppSettings::from_env`, `to_toml`, Task 5 `app_settings_override`.
- Produces: `migrate_settings(paths: &RikuPaths) -> Vec<Check>`:
  - `ROUTER` and `CONTAINER_ENGINE` file values move into settings.toml (an existing settings key wins; the file is then removed).
  - For each app, table keys in `ENV` move into the server override (existing override keys win), and are removed from `ENV`. Other ENV lines are kept byte for byte, comments included.
  - One `Check` per change; one ok `Check` when nothing changed.

- [ ] **Step 1: Failing tests:** `moves_router_file_into_settings`, `moves_table_keys_out_of_env_keeping_other_lines`, `second_run_changes_nothing` (Review Focus 5), `deploy_env_unchanged_after_migration` (load_deploy_env before and after give the same map).
- [ ] **Step 2:** `cargo test -p riku-cli migrate_settings` fails.
- [ ] **Step 3:** Implement. Write each file to a temp file in the same dir, then rename.
- [ ] **Step 4:** `cargo test -p riku-cli` passes.
- [ ] **Step 5:** Commit `Migrate env settings with riku doctor --fix`.

### Task 8: Docs and roadmap

**Files:**
- Modify: `docs/docs/settings.md`, `docs/mkdocs.yml` nav (find the real path), `docs/superpowers/ROADMAP.md`
- Create: `docs/docs/riku-toml.md`

- [ ] **Step 1:** `riku-toml.md`: what the file is, one full example, the key table (generated by hand from `keys.rs`), precedence, Procfile fallback rule, `riku settings`. `settings.md`: the new `[router]`, `[engine] program`, `[plugins]` keys and the migration. Remove env var instructions that now point at `riku config set RIKU_*`.
- [ ] **Step 2:** ROADMAP: step 1 and Project 1 done with commit hashes.
- [ ] **Step 3:** Commit `Document riku.toml and server settings`.

### Final gate (once, before review)

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`
- [ ] Docker e2e as the repo runs it (`tests/deploy-smoke/test-all.sh` or the e2e target the README names)
- [ ] Search the branch diff for the confidential names before any push.

## Not in this plan

- Stripping riku keys from the app process env. Today they already reach the process through `LIVE_ENV`; this plan keeps that behavior, so nothing regresses, but the leak the spec names is not closed here.
- `[routing] mode`, `port` and the fallback domain keys: Project 2, since they need router changes.
