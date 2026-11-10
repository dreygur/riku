# Code map

Where each job lives, so you can open the right file without searching. One section per crate: its job, riku crate dependencies, entry points, then one line per module or directory, then gotchas (things that surprise someone changing the code).

Keep it true. Any commit that adds, moves, renames or changes the contract of a module updates this file in the same commit. A line here is a pointer, not a spec: read the code before relying on a contract.

Crate layers, lowest first:

- Leaf: riku-util, riku-config, riku-error, riku-http, riku-bundle, riku-args
- Drivers and renderers: riku-container, riku-nginx, riku-caddy, riku-route, riku-plugins
- Services: riku-supervisor (must not call deploy or cli), riku-deploy
- Entry points: riku-cli, riku-dashboard, riku-remote, riku (server binary), riku-client (`rk`, laptop only)


## riku
Job: the `riku` binary. Parses argv, picks local vs forwarded target, runs local commands. Depends on: riku-config, riku-util, riku-error, riku-nginx, riku-plugins, riku-supervisor, riku-deploy, riku-cli, riku-dashboard. Used by: tests (`riku-route` is a dev-dependency only).
Entry: `fn main` in `src/main.rs`.

- `src/main.rs`: init tracing (`RUST_LOG`, default info), `cli::ssh_command::process_argv`, `parse` (help heading names the implied server), `route::target_for`, `run_or_forward`, then `RikuPaths::from_env` and `cli::dispatch::run_local` (or `dashboard::run`). Contract: `SSH_ORIGINAL_COMMAND` set means "running as the ssh forced command", which never forwards again.
- `src/lib.rs`: facade of `pub use` aliases (cli, config, dashboard, deploy, error, nginx, plugins, supervisor, util) so integration tests reach every crate as `riku::x`. No logic.
- `Cargo.toml`: wires the `[[test]]` targets `integration_tests` and `stress_tests` to `../../tests/` and the bench `core_benchmarks` to `../../benches/`.

Gotchas:
- `Commands::RouterHelper` is handled before `RikuPaths::from_env`: the helper runs as a systemd system service with no `HOME`.
- `riku::cli` re-exports `riku_args` and `riku_remote` modules (see riku-cli lib.rs), so `cli::remote::flow` in main.rs is riku-remote code.
- Unit tests of this crate: none in `src/`; tests live in `tests/` (see tests section).

## riku-client
Job: builds the `rk` laptop-only binary (package riku-client, bin `rk`). Depends on: riku-args, riku-remote. Used by: nobody (leaf binary).
Entry: `fn main` in `src/main.rs`.

- `src/main.rs`: `run`: parse, reject `--local`, `check_supported`, `target_for`, `run_or_forward`. Contract: never runs server code, never the ssh forced command (`via_ssh: false`, `local: false`).
- `src/parse.rs`: `parse`. Refusal of a server-only command is checked before clap parses the command's own args, so `rk init --force` is refused, not "unknown flag". Handles `rk help <cmd>`.
- `src/local.rs`: `check_supported` (uses `riku_args::placement::client_rule`), `run_without_server`: runs `server`, `plugin`, `quickstart` and `plugins scaffold|keygen|sign` locally; any other command with no target is the "no server selected" error listing registered servers.
- `tests/deps.rs`: runs `cargo tree --offline` and fails if riku-client or riku-args link a server crate or `reqwest` (riku-args also may not link riku-config).

Gotchas:
- `rk` must stay small: never add a dependency on riku-deploy, riku-supervisor, riku-nginx, riku-plugins, riku-dashboard, riku-cli, riku-http or reqwest (`tests/deps.rs` enforces it). That is why riku-http is a separate crate.
- `rk` dashboard works only in its remote form (`run_remote` in riku-remote), local `dashboard` falls to the no-server error.

## riku-remote
Job: laptop-side behaviour shared by `riku` and `rk`: named server list, ssh forwarding, remote dashboard client, client plugins, quickstart, plugin authoring. Depends on: riku-args, riku-bundle, riku-util. Used by: riku-cli (re-exports as `cli::remote`, `cli::client_plugins`, `cli::quickstart`, `cli::routing`), riku-client.
Entry: `remote::flow::run_or_forward` (`src/remote/flow.rs`), `remote::route::target_for`, `remote::route::implied_server`, `remote::server_cmd::run`, `plugin_author::cmd_plugins_*`, `quickstart::cmd_quickstart`, `plugin_cmd::run_plugin`.

- `src/remote/flow.rs`: `run_or_forward`, `Outcome`. Order: apply app hint, client plugin, then (`Target::Remote`) remote dashboard or `forward`, else `Outcome::Local`. A forwarded command calls `process::exit` with the remote exit code.
- `src/remote/route.rs`: `target_for`, `implied_server`, `forward`, `is_plugin_authoring`. Contract: server list and `git remote -v` are read only for forwarded commands and `dashboard`, never under `SSH_ORIGINAL_COMMAND`.
- `src/remote/target.rs`: `Target`, `Selection`, `Source`, `resolve`, `registered_names`. Picks server from `-s`, git remote, then default. `--local` and server-side invocation always mean local.
- `src/remote/forward.rs`: `ssh_args`, `run`, `server_arg_positions`, `strip_target_args`, `needs_tty`. Contract: destination is a validated `SshAddress` after `--`; remote command is one `shlex::try_join` argument; no local shell.
- `src/remote/git_remote.rs`: `read_remotes`, `read_remotes_in`, `parse_remotes`, `RemoteRef`. Only scp-style remotes. Reads the repo's git config, so it is untrusted: only matched against registered servers, never adds an ssh target.
- `src/remote/store.rs`: `servers_path`, `load`, `save`. Reads/writes `${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml` (not `dirs::config_dir`), dir 700, file 600, atomic rename, entries revalidated on load.
- `src/remote/types.rs`: `Server`, `ServerList`; re-exports `ServerName`, `SshAddress` from riku-args (validation lives there).
- `src/remote/server_cmd.rs`: `riku server add|list|remove|default`; `run_at(path, repo_dir, cmd)` is the testable form. `add` then runs `server_add.rs`: `key_install.rs` sends this machine's public key (type and data only, one quoted argument) as `ssh -t <admin> sudo riku deploy-key add --user <user from the address>`, or prints that command without an admin; `remote_add.rs` adds the repo's `riku` remote unless one exists.
- `src/remote/dashboard/`: remote `riku dashboard`. One ssh ControlMaster to the admin login, sudo to the deploy user, token sent on stdin only after `READY_MARKER`, local port forward to 127.0.0.1, browser opened. Key: `run_remote` (mod.rs), `check_remote_flags` (flags.rs), `Handshake` state machine (handshake.rs), `remote_script.rs` (script text, markers), `ssh_master.rs` (arg builders, control socket in private temp dir), `sudo.rs` (password via /dev/tty, zeroed), `stop.rs` (SIGTERM only), `interrupt.rs`, `session.rs`, `events.rs`, `browser.rs`, `ended.rs`.
- `src/client_plugins/`: executables in `$HOME/.riku/client-plugins/<command>`. `try_execute_client_plugin`, `cmd_plugin_list`, `cmd_plugin_exists`. Plugin argv: `$1` server address (empty if local), `$2` app, `$3` full command, `$4+` extras.
- `src/routing.rs`: `get_plugin_command` (which `Commands` a client plugin may override; `Rollback`, `Init`, `Backup`, `Doctor`, plugin/hook/agent commands cannot), `build_plugin_args`.
- `src/plugin_cmd.rs`: dispatch for `riku plugin list|exists`.
- `src/plugin_author/`: `plugins scaffold|keygen|sign` on the author's machine, `Program` (Riku or Rk) sets hint wording. Secret key created 0600 up front and never printed; signing covers the entry file only (see riku-bundle); `skeleton.rs` holds the manifest and entry text per `SeamType`.
- `src/quickstart/`: `cmd_quickstart` writes a sample app (`Runtime` in `scaffold.rs`) into the current directory, `git init`s, prints the remote step (`--remote`, else the default registered server, else `rk server add`) and `git push`. `scaffold_into(base, ...)` takes the directory so tests do not change cwd.
- `src/lib.rs`: `pub(crate)` aliases `cli`, `cmds`, `help`, `placement` (riku-args) and `util` (riku-util).

Gotchas:
- Two homes: server list under XDG/`~/.config/riku`, client plugins under `~/.riku/client-plugins` (note `HOME`, not `RIKU_ROOT`).
- Server-side `riku` also links this crate (through riku-cli), so a change here changes the forced-command path; keep `via_ssh` handling intact.
- Name validation for servers is in riku-args (`address.rs`), not here; `store::load` reuses it via `TryFrom`.
- Dashboard URL (token included) is visible in the local `open`/`xdg-open` argv for an instant; documented and accepted in `browser.rs`.

## riku-util
Job: shared leaf helpers: validation, fs, env/Procfile parsing, cron, display, keys, naming. Depends on: none. Used by: riku-caddy, riku-cli, riku-container, riku-dashboard, riku-deploy, riku-nginx, riku-plugins, riku-remote, riku-route, riku-supervisor, riku. Also linked by `rk`, so no heavy deps.
Entry: re-exports in `src/lib.rs`: `validate_app_name`, `ensure_path_within`, `parse_procfile`, `parse_settings`, `write_config`, `write_atomic`, `write_private`, `create_private_dir`, `get_free_port`, `echo`, `print_table`, `setup_authorized_keys`.

- `src/validation.rs`: `sanitize_app_name`, `validate_app_name`, `existing_app` (error, no output), `exit_if_invalid`, `ensure_path_within`, `REPO_SUFFIX`. Contract: app name keeps only alnum `.` `_` `-`, rejects `..`, dot-only, trailing `.git`; `ensure_path_within` canonicalizes both paths and errors if either does not exist.
- `src/file_names/`: names derived from an app: `worker.rs` (`WorkerName` `<app>@<kind>.<n>`, config file `<id>.toml`), `copy_socket.rs` (`<app>@<n>.sock`). `@` is the separator because neither app names nor kinds may contain it; do not build these names by hand.
- `src/fs.rs`: `write_atomic` (tmp + fsync + rename, mode 0666 less umask), `write_private` (0600 from creation), `create_private_dir`, `restrict_to_owner`, `replace_symlink`, `copy_dir_recursive`, `count_files`.
- `src/write_dir.rs`: `check_write_dir`: root writes only into a real dir owned by `owner` with no group/other write.
- `src/socket_dir.rs`: uWSGI socket dir, mode 2750 with shared group; `prepare`, `ensure_exists`, `group_id`. Refuses a non-directory (symlink) before chown/chmod.
- `src/env.rs`: `parse_settings` (KEY=VALUE with `$VAR` expansion, strips control chars; a line without `=` is an error naming file and line, and the caller's map is left untouched), `write_config` (atomic, 0600), `expandvars`.
- `src/runtime_markers.rs`: `BUNDLED_RUNTIMES`, the runtime plugins riku can install by name, and `needed_runtime`, the shell runtime an app's marker files call for (shared by server and laptop code).
- `src/legacy_env.rs`: `RENAMED_KEYS` (old name to `Replacement::Key(new)` or `Replacement::Removed` when the new name is refused or unread), `legacy_keys`, `Replacement::hint`: deploy and doctor report each old key with the same message.
- `src/flag.rs`: `is_on`, `https_only`, `https_only_warning`: only explicit yes values are on, typos are off.
- `src/nginx_validation.rs`: `validate_env_vars`, `print_env_warnings`: warns only, the route gate decides what reaches config.
- `src/procfile.rs`: `parse_procfile` (None if file missing; any bad line fails the whole parse), `is_valid_kind`. Cron kinds start with `cron` and are checked with `cron::validate_cron_expression`; wsgi supersedes web.
- `src/cron/`: `CronScheduler`, `CronJob`, `validate_cron_expression`. 5-field cron, interpreted in UTC.
- `src/domain.rs`: `Domain` newtype, `parse_domains`: only lowercase alnum, `-`, `.`, optional leading `*` label, so no config syntax gets into router files.
- `src/backend.rs`, `src/route_plan.rs`: `Backend` (Tcp or Unix, `Display` is the router form), `RoutePlan`, `HttpsPolicy` (`only`/`also_plain_http`, serde, sent to the router helper).
- `src/ssh_keys.rs`, `src/authorized_keys.rs`, `src/public_key.rs`: deploy-key management. `setup_authorized_keys(_in)` appends with forced `command=` and dirs 700/files 600; `RIKU_KEY_MARKER` (`FINGERPRINT=`) marks riku's own lines; `parse_public_keys` keeps only a known key type plus the base64 blob, dropping options and comments.
- `src/resource_limits/`: `ResourceLimits::from_env` / `apply` (setrlimit; `RLIMIT_NPROC` only on Linux) / `summary`. Reads `RIKU_MAX_*` env vars.
- `src/spawn.rs`: `spawn_retrying_etxtbsy`, `output_retrying_etxtbsy`: retry a just-written script's exec.
- `src/process_util.rs`: `get_free_port`, `validate_node_version`.
- `src/deploy_logger.rs`: `DeployLogger` timestamped log plus stdout; path comes from `RikuPaths::deploy_log_file`.
- `src/display.rs`: `info`, `success`, `warn`, `error`, `echo`, `format_table`, `print_table`; output styles table is in the module doc.
- `src/listening.rs`: `listening_ports` (TCP LISTEN ports from `/proc/net/tcp{,6}`, empty off Linux), used by port mode in riku-deploy and the router helper's bind check.
- `src/token.rs`, `src/secure.rs`: `generate_token`; `constant_time_eq`.
- `src/root_binary.rs`: `ROOT_BINARY` = `/usr/local/bin/riku`, the one server install path.

Gotchas:
- `echo` only prints (red goes to stderr). `exit_if_invalid` calls `process::exit(1)` on a bad or unknown app name; fine in CLI. Services and JSON interfaces (the agent commands) use `existing_app`.
- `lib.rs` module list in the doc comment is incomplete; the `pub mod` lines are authoritative.
- Doc comments in `procfile.rs` still name `crate::supervisor::cron`; the cron engine now lives in `riku-util/src/cron/`.

## riku-config
Job: repository layer: resolves every riku path, reads host settings files and per app `riku.toml`. Depends on: riku-util. Used by: riku-caddy, riku-cli, riku-container, riku-dashboard, riku-deploy, riku-nginx, riku-plugins, riku-route, riku-supervisor, riku. Not linked by `rk` or riku-args.
Entry: `RikuPaths::from_env`, `RikuPaths::from_dirs`, `Router::load` (`src/lib.rs`, `src/router.rs`).

- `src/lib.rs`: `RikuPaths` (all dirs under `$RIKU_ROOT`, default `$HOME/.riku`), host path constants (`SOCKET_ROOT`, `ROUTER_SOCKET`, `CADDY_SITE_ROOT`, `NGINX_SITE_ROOT`, `CERT_ROOT`, `ACME_WEBROOT`, `STATIC_ROOT`), defaults (`RIKU_WORKER_TIMEOUT`, `RIKU_WORKER_GRACE_PERIOD`, `RIKU_MAX_RESTARTS`). `RikuPaths::for_tests(root)` is public (not cfg(test)) so other crates' tests can use it; it keeps nginx, socket, caddy and acme paths inside `root`.
- `src/root_dirs.rs`: validates `HOME` and `RIKU_ROOT`: absolute, no `..`. Only path inputs taken from env.
- `src/router.rs`: `Router` (Nginx default, Caddy, Plugin(name)), `stored`, `load`, `save`. `stored`: settings.toml `[router] name`, else legacy `<root>/ROUTER`, else `None`; `load` is `stored` or nginx; an invalid settings.toml is an error. `save` writes settings.toml only.
- `src/container_engine.rs`: `load`, `from_settings` (for callers that already loaded settings): `[engine] program`, else legacy `<root>/CONTAINER_ENGINE` (name or absolute path); validated where used, not here.
- `src/app_config.rs`: the one app config loader. `load_env` merges repo `riku.toml`, ENV, server override (later wins) into `AppEnv { env, legacy_keys }`; `load_processes` gives repo `[processes]`/`[cron]` or the Procfile, never both; `load_cron_entries` gives the cron jobs as an ordered list (Procfile `cron:` lines may repeat, so it scans lines). A server override `false` flag removes the key (`AppSettings::cleared_env`). `RikuPaths::app_settings_repo`/`app_settings_override` (lib.rs) name the two files.
- `src/app_settings/`: `riku.toml` (repo file or server override). `mod.rs` `AppSettings::{parse, load, env, processes}`, `Source` (Repo, Server); `keys.rs` `KEYS` table `section.name` to `RIKU_*` env key with `Kind` and `Scope` (only keys something reads; user docs in docs/docs/riku-toml.md); `values.rs` type checks, repo charset, rendering (false `Flag` omitted, `Bool` renders "false"); `processes.rs` `[processes]`/`[cron]` (cron kinds `cron-<name>`); `parse.rs` section walk; `location.rs` file:line errors that never echo a value; `edit.rs` `set`, `unset`, `get`, `entries`, `to_toml`, `from_env`.
- `src/settings/`: `<root>/settings.toml`. `mod.rs` `ServerSettings { router, engine, routing }`, `parse`, `load`, strict: unknown key/section, wrong type, bad duration (`30s|m|h`) fail with key and file in the message; `engine.rs` `EngineSettings` (program plus time limits); `routing.rs` `RoutingSettings { fallback_domain, server_ip (IPv4 only), shared_port, port_range, present }` (`present` false keeps a server without `[routing]` routing as before), `FallbackPattern::expand` (result checked as a `Domain`), `PortRange`, `routing_section` (text from `templates/settings_routing.toml.txt`, so no naming service is in the code); `save.rs` `save_key` patches one key, re-parses before replacing the file (comments are lost), `add_routing_section` appends `[routing]` with comments when missing.
- `src/nginx_host.rs`: `NginxHostPaths` (`from_env` honors `RIKU_NGINX_SITES_ENABLED`, `RIKU_NGINX_HOST_INCLUDE`; `system()` ignores them).
- `src/program.rs`: `installed_program` (path or `None`) and `find_program` over `$PATH` plus fixed system dirs (including `/snap/bin`). `src/reload_programs.rs`: `ReloadPrograms` (systemctl, caddy, nginx), replaced by mock scripts in tests.

Gotchas:
- Root init must use `NginxHostPaths::system`, never `from_env`, because `sudo -E` keeps the caller's env.
- `socket_root`, `caddy_root`, `acme_root`, `static_root`, `router_socket` are NOT taken from the environment on purpose; do not add env overrides.
- Router, engine and settings are files, not env vars, because git push deploys and the post-receive hook run with no riku env.
- `RIKU_ACME_ROOT` is read inside `RikuPaths::with_nginx_host` (lib.rs).
- Tests are in `src/tests.rs` and `*_tests.rs` siblings.

## riku-http
Job: one blocking reqwest client builder with rustls on ring. Depends on: none. Used by: riku-cli, riku-deploy, riku-supervisor.
Entry: `blocking_client_builder` (`src/lib.rs`).

- `src/lib.rs`: `blocking_client_builder` installs ring as the process rustls provider (ignores "already installed") and returns `reqwest::blocking::Client::builder()`.

Gotchas:
- reqwest is built with `rustls-no-provider`, so any client built without this function panics, even for plain http. Always use this builder.
- Separate from riku-util so `rk` never links reqwest.

## riku-error
Job: typed deploy errors. Depends on: none. Used by: riku-deploy, riku-plugins, riku-supervisor, riku.
Entry: `DeployError` (`src/lib.rs`).

- `src/lib.rs`: `DeployError::DeployInProgress(app)` (callers map to 409 and downcast with `anyhow::Error::downcast_ref`), `DeployError::ResourceExhausted(msg)` built by `resource_exhausted(stage, command, cause)`, a multi-line diagnostic that names `RIKU_MAX_MEMORY_MB` and `RIKU_MAX_CPU_SECONDS`.

Gotchas:
- Callers detect these through `downcast_ref::<DeployError>()` on an `anyhow::Error`; wrapping with `.context()` still works, converting to a string loses it.
- The remedy text hardcodes limit names and defaults (512 MB, 3600 s); keep it in step with `riku-util/src/resource_limits/mod.rs`.
- Inline `mod tests` at the bottom of `src/lib.rs` (not a `_tests.rs` sibling).

## riku-plugin-java
Job: runtime plugin binary for Maven and Gradle apps. Depends on: none. Used by: nobody; shipped as release asset `riku-plugin-java-<target>`, installed by `riku install-plugins --plugins java`.
Entry: `fn main` in `src/main.rs`, same argv[1] protocol as the shell runtime plugins.

- `src/main.rs`: `detect` (exit 0 if `pom.xml` or `build.gradle`), `build` (`mvn package -DskipTests --batch-mode`, or `./gradlew`/`gradle build -x test`), `env` prints `JAVA_OPTS=-Xmx512m`, `start` prints `java $JAVA_OPTS -jar target/*.jar` (Maven) or `build/libs/*.jar`, `destroy` no-op. Reads `RIKU_APP_PATH` (default `.`).

Gotchas:
- `start` prints a glob jar path; with more than one jar in `target/` or `build/libs/` (for example plain plus `-sources`) the command breaks.
- Maven is checked first when both build files exist.

## riku-plugin-clojure
Job: runtime plugin binary for Leiningen and Clojure CLI apps. Depends on: none. Used by: nobody; release asset `riku-plugin-clojure-<target>`, installed by `riku install-plugins --plugins clojure`.
Entry: `fn main` in `src/main.rs`.

- `src/main.rs`: `detect` (`project.clj` or `deps.edn`), `build` (`lein uberjar`; for deps.edn `clojure -T:build uber` only if `build.clj` exists, else no-op), `env` prints `JVM_OPTS=-Xmx512m`, `start` prints `java $JVM_OPTS -jar target/uberjar/*.jar` (lein) or `clojure -M -m core` (deps.edn), `destroy` no-op.

Gotchas:
- deps.edn `start` assumes the main namespace is `core`; apps with another name must override via Procfile.

## plugins/ (bundled runtime and event plugins)
Single-file bash runtime plugins, no extension: `node`, `python`, `ruby`, `go`, `rust-lang`. Each takes argv[1] in `detect|build|env|start|destroy`, reads `RIKU_APP_PATH` (fallback cwd). `detect` exit 0 means "mine". `env` prints KEY=VALUE lines, `start` prints the default start command. Triggers: node `package.json`; python `requirements.txt`/`pyproject.toml`/`setup.py`; ruby `Gemfile`; go `go.mod` or `*.go`; rust-lang `Cargo.toml`. node builds with nub (npm fallback, prints the install hint; `RIKU_NODE_VERSION` is not used yet); python builds with uv for `RIKU_PYTHON_VERSION` when set (`--python`; pip needs `python<version>` and fails naming the key; a venv made for another version is made again) (`uv sync --locked` with a `uv.lock`, else `uv pip install`; pip fallback with a hint) and a failed install stops the build.
- `plugins/riku-notify/`: event bundle (not a runtime). `riku-plugin.toml` (type notifier, subscribes `app.restarted`, `app.failed`, mode observe, network capability) and `bin/on-event` (argv `on_event`, JSON envelope on stdin, posts to webhook/Discord/Slack/Telegram per `RIKU_NOTIFY_*` env vars).
- Built into riku: the five runtimes and `riku-notify` are embedded with `include_bytes!` in `crates/riku-cli/src/apps/embedded_plugins.rs` and written out by `riku install-plugins`; java and clojure are downloaded from this riku's release tag and checked against their `.sha256` (`plugin_download.rs`), for the triple riku itself was built for (`RIKU_TARGET` from `crates/riku-cli/build.rs`, checked against `RELEASE_TRIPLES`; a test keeps that list equal to the `release.yml` matrix). A new plugin here must be added to `EMBEDDED` and to `riku_util::runtime_markers::BUNDLED_RUNTIMES` (a test checks they agree).
- Runtime selection: `RIKU_RUNTIME` override, else first alphabetical `detect` exit 0 (see riku-plugins).

## templates/ (nginx Tera templates)
Rendered only by riku-nginx. Embedded at compile time with `include_str!` and registered in `crates/riku-nginx/src/template.rs` (`register_templates`); a new template must be added to that list. The site entry template is chosen in `crates/riku-nginx/src/route_site.rs`.
- Entry templates: `nginx.conf.tera` (plain proxy site, port 80), `nginx_wsgi.conf.tera` (uWSGI site on the external port), `nginx_portmap.conf.tera` (proxy on a mapped external port), `nginx_static.conf.tera` (static root), `nginx_https_only.conf.tera` (port 80 redirect plus ACME location, used when HTTPS only).
- Fragments (via `{% include %}`): `nginx_upstream.conf.tera` (upstream block from `RIKU_BACKENDS`), `nginx_common.conf.tera` (body limits, `.git` block unless `RIKU_NGINX_ALLOW_GIT_FOLDERS`), `nginx_site_locations.conf.tera` (ACME plus static aliases), `nginx_acme.conf.tera` (http-01 location, `ACME_WWW`), `nginx_uwsgi.conf.tera` (uwsgi_pass and written-out uwsgi params).
- Values are sanitized in riku-nginx before they reach a template; backend addresses go through `backend_address`.
- There is no Caddy template here; Caddy site files are built in code (riku-caddy).

## tests/
- `tests/integration_tests/`: one cargo test target `integration_tests` (`mod.rs` lists the groups). Groups: `cli_tests`, `e2e_tests`, `nginx_tests`, `plugin_tests`, `push_tests` (git push through `riku git-receive-pack`, ext:: transport), `regression_tests`, `remote_tests`, `resilience_tests`, `sandbox_tests`, `security_tests`, `smoke_tests`, `supervisor_tests`. `mock_supervisor.rs` is a shared stand-in that listens on enabled workers' ports so deploy health checks pass.
- `tests/stress_tests.rs`: separate cargo target `stress_tests` (deploy lock, plugin process-group reaping, build-phase rlimits). Includes `mock_supervisor` itself.
- Unit tests sit beside the code in `*_tests.rs` siblings, not here.
- `tests/deploy-smoke/`: `test-all.sh`, `quick-test.sh <app> <runtime>`; deploy a generated app against a real riku. `tests/run-all-tests.sh` runs unit, integration and deploy tests.
- `tests/e2e/cases/NN-*.sh` plus `lib.sh`: shell e2e cases, run inside the Docker image (`docker/Dockerfile.test`, `docker/test-entrypoint.sh`; locally `scripts/run-docker-tests.sh`). Each case gets its own temp `RIKU_ROOT`.
- `tests/stress/`: shell audit suite for the supervisor (`run_all.sh`, lifecycle, leak, chaos, resource limit scripts, `container/`, `dashboard/`, `bad_tenant_app/`); not part of `cargo test`.
- `tests/demo/`: long-running sandbox container with 5 demo apps (`run_demo.sh`, `stop_demo.sh`, `compose.yml`); for humans, not CI.
- Run: `cargo test` (all), `cargo test --test integration_tests`, `cargo test --test stress_tests`, `cargo test -p <crate>`.

## Release and CI
- `.github/workflows/ci.yml`: tests, clippy, dashboard checks, integration and Docker e2e on push and PR. It publishes nothing.
- `.github/workflows/release.yml`: on a `v*` tag. Checks the tag matches the workspace version, builds every target, publishes `<bin>-<platform>.tar.gz` plus `.sha256` and the raw `riku-plugin-<name>-<triple>` plugins. Linux builds use cargo-zigbuild: gnu triples link against glibc 2.31 (the `glibc` matrix field), musl ones are static; platforms are `linux-{amd64,arm64,armv7,armv6}` with a `-musl` variant. A tag with a `-` (rc) is published as a prerelease, so `latest` never points at it.
- `scripts/install.sh`: one-line installer. `pick_platform` picks the release build (see `scripts/README.md`); `main` downloads and checks it. Sourcing it with `RIKU_INSTALL_LIB=1` defines the functions only; `tests/install/pick_platform.sh` tests the choice with mocked `uname` and `getconf`, run in the `ci` job with shellcheck.

## riku-bundle
Job: plugin bundle manifest (`riku-plugin.toml`) parsing/validation and Ed25519 signing. No server knowledge, usable by `rk`. Depends on: none. Used by: riku-plugins, riku-deploy, riku-remote, riku-client.
Entry: `PluginManifest::from_dir` / `from_toml_str` (`src/manifest.rs`), `RouterSection`, `signing::{Keypair, verify, decode_pubkey}`, consts `RIKU_PLUGIN_API` (1), `ROUTER_PROTOCOL` (2) in `src/lib.rs`.

- `src/manifest.rs`: manifest structs (`PluginType`, `Capabilities`, `EventSubscription`, `FilterSubscription`, `UiPanel`, `Lifecycle`, `PluginManifest`). Key: `validate`, `entry_path`, `subscribes_to`. Contract: `validate` refuses `api != RIKU_PLUGIN_API`, names outside `[A-Za-z0-9_-]`, and any `entry` that is empty, absolute or has `..`. `from_toml_str` always validates.
- `src/router_section.rs`: the `[router]` table (`protocol`, `graceful_reload`). Riku only drives routers whose `protocol == ROUTER_PROTOCOL`.
- `src/signing.rs`: hex Ed25519 keypair, `verify(message, signature, pubkey) -> bool`. Contract: a signature covers the entry bytes only, not the rest of the bundle; trust in the key is the caller's job (keyring lives in riku-plugins).

Gotchas:
- `manifest.checksum` and `signature` are only data here; the install gate that enforces them is `riku-plugins/src/install.rs`.

## riku-caddy
Job: render and locate Caddy site files for the Caddy router. Depends on: riku-config, riku-util. Used by: riku-route, riku-cli.
Entry: `render_site`, `site_path` (`src/site.rs`); `import_line`, `has_import`, `SYSTEM_CADDYFILE`, `ADMIN_ADDRESS` (`src/host.rs`).

- `src/site.rs`: `render_site(app, &RoutePlan)` gives `addr1, addr2 { reverse_proxy up1 up2 }`, plus `http://:<port>` for `RoutePlan.open_port`. `paths.rs` `render_paths` for the shared path site (`redir /<app> /<app>/`, `handle_path /<app>/*` with `X-Forwarded-Prefix`). Contract: errors on no backend or any unix backend (TCP only); `HttpsPolicy::AlsoPlainHttp` adds an `http://<domain>` address per domain. `site_path` is `paths.caddy_root/<app>.caddy`.
- `src/host.rs`: the single `import` line the system Caddyfile needs (`/etc/caddy/Caddyfile`), and the admin socket address `unix//run/caddy/admin.sock`.

Gotchas:
- This crate never writes a file or reloads Caddy. The helper in riku-route does both as root (`riku-route/src/caddy_apply.rs`).

## riku-nginx
Job: nginx config rendering from a checked route, plus what the deploy user still does: publish static files, issue certs. Depends on: riku-config, riku-util. Used by: riku-route, riku-deploy, riku-cli, riku.
Entry: `render_route_site`, `RouteSite`, `SiteFiles`, `SiteShape` (`src/route_site.rs`); `default_backends`, `bind_address`, `upstream_port_number` (`src/backends.rs`); `publish_static`, `remove_static`, `static_locations` (`src/static_site.rs`); `ensure_ssl_certificates` (`src/ssl.rs`); `render_paths_site`, `PathsSite`, `PathApp` (`src/paths_site.rs`, template `nginx_paths.conf.tera`: one `location /<app>/` per app, prefix stripped, `X-Forwarded-Prefix`); `issue_single` (`src/ssl_single.rs`); `remove_deploy_files` (`src/remove.rs`); `breaks_directive` (`src/sanitize.rs`); `write_include_file`, `include_line`, `includes_riku_sites` (`src/host.rs`); `needs_catch_all` (`src/default_server.rs`); `warn_about_app_nginx_configs` (`src/lib.rs`).

- `src/route_site.rs`: Tera render from typed `RouteSite` and `SiteFiles`; `open_port` adds `listen <port>` lines (`nginx_open_port.conf.tera`; with no domains the site listens only there as `server_name _`); `also_tls` adds a 443 listener with no redirect (`nginx_also_tls.conf.tera`, included by the plain and portmap templates) for a fallback domain that has a pair. Contract: runs as root in the helper, so reads no file, runs no program; unquoted values still go through the directive character refusal.
- `src/template.rs`: registers the ten `templates/nginx*.conf.tera` files via `include_str!`; `backend_address` refuses directive characters and whitespace in a unix socket path.
- `src/sanitize.rs`: one deny-list `breaks_directive`, shared with riku-route's gate so gate and renderer cannot disagree.
- `src/backends.rs`: reads ENV (`RIKU_BIND_ADDRESS`, `RIKU_UPSTREAM_PORT`, `PORT`, `RIKU_NGINX_SOCKET`, `RIKU_NGINX_PORTMAP`) into backends. Contract: a present but malformed bind address or port is an error; `localhost` is not accepted. `upstream_name` is `riku_<hex of app bytes>` (nginx compares upstream names case-insensitively).
- `src/static_site.rs`, `src/static_copy.rs`: copy `RIKU_NGINX_STATIC` / `RIKU_NGINX_STATIC_PATHS` out of the checkout to `<static_root>/<app>~<stamp>` and swap the `<app>` symlink. Contract: every path and symlink canonicalized and must stay inside the checkout; dirs `2750`, files `0640`.
- `src/ssl.rs`: acme.sh (http-01 files in `acme_root`) else self-signed via openssl. Writes `<nginx_root>/<app>.{key,crt,fullchain.crt}`. Contract: validates app name; key always chmod `0600`.
- `src/ssl_single.rs`: `issue_single` for a fallback domain: reuses an existing key and full chain, else one acme.sh issue; never self-signed (a failure means plain HTTP).
- `src/host.rs`, `src/default_server.rs`: root-owned include file holding one `include /etc/riku/nginx/*.conf;` and optionally a catch-all server when the host has no port 80 default.
- `src/remove.rs`: removes the deploy user's cert files and sockets only.
- `src/test_support.rs`: test helper.

Gotchas:
- Two cert locations: this crate writes certs under `nginx_root` (deploy user); the helper writes the TLS pair it receives under `cert_root` (riku-route `nginx_render.rs`).
- The `ensure_ssl_certificates` return bool is "cert pair is usable", not "issued by acme".
- Apps cannot ship their own nginx config; `warn_about_app_nginx_configs` only warns about `nginx.conf` and three variants.

## riku-route
Job: the router helper's protocol, route gate, and nginx/Caddy apply as root. Depends on: riku-caddy, riku-config, riku-container, riku-nginx, riku-util. Used by: riku, riku-cli, riku-deploy.
Entry: `check_nginx_route`, `check_caddy_route`, `CheckedRoute` (`src/check.rs`); `Request`, `Reply`, `Stage`, `RouterKind`, `parse_request`, `encode_request`, `parse_reply`, `encode_reply`, `PROTOCOL_VERSION` (`src/protocol.rs`); `Route`, `RouteBackend`, `SiteKind`, `Listen`, `Tls` (`src/schema.rs`); `run`, `bind_socket`, `restrict_umask` (`src/listen.rs`); `serve_one`, `REQUEST_BUDGET` (`src/server.rs`); `read_bounded`, `MAX_MESSAGE_BYTES` (`src/bounded.rs`); `answer` (`src/answer.rs`); `apply_nginx`, `remove_nginx`, `NginxRoots`; `apply_caddy`, `remove_caddy`, `CaddyRoots`.

- `src/schema.rs`: wire route structs, all `deny_unknown_fields`. Parsed, not checked; only the gate says safe. `Route.open_port` (port mode) is left out of the JSON when unset, so an older helper keeps taking routes that do not use it; the gate refuses it below 1024, at 2019, and on uWSGI or static sites. `port_wait.rs` waits up to 2 s after a reload for `open_port` to be in LISTEN and the apply restores the old files when it is not. `Request::ApplyPaths { router, listen, apps: Vec<PathApp> }` replaces the shared path site: `paths_check.rs` (privileged port, `AppName`, duplicates, backend rule), `paths_apply.rs` writes `_paths.conf` or `_paths.caddy` (no app name can start with `_`), waits for the shared port, restores on failure; an empty list removes the file.
- `src/protocol.rs`: one JSON object per request/reply, `"v": 1`, ops `apply`, `remove`, `status`. Missing `router` means nginx. Refuses other versions.
- `src/check.rs`, `check_text.rs`, `check_tls.rs`: the single gate. Domains lowercase `Domain` or `_`; at least one domain; backends must be loopback TCP or `unix_copy >= 1`; listen port 80 or >= 1024; TLS pair re-encoded from parsed DER (cert chain in one field, exactly one matching key in the other); `https: only` needs TLS (nginx). Caddy refuses TLS pair, `allow_git_folders`, custom listen, unix/wsgi/portmap/static, and `_`.
- `src/listen.rs`, `src/server.rs`, `src/bounded.rs`: socket root:deploy-group `0660`, bound under umask `0177`. `serve_one` reads `SO_PEERCRED` and silently drops any uid except the deploy user and 0. Input capped at 1 MiB and 10 s, one connection at a time.
- `src/answer.rs`: validates the app name as `AppName`, dispatches to nginx or Caddy side by `RouterKind`.
- `src/nginx_apply.rs`, `nginx_render.rs`, `nginx_roots.rs`, `nginx_run.rs`: snapshot old files, write new, `nginx -t`, reload, restore on any failure. `NginxRoots::production()` uses riku-config constants and picks `systemctl reload` if `SYSTEMD_RUNTIME_DIR` exists, else `nginx -s reload`. Tests pass temp roots.
- `src/caddy_apply.rs`: writes `<site root>/<app>.caddy`, reloads via Caddy admin unix socket, restores previous file if reload fails.
- `src/site_files.rs`: write via `.new` sibling and rename; modes set on the open file, never left to umask; `Snapshot` for rollback.
- `src/caddy_test_server.rs`, `test_pems.rs`: test helpers (`test-server` feature / cfg(test)).

Gotchas:
- The gate runs twice by design: the deploy side calls `check_caddy_route` for an early error, the helper calls the gate again on receipt. Never skip the helper-side call.
- `nginx_run::check_config` trusts exit status only; the old deploy-side wrapper accepted "syntax is ok" text.
- No request field is a path. New fields must not become one.

## riku-plugins
Job: plugin system: runtime plugins, hook plugins, manifest bundles (addon/router/events/filters/ui), sandbox, install, marketplace, trust keyring. Depends on: riku-config, riku-util, riku-error, riku-bundle. Used by: riku-cli, riku-dashboard, riku-deploy, riku-supervisor, riku.
Entry: `PluginManager::run_hook`, `PluginHook`, `HookContext` (`src/manager.rs`, `src/hooks.rs`); `runtime::{discover, detect, build, build_with_limits, destroy, get_env, get_start_cmd}` (`src/runtime.rs`); `PluginInstaller` (`src/install.rs`); `AddonService` (`src/addon/`); `EventBus` (`src/events/`); `FilterBus` (`src/filters/`); `router::run_verb` (`src/router/`); `run_ui_panel` (`src/ui/`); `MarketplaceService`; `bundles::{find_bundles, find_addon, find_router, find_plugin}`; `plugin_data_path`; `Keyring`, `Lockfile`.

- `src/discovery.rs`: `validate_plugin_name` (no `/`, `\`, `..`, empty), `list_plugins` (executable files in `plugin_root`), `plugin_exists`.
- `src/hooks.rs`, `src/manager.rs`: legacy name-based hooks `riku-pre-deploy|pre-build|post-build|post-deploy` in `plugin_root`. Contract: pre-deploy, pre-build, post-build failure or timeout aborts; post-deploy only warns. Runs in its own process group.
- `src/runtime.rs`: runtime plugins are executables in `plugin_root` not named `riku-*`; verbs `detect|build|env|start`. Order: `RIKU_RUNTIME` ENV override, else alphabetical first `detect` exit 0. Hidden files (an interrupted install's `.<name>.<pid>.<serial>.new`) are never plugins. `RIKU_RUNTIME` naming no installed plugin is the typed `MissingRuntime` error.
- `src/runtime_bundles.rs`: `add_runtime_bundles` adds bundle dirs with `type = "runtime"` to `discover`; a plain file wins a name clash.
- `src/executor.rs`: timeout and output helpers. Reads `RIKU_PLUGIN_TIMEOUT` (`plugin_timeout`). Re-exports ETXTBSY-retry spawn from riku-util.
- `src/bundles.rs`: finds `riku-plugin.toml` dirs; invalid manifests are skipped with a warning.
- `src/addon/`: `state.rs` instance registry `~/.riku/addons/instances/<instance>.toml`; `dispatch.rs` runs a verb (JSON line on stdin, JSON on stdout); `service.rs` bind/unbind env merge. Contract: bind rewrites only the keys it owns and leaves other ENV lines byte-for-byte.
- `src/events/`: `EventName`, `EventEnvelope`, `EventBus::emit` (verb `on_event`). Observe mode only; subscriber failure never fatal; `gate` mode runs as observe and logs that veto is not enforced.
- `src/filters/`: `FilterBus::apply` runs `on_filter` in priority order; any failure passes the input through unchanged.
- `src/router/`: raw verb dispatch (`configure`/`reload`) to a third party router plugin; stdout discarded, success is exit code. Active router is named in `<riku root>/ROUTER` (default `nginx`); that choice lives in riku-deploy.
- `src/ui/`: `ui_panel` verb, structured JSON only; any failure returns an empty panel.
- `src/sandbox/`: Landlock (fs and TCP net, kernel >= 6.7) plus `PR_SET_NO_NEW_PRIVS` applied in `pre_exec` from the manifest `[capabilities]`. `writes` names `app_dir|data_dir|env_dir` resolved in `paths.rs`. Linux only, otherwise runs unconfined with a stderr warning.
- `src/install.rs`, `lockfile.rs`, `signing.rs`: install from local dir or git URL; pinned `checksum` mismatch rejected; a `signature` must verify against a key in `~/.riku/trusted-keys.toml` else rejected. Lockfile `~/.riku/riku-plugins.lock`. A bundle with neither checksum nor signature installs, flagged unverified.
- `src/marketplace/`: `~/.riku/marketplaces.toml`, clones to `~/.riku/marketplaces/<name>/`, index `marketplace.toml`; payload pulled only on `add`.
- `src/plugin_data.rs`: per-plugin scratch dir `data_root/plugin-data/<name>/`, created lazily.

Gotchas:
- Two plugin systems coexist: name-based hooks/runtimes (no manifest, no sandbox) and manifest bundles (sandboxed). Legacy ones are spawned by `manager.rs`/`runtime.rs`, not through `sandbox::harden`.
- `PluginManager::run_hook` sets the exec bit on a hook file that lacks it instead of skipping it.
- Sandbox callers: addon dispatch, events bus, filters bus, ui dispatch, install (`harden` in `install.rs`). A new seam must call `sandbox::harden` or it runs unconfined.
- Env keys read here: `RIKU_PLUGIN_TIMEOUT`. Env passed to plugins: `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`, `RIKU_HOOK`, `RIKU_DETECTED_RUNTIME`(hooks, runtimes), plus `RIKU_PLUGIN_NAME`, `RIKU_PLUGIN_DATA_PATH`, `RIKU_ADDON_DATA_PATH` appear in bundle dispatch code.

## riku-supervisor
Job: process supervisor daemon: spawns and restarts workers from TOML configs, health checks, cron, container watch, stats, log rotation, loopback HTTP health/control plane. Depends on: riku-config, riku-util, riku-http, riku-error, riku-plugins, riku-container. Used by: riku-cli, riku-dashboard, riku-deploy, riku.
Entry: `Supervisor::new`, `with_actions`, `run` (`src/daemon/`); `WorkerConfig`, `create_worker_config`, `app_worker_configs` (`src/config/`); `ProcessManager` (`src/process/mod.rs`); `read_stats_file` (`src/stats/repository.rs`); `ControlActions` trait (`src/health/actions.rs`); `start_health_server` (`src/health/mod.rs`); `app_has_live_worker`, `process_has_live_worker` (`src/process/pid_file.rs`); `setup_signal_handlers`, `is_running` (`src/lib.rs`).

- `src/config/`: worker TOML model (`[worker] app kind command ordinal`, `[env]`, `[options]`, isolation, health check). `ownership.rs` decides which files in `workers-available/` or `workers-enabled/` belong to an app, by `[worker] app`, falling back to name only for `<app>@<kind>.<n>` files (unowned means not deleted).
- `src/daemon/`: `init.rs` constructor (riku root is `config_dir` parent: `stats.json`, `supervisor.pid`, `control.token`, `logs/`); `startup.rs` flock PID lock, cgroup diagnostic, socket dir, health port; `config_watcher.rs` + `config_file.rs` notify watcher of `workers-enabled`; `event_loop.rs` tick, SIGHUP reload, shutdown; `periodic.rs`, `maintenance.rs`, `cron_tasks.rs` (`load_cron_jobs` schedules an ordered `(kind, command)` list; `config_watcher::load_app_cron_jobs` gets it from `app_config::load_cron_entries` with `RikuPaths::from_env`), `container_tick.rs`, `image_watch.rs` (60 s registry tag pull via `pull_service`; `RIKU_WATCH_SERVICES` from the merged env).
- `src/process/`: `spawn.rs` spawn (own process group, drops to configured gid then uid, execs `riku __ns-shim` when isolation on); `stop.rs` stop whole group then cgroup; `health_check.rs` + `health_interval.rs` monitor and restart; `orchestration.rs` + `generation.rs` canary generations with probe, promote or rollback; `pid_file.rs` marker files `<riku_root>/pids/<process_id>.<pid>`; `isolation/` namespace shim (`exec_isolated`, Linux only).
- `src/containers/`: `ContainerWatch` adopts and watches engine containers via riku-container: `feed.rs` events, `listing.rs` 30 s listing, `log_pull.rs` + `log_thread.rs` log copying, `restart_policy.rs`/`restarts.rs`/`tally.rs`/`candidates.rs` restarts with backoff, `stats.rs` rows. Contract: engine output and labels untrusted; names pass as validated types; log files opened `O_NOFOLLOW`, path under log root, output capped per stream. `mock_*`, `test_ticks.rs` are test helpers.
- `src/health/`: axum on `127.0.0.1:<RIKU_HEALTH_PORT, default 9091>`. Open GET `/health`, `/metrics`, `/metrics/apps[/{app}]`, `/metrics/stream` (SSE), `/plugins`, `/hooks` with CORS limited to `RIKU_DASHBOARD_ORIGIN` (default `http://127.0.0.1:3000`). Mutating `/control/apps...` and `/control/plugins/install` need `Authorization: Bearer <control.token>` (`auth.rs`, token file `0600`, constant-time compare, created if missing). `actions.rs` trait is how higher crates inject deploy/restart/destroy; default `NoopControlActions`.
- `src/stats/`: `StatsManager` in memory, `write_stats_to_file` writes `stats.json` (5 s), `repository.rs` is the CLI side reader, `resources.rs` /proc on Linux (stub elsewhere).
- `src/cgroups/`: cgroup v2 `/sys/fs/cgroup/riku/apps/<id>/`, `memory.max`, `cpu.max`; `verify_root_writable` at startup.
- `src/log_rotation/`: size based rotation, run every 300 s.
- `src/lib.rs`: signal handlers, SIGHUP listener, re-exports `cron` and `resource_limits` from riku-util.

Gotchas:
- The supervisor is a lower crate: it must not call riku-cli or riku-deploy; behaviour reaches it through `ControlActions` injected by the binary.
- The main loop is single threaded; `spawn_process` must not block (the reason isolation is a separate exec, see `process/isolation/mod.rs`).
- A worker config runs only if its filename equals `WorkerName::file_name()` of its `[worker]` table (`daemon/config_file.rs`); a misnamed one is logged and ignored.
- `RIKU_ENV` defaults to production: missing cgroup v2 writes an error to stderr; set `development` or `dev` to downgrade it to a warning (`daemon/startup.rs`).
- Health server bind failure and cgroup check failure are non-fatal; PID lock failure is fatal.
- Env read: `RIKU_ENV`, `RIKU_HEALTH_PORT`, `RIKU_DASHBOARD_ORIGIN`; limits via `ResourceLimits::from_env` (`RIKU_MAX_*`, `RIKU_ISOLATION_*`).

## riku-deploy
Job: deploy orchestration (service layer): sync checkout, build, start workers or containers, route, release history, backup. Depends on: riku-bundle, riku-config, riku-util, riku-http, riku-error, riku-nginx, riku-route, riku-plugins, riku-supervisor, riku-container. Used by: riku, riku-cli, riku-dashboard.
Entry: `do_deploy`, `rollback` (src/lib.rs); `spawn_app` (supervisor_ctl.rs); `create_workers_generic` (workers.rs); `router::{configure, apply, remove, renew, check, build_route}`; `containers::*` (deploy_containers, restart_service, scale_service, stop_service, destroy_containers, update_service, restart_copy, open_engine); `lock::{acquire, acquire_for_deploy, acquire_for_restart, is_locked}`; `ReleaseLog`; `BackupService`.

- `src/lib.rs`: `do_deploy` pipeline. Takes deploy lock, git sync, loads ENV, branches container app vs Procfile app, then `finish_deploy`. Re-exports dependency crates as `config`, `error`, `nginx`, `plugins`, `supervisor`, `util` (crate-internal aliases). Contract: container apps refuse scaling `deltas`; `rollback` redeploys a prior sha from `ReleaseLog`.
- `src/deploy_env.rs`: `load_deploy_env` (merged env via `app_config::load_env`, warns on invalid, renamed and legacy keys, sets env dir 0700 and ENV 0600), `load_deploy_processes` (`app_config::load_processes`, logged), `refuse_container_processes` (container apps refuse riku.toml `[processes]`/`[cron]`). `do_deploy` loads each once.
- `src/deploy_finish.rs`: `finish_deploy`. Post-deploy hooks, release record, tells pusher where app is live (`live_urls.rs` `live_lines`: one line per domain, the fallback certificate outcome, or how to add a domain). Never fails the deploy.
- `src/procfile_deploy.rs`: `deploy_procfile_app` takes the process list (riku.toml or Procfile). Runtime plugin detect/build (a missing shipped runtime names `rk plugins install <name>`), preflight/release, scaling deltas, workers, router, returns runtime name.
- `src/env_setup.rs`: `run_preflight`, `run_release`, `write_live_env`, `update_env_and_redeploy`. Runs Procfile commands via `sh -c` with resource limits applied in `pre_exec`.
- `src/git_ops.rs`: `sync_app_repo`, `git_fetch`, `git_reset`, `git_update_submodules`, `head_sha`. Shells out to git.
- `src/hooks.rs`: `run_pre_deploy`, `run_pre_build`, `run_post_build`, `run_post_deploy`. Publishes the lifecycle event, then runs plugin hooks via `PluginManager`. Pre hooks abort the deploy.
- `src/lock.rs`: per-app `flock` at deploy time, holder tag (`operation` or `restart`) written in the file. Contract: `acquire` is non-blocking, `acquire_for_deploy` waits out a supervisor restart (up to `HEALTHCHECK_WAIT` + 30s); EINTR retried; 1s grace against fork-inherited fds. Hold the returned `File` for the whole operation.
- `src/releases.rs`: `ReleaseLog` (`record`, `list`, `previous`). File `releases/<app>.log`, lines `<unix_ts> <sha>`.
- `src/scaling.rs`: `apply_scaling_deltas`. Reads and writes the app `SCALING` file, prunes worker symlinks.
- `src/supervisor_ctl.rs`: `ensure_supervisor_running`, `notify_supervisor_reload` (SIGHUP via nix), `resolve_riku_bin`, `spawn_app`, `read_app_env` (merged env via `app_config::load_env`; feeds spawn, router renew and web rollout). Contract: pid from `<riku_root>/supervisor.pid`; binary is `ROOT_BINARY` if present, else `current_exe()`.
- `src/app_stop.rs`: `stop_procfile_app`. Removes enabled worker configs and riku-made uWSGI sockets (`riku stop`).
- `src/worker_control.rs`: `restart_worker`, `delete_worker` for one `kind` + ordinal. Validates app name and kind before building paths.
- `src/teardown.rs`: `plugin_destroy`. Runtime plugin cleanup on destroy, detected with the merged env so `runtime.name` in riku.toml counts.
- `src/backup.rs`: `BackupService::{backup, restore}`. tar.gz of `apps/`, `envs/`, `data/`, `repos/<app>.git`. Restore validates every member (no absolute, no `..`, must sit under this app's dirs).
- `src/service_update.rs`: `pull_service`. Supervisor image watch: pull, compare image ids, replace only that service's copies.
- `src/mock_engine.rs`: test doubles (script engine recording argv, parsed project). Test builds only.
- `src/workers.rs`: `create_workers_generic`, `create_workers_with` (both take the process list; they never read the Procfile), `read_scaling_count`. Web-facing kinds via rolling replace, other kinds rewritten stop-first.
- `src/workers/`: Procfile worker mechanics. `files.rs` write/enable/remove worker TOML (`workers_available` + `workers_enabled` symlink); `web_env.rs` per-copy PORT or uWSGI socket env and persisted route keys (`RIKU_NGINX_PORTMAP` or `RIKU_NGINX_WSGI`, internal markers; `RIKU_NGINX_EXTERNAL_PORT` is no longer written and is dropped where found) (`is_web_facing`, `is_wsgi_kind`, `drop_route_keys`, `clear_route_env`); `process_runtime.rs` `CopyRuntime` for supervised copies; `web_rollout.rs` rolling deploy of web kinds; `stop_wait.rs` polls supervisor pid files; `ports.rs`, `pinned.rs`, `socket.rs` port and socket checks that run before any config change; `scaling.rs` reads SCALING; `mock_supervisor.rs` test double.
- `src/rolling/`: rolling replacement algorithm independent of copy type. `replace.rs` traits `CopyRuntime`, `RouteSink`, `replace_all`, `LiveCopy`, `CopyHandle`; `scale.rs` `scale_to`, add/drain one copy; `health.rs` health wait (`HEALTHCHECK_WAIT` 120s, `HealthTarget`); `probe.rs` HTTP/TCP/unix probes; `mock_copies.rs` doubles.
- `src/router.rs` + `src/router/`: router seam. `router.rs` `configure`, `apply`, `apply_to` dispatch on `config::router::Router` (Nginx, Caddy, Plugin); `backends.rs` backends from enabled worker configs or running containers; `route_build.rs` + `route_env.rs` build the checked `riku_route::Request` from ENV, copies and certs; `helper_client.rs` unix socket to the root router helper; `nginx_site.rs`, `caddy.rs`, `plugin.rs` per-router apply; `reload.rs` confirmed reload and `ERROR_FILE` failure record; `remove.rs`, `renew.rs`, `check.rs`, `fallback.rs`. Routing modes: `app_routing.rs` `resolve` (`AppRouting { domains, fallback, port, mode, legacy, fallback_skipped }`; no `[routing]` keeps the old routing; fallback domain for an app without one; port mode when no domain and no fallback; path mode takes no domains or port; router plugins get domain mode only); `routing_state.rs` `<root>/routing/state.toml` per app `Entry { mode, port, picked, tls, cert_for }` (`cert_for`: the fallback name riku's pair covers), changed only inside `with_routing_lock` (server wide flock); `restore` puts back the entry from before an apply that failed; `port_rules.rs` `check_port`, `pick_port` (below 1024, Caddy admin 2019, the shared port, another app's port and any port already listening are refused); LISTEN ports from riku-util `listening.rs`. `routing_env.rs` `routing_env(.., Record::{Save, Peek})` resolves and lays the fallback domain and the port mode port over a copy of the env (`RIKU_DOMAINS` set, `RIKU_HTTPS_ONLY` dropped, `RIKU_ROUTING_PORT` set to the resolved port, which `route_build.rs` sends as `open_port`), used by `apply_to`, `check`, `renew` and the push output; `fallback_tls.rs` fallback certificate on nginx with a one hour wait after a failure, Caddy marked `by_caddy`; `drop_stale_pair` deletes a pair that covers another name than the one served (fallback to own domain, own domain to fallback, server_ip change). `nginx_site.rs` installs over HTTP first when a wanted pair is missing, then issues, then installs with TLS. `remove` drops the routing entry. `paths.rs` path mode: `route_by_path` drops the app's own site and publishes the path site (refused for uWSGI and container apps); `publish_paths` lists path apps from the routing state, never by resolving every app, and sends nothing when there are none unless an app just left path mode, holding the routing lock until the helper answers; `check_shared_port` refuses a shared port another program listens on before the path site exists; called from `apply_to` and `remove`.
- `src/containers/`: container app lifecycle. `mod.rs` re-exports (`AppName`, `ServiceName`, `CopyIndex` come from riku-container); `app.rs` `is_container_app`, `load_project`, `read_scaling`, `SINGLE_IMAGE_PORT` (8000); `context.rs` `ContainerApp` (everything one operation needs, scratch dir 0700); `deploy.rs` `deploy_containers`, `CONTAINER_RUNTIME`; `images.rs` registry login + pull/build; `release.rs` Procfile `release` in a one-off container; `rollout.rs` replace copies in start order; `runtime.rs` `CopyRuntime` for a service; `spec.rs` `copy_spec` run spec per copy; `start.rs` `start_copy`; `control.rs` restart/stop/scale without rebuild; `copy_restart.rs` supervisor restarts one dead copy; `image_update.rs` follow registry tag; `removal.rs` stop/remove copies, destroy; `retire.rs` + `procfile_workers.rs` park and stop Procfile workers when an app switches to containers; `engine_open.rs` `open_engine`, `EngineUnreachable`; `routes.rs`, `ports.rs`, `worker_files.rs`; `mock_state.rs` (+ `.sh`), `mock_workers.rs` test doubles.

Gotchas:
- Two worker kinds of copies: Procfile copies are `workers_enabled/<app>@<kind>.<n>.toml`, container copies are engine containers with `riku.*` labels. `rolling/` serves both through `CopyRuntime`, so change ordering rules there once.
- Container apps are detected by `RIKU_RUNTIME=container`, a compose file (`COMPOSE_FILE` in ENV picks another name) or a Containerfile/Dockerfile (`containers/app.rs`). A switch from Procfile to containers parks old workers as `<name>.toml.migrating`.
- `lock.rs` file is keyed by app name; container control ops (`containers/control.rs`) and `copy_restart.rs` take it too, so they conflict with a deploy by design.
- `resolve_riku_bin` returns `None` (supervisor not started) when there is no `ROOT_BINARY` and `current_exe()` fails; it never hands a bare `riku` to a `$PATH` lookup.
- Registry login (`containers/images.rs`) stores credentials in the engine user's shared auth file; last login wins across apps.
- `start_copy` re-walks bind sources for symlinks right before `run` (check then use, accepted).
- `RIKU_*` app env keys are not passed to containers (`containers/spec.rs`); the compose project label is dropped.
- Health and probe code only hits 127.0.0.1, forces paths to start with `/`, follows no redirects, ignores proxy env.
- Router is a host singleton from `config::router::Router::load` (settings.toml, then legacy `ROUTER`, default nginx). nginx and Caddy go through the root helper socket; Caddy backends are rewritten to loopback. A plugin must declare router protocol 2.
- `src/README.md` lists `ensure_host_ready` / `router/host_guard.rs`; neither exists in the source.
- Crate comments say `?` is avoided; `#[allow(clippy::question_mark)]` marks that. Unit tests live in sibling `*_tests.rs` files.

## riku-container
Job: container engine driver, compose file parser and validated name/port/mount types. Depends on: riku-config, riku-util. Used by: riku-deploy, riku-cli, riku-route, riku-supervisor.
Entry: `engine::{Engine, detect, RunSpec, BuildSpec, ContainerInfo, ContainerState, ContainerId, ContainerStats}`; `compose::{read_project, single_image_project, find_compose_file, ComposeProject, ServiceDef, check_scale, replace_order, routed_http}`; `types::{AppName, ServiceName, CopyIndex, ContainerName, NetworkName, labels}` (src/lib.rs has the three `pub mod`).

- `src/types/`: validated value types. `names.rs` name newtypes and `labels::*` constants (`riku.app`, `riku.service`, `riku.copy`, `riku.public`, `riku.http`, `riku.health`, `riku.port`, `riku.restart`, compose project label), `check_app_name`; `ports.rs` `PortBinding`, `HostIp`, `Protocol`; `mounts.rs` `Mount`, `MountSource`; `lifecycle.rs` `StopSignal`, `RestartPolicy`, `HealthCheck`, `DEFAULT_STOP_GRACE` (30s). Contract: constructors enforce `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, so no leading `-`, separator or whitespace.
- `src/engine/mod.rs`: re-exports; the driver needs Docker 23+ (or CLI compatible engine). Every value is one argv element, no shell.
- `src/engine/detect.rs`: `detect`, `EngineBinary`, `KNOWN_ENGINES` (docker, podman, nerdctl, finch). `CONTAINER_ENGINE` must be an absolute executable or a bare name looked up on a supplied search path; relative path entries skipped.
- `src/engine/identity.rs`: `Engine::open`, `with_settings`, `ping`, `parse_identity`. Probes the binary first. Timeouts come from `riku_config::settings::EngineSettings`.
- `src/engine/probe.rs`: `REQUIRED_COMMANDS`, `answers_version`, `exits_zero_within`, `first_unanswered`. Checks the engine answers the commands riku uses.
- `src/engine/control.rs`: `Engine::{run, stop, kill, remove, wait, pull}`. `run` builds args with `run_args.rs`, passes env through a 0600 file from `env_file.rs`, validates with `spec_check.rs`.
- `src/engine/run_spec.rs`: `RunSpec`, `PullPolicy`. `run_args.rs` turns it into `run -d` argv. `spec_check.rs` refuses values the engine could read as flags.
- `src/engine/build.rs`: `BuildSpec`, `Engine::build`. Context path must be absolute, no `..`.
- `src/engine/inspect.rs` + `inspect_parse.rs`: `Engine::{list, inspect}`, `ContainerInfo`, `ContainerState`. Inspect JSON is untrusted, ids become `ContainerId`.
- `src/engine/snapshot.rs`: `fetch_present`. Batch inspect that tolerates containers removed mid-read; a daemon failure is never treated as "gone".
- `src/engine/lookup.rs`: `owner_of`, `id_of`, `list_project`, `image_id` by name, project or image ref.
- `src/engine/resources.rs`: `ensure_network`, `volume_exists`, `volume_labels`, `remove_network`. Existence is an exact name match in a listing.
- `src/engine/bounded.rs` + `output.rs` + `streamed.rs`: process helpers. Calls run in their own process group with a time limit; the group is signalled on timeout. `streamed.rs` is for pulls/builds that print progress to the operator.
- `src/engine/logs.rs`, `events.rs`, `stats.rs`, `login.rs`, `ids.rs`: logs (`LogRange`, `TimestampedLogs`, `ContainerGone`), event stream parsing, `ContainerStats`, `Engine::login` (token on stdin only), `ContainerId` (hex only).
- `src/engine/fake.rs`, `fixture.rs`, `docker_tests.rs`: test-only fake engines; `fixture.rs` records argv after the probe.
- `src/compose/mod.rs`: re-exports. `raw.rs` reads file into a checked, interpolated tree (`read_raw`, `RawProject`); `locate.rs` finds the compose file; `project.rs` `read_project`, `ComposeProject`, `VolumeDecl`; `service.rs` + `service_steps.rs` + `service_env.rs` + `fields.rs` + `build.rs` + `depends.rs` + `health.rs` + `ports.rs` + `volumes.rs` + `mounts.rs` parse one service into `ServiceDef`; `definition.rs` typed results; `rules.rs` routed port, scale limits, replace order; `order.rs` start order from `depends_on`; `keys.rs` allowed key lists; `paths.rs` `check_bind_source`, `resolve_inside`; `environment.rs` + `dotenv_value.rs` + `interpolate.rs` + `scalar.rs` + `duration.rs` low-level value parsing; `test_support.rs` test fixtures.

Gotchas:
- The compose file is untrusted push input. Any key outside `compose/keys.rs` is a deploy error; YAML duplicate keys rejected; compose file and `.env` must resolve inside the app dir and are capped (1 MiB each, 4 MiB interpolated).
- `.env` next to the compose file follows docker compose syntax, not riku's own ENV file syntax.
- A published port may not name a host IP; the `riku.public` label alone decides loopback vs all interfaces (`compose/ports.rs`). The parser refuses labels riku sets itself.
- Bind sources are resolved lexically in `compose/paths.rs`; the symlink walk is repeated at start time in riku-deploy.
- Debug output of `ServiceContext`, `ServiceDef`, `BuildDef`, `RawProject` lists variable names, never values.
- `engine/bounded.rs` timeouts are why a wedged daemon cannot hang the caller; use `output_within`/`run_within`, not raw `Command`, for new engine calls.
- `compose/mod.rs` has `#![allow(clippy::question_mark)]` because of the no-`?` project rule.

## riku-args
Job: clap command tree and the rules for where each command runs, shared by `riku` and `rk`; no server deps. Depends on: none. Used by: riku-cli, riku-remote, riku-client, riku.
Entry: `Cli`, `Commands`, `require_app` (src/cli.rs); `placement_of`, `client_rule` (src/placement.rs); `parse_with_help` (src/parse.rs); `apply_app_hint` (src/app_hint.rs); `ServerName`, `SshAddress` (src/address.rs); `help::command`, `help::client_command`.

- `src/cli.rs`: `Cli` (global `-s/--server`, `--local`, hidden `--app-hint`) and the top-level `Commands` enum with examples in `after_help`. Key: `Cli`, `Commands`, `require_app` (errors `app name required`).
- `src/cmds.rs`: subcommand enums. Key: `AppsCmd`, `ConfigCmd`, `SettingsCmd` (server override of riku.toml), `StatsCmd`, `PluginCmd` (laptop client plugins), `PluginsCmd` (server bundles, with `TrustCmd`, `MarketplaceCmd`), `HookCmd`, `AddonCmd`, `ServerCmd`.
- `src/placement.rs`: `Placement` (Local, Server, Forwarded) by clap name via `placement_of`; `ClientRule` (RunsHere, Forwards, Refused) via `client_rule` for `rk`. Contract: this match is the one list of command names; a new top-level command must be added here or it is treated as forwarded.
- `src/app_hint.rs`: `apply_app_hint` fills an omitted app from the git remote hint; returns whether it did (then the hint must travel with a forwarded command). Contract: `destroy` and `ps` are deliberately not hinted.
- `src/address.rs`: `ServerName`, `SshAddress`, built only by `TryFrom<&str>`. Contract: trust boundary for the system `ssh` argv; user and host must start alphanumeric, exactly one `@`, bracketed IPv6 allowed. `destination()` strips the brackets for ssh, `as_str()` keeps them. TOML values go through the same check.
- `src/help.rs`: grouped `-h` output (Local, Server, Forwarded) built as a clap `help_template`. Key: `command`, `client_command`, `forward_heading`, `FORWARD_HEADING`. Contract: descriptions go through `escape_braces` because the template has no escape for `{`.
- `src/client_text.rs`: `for_client` rewrites `riku` examples to `rk` and the `dashboard` description for the client binary.
- `src/parse.rs`: `parse_with_help` parses once with no server and re-renders help with the implied server only when help was requested; `command_names`.

Gotchas:
- `help.rs` hardcodes the client program name `"rk"` because riku-remote depends on this crate.
- `cli.rs` takes the clap version from `CARGO_PKG_VERSION`, the one workspace version in the root Cargo.toml; the release workflow checks the tag matches it.
- `placement_of` lists the hidden internal commands (`__ns-shim`, `__dump-state`) as Server; adding a `Commands` variant also needs `dispatch::run_local` in riku-cli (exhaustive match).

## riku-cli
Job: command handlers for everything `riku` runs locally (apps, config, deploy, init/setup, doctor, git push plumbing, agent interface, plugins, addons). Depends on: riku-config, riku-util, riku-http, riku-args, riku-remote, riku-nginx, riku-caddy, riku-plugins, riku-supervisor, riku-deploy, riku-container, riku-route. Used by: riku (binary), riku-dashboard.
Entry: `dispatch::run_local` (src/dispatch/mod.rs, one match over `Commands`); `process_argv` (src/ssh_command.rs); `setup::cmd_init`, `InitOptions` (src/setup/init.rs); `doctor::collect`, `doctor::cmd_doctor`; `apps::*` re-exports (src/apps/mod.rs); `CliControlActions` (src/control_actions.rs); `router_helper::cmd_router_helper`.

- `src/lib.rs`: aliases dependency crates under old module names (`config`, `deploy`, `nginx`, `caddy`, `supervisor`, `util`) and re-exports riku-args modules and riku-remote's `client_plugins`, `quickstart`, `remote`, `routing`. `dashboard` is not here: riku-dashboard depends on this crate, so the binary dispatches it.
- `src/dispatch/`: `mod.rs` routes each `Commands` variant to a handler (also the `__ns-shim` exec path, reading `RIKU_NS_ROOT` and `RIKU_NS_CMD`); `app_cmds.rs` apps, config, logs, ps, stats, restart, addon; `plugins.rs` plugins, trust, marketplace, hook.
- `src/ssh_command.rs`: `process_argv` rebuilds argv from `SSH_ORIGINAL_COMMAND` only when riku runs with no args and the variable is nonempty; `split_original` tokenizes with shlex (no expansion) and drops a leading `riku`.
- `src/apps/`: one file per app command. `create.rs` (dir, bare repo, hook), `deploy.rs` (`--from` path or bare repo, then `deploy::do_deploy`), `destroy.rs` + `destroy_engine.rs` + `destroy_files.rs` (`DestroyMode` Safe/Force; engine step errors in Safe mode, app dir removed last so destroy is retryable; data and cache kept), `control.rs` (run, restart, stop, supervisor), `control_target.rs` (`control_target`: container app vs Procfile app, validates the service name), `hot_reload.rs` (bumps worker TOML mtime so the supervisor watcher reloads), `config.rs` (ENV show/get/set/unset/live; `set` refuses riku table keys and names the `riku settings` key), `settings_cmd.rs` (`riku settings show|get|set|unset` on `<env_root>/<app>/riku.toml`: validated app, file 0600, secrets masked in `show`, set and unset redeploy), `info.rs`, `list.rs`, `logs.rs` + `logs/` (tail, rotation, per service), `stats.rs`, `rollback.rs`, `install_plugins.rs` (`install_runtime`: writes an embedded plugin from `embedded_plugins.rs`, or downloads java/clojure pinned to the release tag and sha256 checked in `plugin_download.rs`; files written by `plugin_write.rs` through a hidden 0600 temp file and rename; any failure fails the command), `plugin_emit.rs` (`plugin.custom.*` events only, needs `RIKU_PLUGIN_NAME`).
- `src/apps/process/`: `ps` family. `ps_all.rs`, `ps_show.rs`, `ps_scale.rs`, `scaling_display.rs`, `helpers.rs` (stats.json and worker config lookups).
- `src/apps/dump_state.rs` + `dump_state/`: `__dump-state` and `state_json`, read-only JSON of ports, PIDs, locks, routes from on-disk files. Contract: env values (merged config) come only through `ROUTING_ENV_ALLOWLIST`; secrets are omitted, not masked; an unreadable config sets `config_error`. Also serves the dashboard `/api/state`.
- `src/setup/`: `riku init`. Plain user init (`init.rs`: dirs, git hook, default branch, user systemd unit, SSH key) vs root init (`root_init.rs`: installs `/usr/local/bin/riku`, re-runs `init --user-steps` as the deploy user, then host setup). `deploy_user.rs` `DeployUser` runs every action inside the deploy user's tree as that user (setsid, fixed argv). `binary.rs` root binary install. `deploy_keys.rs`/`ssh.rs` authorized_keys with forced command; `deploy_key_cmd.rs` `riku deploy-key add` (root only, `--user` account must have a real `~/.riku`, exactly one key, written by the deploy user through hidden `init --user-steps --keys-only`). `nginx_host.rs`, `nginx_worker.rs`, `caddy_host.rs`, `caddy_admin.rs`, `init_router.rs` router host setup; `router_choice.rs` picks the router at init (`RIKU_ROUTER`, else stored, else nginx on an existing tree when nginx is installed, else the one installed, asking on a terminal when both are); `helper_install.rs` router helper unit; `socket_group.rs`, `var_root.rs` `/run/riku-sockets` and `/var/lib/riku`; `host_cleanup.rs` removes deploy-writable files older versions left for root's nginx/Caddy; `system_service.rs` (root unit), `user_service.rs`, `router_units.rs`; `stale_binary.rs`, `default_branch.rs`, `deploy_account.rs`, `guidance.rs`, `prerequisites.rs`, `systemd.rs` (re-exports only). `server_ip.rs` `detect_server_ip` (UDP connect to TEST-NET-1, no packet sent), `public_v4` (private, shared, loopback, link-local refused), `ensure_routing_section` used by init and `doctor --fix`. Contract: root never creates, writes or chmods a path in the deploy user's home.
- `src/router_helper.rs`: `riku router-helper`, the root daemon that installs router config for the deploy user. Contract: refuses to start unless root; socket `ROUTER_SOCKET` group-restricted to deploy user's gid; logic lives in riku-route (`answer`, `bind_socket`, `run`).
- `src/doctor/`: `riku doctor`. `mod.rs` `collect` lists checks in display order (also served by dashboard `/api/doctor`), `Check`, `Status`; one file per check area. `--fix` runs `redeploy_sites.rs`, `migrate_settings.rs` (legacy `ROUTER`/`CONTAINER_ENGINE` into settings.toml; per app riku keys from ENV into the server override via `migrate_env.rs` under the app's deploy lock, only keys whose value renders back unchanged, and only when `migrate_check.rs` (staged copy in a 0700 scratch dir) shows the merged env unchanged, `ENV.bak` once, other ENV lines byte for byte; files written by `mode_write.rs` keep their mode) and `stale_paths.rs`. `app_env.rs` warns on riku keys left in ENV, `settings_check.rs` on legacy files and a missing `[routing]` section (user docs: docs/docs/routing.md and settings.md) (added by `--fix` through `migrate_settings.rs`), `package_tools.rs` while nub or uv is missing (also looks in `~/.local/bin`, `~/.cargo/bin`). Contract: checks are read-only; exit 1 on any Fail; every `--fix` step is skipped as root.
- `src/git/`: push path. `receive_pack.rs` (`cmd_git_receive_pack`, `cmd_git_upload_pack`: validates repo name from the ssh command, creates bare repo and hook lazily), `hook.rs` (`cmd_git_hook`: reads `old new ref` lines from stdin, clones on first push, `do_deploy` per ref), `repo.rs` (`post_receive_hook` text, `ensure_repo_symlink`), `push_status.rs` (carries a hook failure back to `git push` through a status file named in `STATUS_ENV`).
- `src/agent/`: `riku agent` JSON interface for AI agents. `execute.rs` order: state dir, rate limit, scope check, audit log opened before mutating commands, run, audit entry. `auth.rs` `AgentSession::from_flags` (id and scope from forced-command flags only), `check_rate_limit` (flock'd file), `types.rs` `AgentScope` (Readonly default, Staging, Production), `state.rs` (0700 dir, 0600 files, `O_NOFOLLOW`), `confirm.rs` (one-use stop/destroy tokens: 256 random bits, record in the state dir, checked as hex before use, bound to action and app, 5 minute expiry), `audit.rs` (JSON lines), `schema.rs`, `help.rs`, `commands/` per command.
- `src/plugins/mod.rs`: `riku plugins` output layer over riku-plugins (`PluginInstaller`, `MarketplaceService`, trust keys); `install_route.rs` sends a bare shipped plugin name (`BUNDLED_RUNTIMES`, riku-notify) to `install_runtime`, paths and git sources to the bundle installer, and refuses any other bare name. `src/addon/mod.rs`: `riku addon` over `AddonService`. `src/hooks.rs`: `riku hook list|check`.
- `src/backup.rs`: `riku backup|restore` over `deploy::backup::BackupService`. `src/scp.rs`: runs `scp` in `git_root`. `src/control_actions.rs`: `CliControlActions` implements the supervisor's `ControlActions` trait by calling the same `apps::cmd_*` functions.
- Tests: sibling `*_tests.rs` files via `#[path]`; cross-crate tests in tests/integration_tests/.

Gotchas:
- Some READMEs under src/ list stale symbols (`container.rs`, `ContainerCmd`, `DeployUser::create_dir`/`chmod` do not exist); trust the code.
- `cmd_hook_check` calls `std::process::exit`, and `cmd_doctor` and `cmd_scp` exit the process too; they do not return to the dispatcher.
- Two ways to install the supervisor unit: `setup/user_service.rs` (plain user) and `setup/system_service.rs` (root init); changes to the unit text usually need both.
- `RIKU_USER` (default `riku`) is read in `setup/deploy_user.rs` and again by `router_helper.rs`; `RIKU_DEFAULT_BRANCH` in `default_branch.rs`.
- `git/hook.rs` deploys only the repo's default branch (its `HEAD`); other branches, tags and deletions are skipped with a message. When `HEAD` names a branch that does not exist, the first pushed branch becomes `HEAD`.
- `control_actions.rs` and the dashboard both call `apps::cmd_*`; those functions print to the terminal.
- Agent `config:set` and `run` are not implemented and return `NOT_IMPLEMENTED`; agent scope table in `types.rs::allows` lists `config:unset` but no handler exists.
- Files here use `?` in older code; new code follows rule 16 with `#[allow(clippy::question_mark)]`.

## riku-dashboard
Job: embedded Axum server for `riku dashboard`: static UI plus JSON API and SSE log stream. Depends on: riku-config, riku-util, riku-supervisor, riku-cli, riku-deploy, riku-plugins. Used by: riku (binary).
Entry: `run(paths, bind, token)` (src/lib.rs); prints `riku-dashboard-bound <port>` (`BOUND_PREFIX` from riku-remote) on stdout for remote sessions.

- `src/lib.rs`: `run`, `DashboardState` (paths, optional token). Contract: non-loopback bind requires a token (`--token` or `RIKU_DASHBOARD_TOKEN`); single-threaded tokio runtime.
- `src/routes.rs`: `router`, read gate `authorize`, `/healthz`, `/api/state` (from `cli::apps::state_json`), `/api/apps/{app}/releases`. Contract: without a token only loopback `Host` headers pass; with a token, Bearer header or `?token=` is accepted (constant-time compare) and Host is not checked.
- `src/mutations.rs`: `authorize_mutation` (token required, Bearer header only), restart, stop, redeploy, scale, rollback, per-worker restart and delete; `finish`, `run_action` run the CLI functions on `spawn_blocking`. `service_mutations.rs`: container service restart, stop, scale.
- `src/appcfg.rs`: env GET and POST, both behind the mutation gate (Bearer token required: ENV holds secrets); POST goes through `config set/unset`. `addons.rs`: addon endpoints via `AddonService`. `market.rs`: marketplace sources, plugin install/remove, trust keyring. `installed.rs`: `/api/plugins` and plugin `ui_panel`. `system.rs`: `/api/doctor`, backup, restore upload (2 GiB limit on that route only; scratch file under `data_root`). `logs.rs`: SSE tail of `{log_root}/{app}/*.log`.
- `src/assets.rs`: `/` and `/assets/{*path}`, served from the fixed `ASSETS` table (`include_str!` of `assets/`). Contract: path is a table key, never a filesystem path; public, no data.
- `assets/`: UI with no build step. `index.html`, `app.css`, ES modules in `js/` (`main.js` entry, `poll.js` refresh loop, `api.js` fetch, `state.js`, `page.js`/`card.js`/`card-events.js` rendering, `render/` pure string builders, `logs/` drawer and stream, `actions.js`, `scale.js`, `releases.js`, `auth.js`); `js/tests/` node tests; `jsconfig.json` for tsc check.

Gotchas:
- A new JS or CSS file must be added to `ASSETS` in `src/assets.rs` or it is not served (a test enforces this).
- The UI sends the token as `?token=` on reads (EventSource cannot set headers) and keeps it in localStorage; the token can land in URLs and logs.
- `lib.rs` module doc still says "read-only" and "Mutating actions (later)"; mutations exist and are token-gated.
- The Next.js frontend in `dashboard/` proxies to this server; API changes affect both.

