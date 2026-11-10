# First Run Setup Implementation Plan (Project 2a)

> Implement with superpowers:subagent-driven-development or superpowers:executing-plans, one task at a time. Steps use checkboxes (`- [ ]`).

**Problem:**
- With no router installed, `riku init` warns and continues.
- Runtime plugins are downloaded from GitHub's `main` branch (shell plugins, the `riku-notify` bundle) or `releases/latest` (java, clojure) with no integrity check.
- A deploy missing a runtime plugin says "run riku install-plugins", and `riku plugins install node` does not install the node plugin.
- `rk server add` saves an entry but cannot put the laptop key on the server.
- The guides print `your-server` placeholders and require a Procfile.

**Goal:** `riku init` uses the router the developer installed and fails on a fresh install without one; bundled runtime plugins are built into riku and written out on demand, so nothing unverified is downloaded; a missing plugin error names the install command; `rk server add` installs the laptop key through the admin login and adds the git remote; the init guides print real values.

**Architecture:** The five shell runtime plugins and the `riku-notify` bundle are embedded with `include_bytes!` and written to `~/.riku/plugins` when asked. java and clojure stay downloads, pinned to the release tag of this riku's version and checked against the `.sha256` published with them. Router choice is a pure function of what is installed, the operator's request, any stored choice and whether `~/.riku` existed before init started. Deploy key installation goes through the existing relay that runs riku as the deploy user.

**Tech Stack:** Rust workspace; `std::io::IsTerminal`; `sha2 = "0.11"` (already used by riku-deploy and riku-plugins) for the java and clojure check.

**Spec:** `docs/superpowers/specs/2026-11-09-developer-experience-design.md`, Project 2 sections "Router detection", "Runtime plugins on demand", "Laptop setup", "Guidance text". Project 2b (URL on first push) has its own plan.

**Terms:** `rk` is the laptop client (crate riku-client); it forwards server commands over ssh. The deploy user owns `~/.riku` and receives pushes; its ssh keys carry riku's forced command. The admin login is a separate account on the server that can run `sudo`, stored per server by `rk server add --admin`. `riku init` run as root creates the deploy user, then re-runs itself as that user (the user steps) through `DeployUser` (`crates/riku-cli/src/setup/deploy_user.rs`). riku.toml is the per app file in the repo; settings.toml is the server file under `~/.riku`.

## Owner decisions (2026-11-10)

- Bundled shell runtime plugins are embedded in the binary and written on demand. java and clojure downloads are pinned to the release tag and verified.
- `rk server add` installs the key through the admin login; without one it prints the command to run on the server.
- A bare bundled runtime name given to `riku plugins install` installs that runtime.

## Decisions made during planning

- One workspace version (`[workspace.package] version = "0.1.2"`). Today 18 crates say 0.1.0 while the release is 0.1.2, so the agent schema, health and router helper report the wrong version, and Task 2 needs the right one to pick a release tag.
- The release workflow fails unless the tag equals `v` plus the workspace version.
- `riku install-plugins` stays and goes through the same install function as `riku plugins install <name>`; one code path, two spellings.
- The key reaches the server as one shell quoted argument, never on stdin, so `sudo` can still ask for a password.

## Out of scope versus spec

- `rk deploy` does not check or offer missing plugins (spec "Yes installs then pushes"). A deploy or push already fails with the exact install command (Task 5), and a laptop side check would guess wrong for container apps and server overrides. `rk deploy` keeps its meaning: redeploy on the server.
- Key copy needs an admin login (spec says `rk server add` copies the key). Only root can add a deploy key.
- Project 2b.

## Alternatives considered

- Pin shell plugin downloads to a release and check a compiled hash: rejected by the owner after review. v0.1.2 has no shell plugin assets, every dev build after a plugin edit would fail the check, and offline servers could not install.
- A laptop side plugin check in `rk deploy`: dropped, see above.

## Risks

- java and clojure still need GitHub: a server without network or a release without assets gets a clear error with the manual step.
- `riku deploy-key add` runs as root and grants push access; it accepts exactly one key line, keeps only the key type and blob, and writes through the deploy user.
- A server that reruns `sudo riku init` must keep its router; the fresh check must run before init creates anything.

## Global constraints

- `.claude/CLAUDE.md`: no `?` (explicit match), tests in sibling `_tests.rs`, files under about 200 lines, doc comments on public items, security model in module docs, no emoji, no em dashes, no hyphenated compounds in prose.
- Read `docs/CODEMAP.md` first; update it and the ROADMAP status in the same commit as the work.
- Native file tools only; no python or perl. Commits: subject under 50 characters, `git commit -F <file> --no-verify`, no AI attribution.
- Confidential names never written; use `prod`, `edge.example`, `myapp`.
- Per task: `cargo check --workspace` and `cargo test -p <touched crates>`. Full suite, clippy and Docker e2e once at the end.
- Tests never change the process working directory, environment or depend on running as root or not.

## Review focus

1. `sudo riku init` rerun on an existing server installed without `RIKU_ROUTER`, with both nginx and Caddy present: keeps nginx, does not fail. Task 4.
2. Fresh install with both routers and no terminal (init piped over ssh): fails naming `RIKU_ROUTER`. Task 4.
3. A crash between writing and renaming a plugin leaves nothing that `runtime::discover` would run. Task 2.
4. A key whose comment holds `;` or `$(...)`, or with `command=` options: reaches the server as one argument and lands as type plus blob only. Task 6.
5. `riku plugins install node` with a `./node` directory in the deploy user's home: installs the runtime. Task 3.
6. A java download whose bytes do not match its `.sha256`: hard error naming the plugin, exit non zero, nothing written. Task 2.

---

## Tasks

### Task 1: One workspace version

**Files:** `Cargo.toml` (`[workspace.package] version = "0.1.2"`), every `crates/*/Cargo.toml` (`version.workspace = true`), `crates/riku-args/src/cli.rs:13` (`version = env!("CARGO_PKG_VERSION")`), `crates/riku-cli/src/apps/dump_state/builder.rs:175` (`env!("CARGO_PKG_VERSION")`), `.github/workflows/release.yml` (a new `verify-version` job: checkout, read `version` under `[workspace.package]` in `Cargo.toml`, fail unless `GITHUB_REF_NAME` equals `v` plus that version; `build` gets `needs: verify-version`. The `create-release` job has no checkout, so the check cannot live there).

- [ ] Test `cli_version_is_the_crate_version` in `crates/riku-args/src/cli_tests.rs` (create if missing, with a `#[path]` module in cli.rs): `Cli::command().get_version() == Some(env!("CARGO_PKG_VERSION"))`.
- [ ] Run it and confirm it fails. Implement. `cargo check --workspace`; `cargo test -p riku-args`.
- [ ] Commit `Use one version for the workspace` (body names the 0.1.0 reports it fixes).

### Task 2: Embedded runtime plugins, verified downloads

**Files:** Create `crates/riku-cli/src/apps/embedded_plugins.rs` (the table), `crates/riku-cli/src/apps/plugin_write.rs` and `plugin_write_tests.rs` (safe write), modify `crates/riku-cli/src/apps/install_plugins.rs:16-203`, `crates/riku-plugins/src/runtime.rs:62-110` (`discover` skips names starting with `.`), `crates/riku-cli/Cargo.toml` (`sha2 = "0.11"`).

**Interfaces:**
- `pub(crate) const EMBEDDED: &[EmbeddedPlugin]` with `struct EmbeddedPlugin { name: &'static str, files: &'static [(&'static str, &'static [u8])] }`: the five runtimes, one file each (`include_bytes!("../../../../plugins/node")` and so on), and `riku-notify` with `riku-plugin.toml` and `bin/on-event`. The canonical runtime list is `riku_util::runtime_markers::BUNDLED_RUNTIMES: &[&str] = &["go", "node", "python", "ruby", "rust-lang", "java", "clojure"]` (created here, extended in Task 5), in riku-util because riku-deploy needs it and cannot depend on riku-cli; a test checks that `EMBEDDED` runtimes plus `BINARY_PLUGINS` equal it. `SHELL_PLUGINS` and the download code for shell plugins go away. With no `--plugins`, `riku install-plugins` and the control route install the five embedded runtimes, as today; java, clojure and riku-notify only when named.
- `pub(crate) fn write_plugin_file(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> Result<()>`: writes `dir/.<name>.new` with mode 0600, fsyncs, sets `mode`, renames to `dir/<name>`. On any error the `.new` file is removed.
- `pub(crate) fn install_runtime(paths: &RikuPaths, name: &str) -> Result<()>`: embedded names are written out (runtimes 0755 into `plugin_root`; for `riku-notify`, the caller runs `create_dir_all` on `plugin_root/riku-notify/bin`, then writes `riku-plugin.toml` 0644 into `plugin_root/riku-notify` and `on-event` 0755 into its `bin`; `write_plugin_file` takes a plain file name, never a path); `java` and `clojure` download from `https://github.com/dreygur/riku/releases/download/v{CARGO_PKG_VERSION}/riku-plugin-{name}-{triple}` and compare sha256 with the first field of `.../riku-plugin-{name}-{triple}.sha256` before `write_plugin_file`.
- Download errors are typed: `enum FetchError { NotFound(String), Status(u16, String), Network(String) }`. Messages: not found `no release v{version} of plugin '{name}' for {triple}`; mismatch `checksum mismatch for plugin '{name}' (release v{version}); not installed`.
- `cmd_install_plugins` (CLI `riku install-plugins` and the supervisor `/control/plugins/install` route, `crates/riku-cli/src/control_actions.rs:34`) returns an error naming every plugin that failed, and fails when any failed. A checksum mismatch never reads as a network problem.
- Fix `host_target_triple` (`install_plugins.rs:178-183`): it prints the OS as the plugin name; take `name` as a parameter.

- [ ] Tests: `write_plugin_file_never_leaves_an_executable_temp_file` (fail after the write: no `.node.new`; a leftover `.x.new` is not returned by `runtime::discover`, Review Focus 3); `embedded_table_holds_the_five_runtimes_and_riku_notify`; `install_runtime_writes_node_without_network` (temp plugin root); `checksum_mismatch_is_a_hard_error` (bytes and a wrong hash through the verify function, Review Focus 6); `binary_url_is_pinned_to_the_version` (`v` plus `CARGO_PKG_VERSION`, never `latest`); `host_target_triple_names_the_plugin`.
- [ ] Run and confirm they fail. Implement. `cargo test -p riku-cli -p riku-plugins`.
- [ ] Commit `Embed runtime plugins and verify downloads`.

### Task 3: `plugins install` accepts a runtime name

**Files:** `crates/riku-cli/src/plugins/mod.rs:33-43`, create `crates/riku-cli/src/plugins/install_route.rs` and `install_route_tests.rs` (mod.rs is already 258 lines).

**Interfaces:** `fn install_target(source: &str) -> Result<InstallTarget>`, `enum InstallTarget { Runtime(&'static str), Bundle(String) }`:
1. A source containing `/` or `:`, or ending in `.git`, is a `Bundle` (paths `./x`, `../x`, `/x`, `plugins/x`, `github:o/r`, `git@...`, `https://...`).
2. A bare source (none of the above) in `BUNDLED_RUNTIMES` is `Runtime`.
3. Any other bare source is an error listing the bundled runtimes. A bare relative bundle dir such as `mybundle` is no longer accepted; write `./mybundle`.

`Runtime` calls `install_runtime` from Task 2. `riku install-plugins` keeps working and uses the same function.

- [ ] Tests: `bare_runtime_name_is_a_runtime` (with a `node` dir present in a temp dir passed in, never the process cwd; Review Focus 5); `paths_and_git_sources_are_bundles` (each form in rule 1); `unknown_bare_name_is_refused`.
- [ ] Run, confirm failure, implement, `cargo test -p riku-cli install_route`.
- [ ] Commit `Install runtimes by name in plugins install`.

### Task 4: Choose the router at init

**Files:** create `crates/riku-cli/src/setup/router_choice.rs` and tests; modify `crates/riku-cli/src/setup/root_init.rs:54-104`, `crates/riku-cli/src/setup/init.rs` (plain, non root init), `crates/riku-cli/src/setup/init_router.rs:31-49` (`persist_router` only writes), `crates/riku-cli/src/setup/prerequisites.rs:39-46` (drop the router warning), `crates/riku-config/src/program.rs` (add `pub fn installed_program(name: &str) -> Option<PathBuf>`; add `/snap/bin` to `FALLBACK_DIRS`), `crates/riku-config/src/router.rs` (add `pub fn stored(paths: &RikuPaths) -> Result<Option<Router>>`: settings `[router] name`, else the legacy file, else `None`).

**Interfaces:** `pub(super) fn choose_router(installed: &[Router], requested: Option<Router>, stored: Option<Router>, fresh: bool, interactive: bool) -> RouterChoice`, `enum RouterChoice { Use(Router), Ask(Vec<Router>), Fail(String) }`. In order:
1. `requested` (`RIKU_ROUTER`, any name including a router plugin) wins.
2. `stored` wins.
3. Not `fresh` (the riku tree existed before init started): nginx, the old default. Init then stores it.
4. One installed router: use it.
5. Two installed: `Ask` when interactive, else `Fail("nginx and Caddy are both installed; run init again with RIKU_ROUTER=nginx or RIKU_ROUTER=caddy")`.
6. None: `Fail("no router found: install nginx (apt install nginx) or Caddy, then run riku init again")`.

`fresh` is taken before anything is created. In root init, before `ensure_account` (root_init.rs:81): `passwd_home(&deploy_user_name())` is `None` (no account yet) or `symlink_metadata(home.join(".riku"))` is not found. In plain init, at its start: `symlink_metadata(&paths.riku_root)` is not found.

`stored` in root init: `None` when `fresh`; otherwise `deploy_router(&user)` (init_router.rs:70), which reads as the deploy user and already returns "stored, else nginx" (rules 2 and 3), so root never opens files in the deploy user's tree. `riku_config::router::stored()` is used by plain init only.

Root resolves the choice, then sets `RIKU_ROUTER` with `std::env::set_var` before `run_user_steps`, which already passes `RIKU_ROUTER` to the user steps (`deploy_user.rs:30`). `persist_router` becomes `persist_router(paths: &RikuPaths, router: &Router) -> Result<()>` (write only); the user steps call it with `Router::from(RIKU_ROUTER)` when set, and plain init calls it with its own choice. The prompt reads one line only when `stdin().is_terminal()`: `Router to use [nginx/caddy]:`; an answer other than those two names fails with the rule 5 message.

- [ ] Tests (pure): `requested_router_wins`; `stored_router_wins`; `existing_tree_keeps_nginx_without_a_stored_choice` (Review Focus 1); `single_installed_router_is_used`; `two_routers_ask_when_interactive`; `two_routers_without_a_terminal_name_riku_router` (Review Focus 2); `no_router_on_a_fresh_install_fails_with_the_install_command`; in riku-config, `stored_is_none_without_settings_or_file`.
- [ ] Run, confirm failure, implement, `cargo test -p riku-cli -p riku-config`.
- [ ] Commit `Choose the router the developer installed`.

### Task 5: Name the missing runtime plugin

**Files:** extend `crates/riku-util/src/runtime_markers.rs` (created in Task 2) and its tests; modify `crates/riku-plugins/src/runtime.rs:119-136` (`detect`), `crates/riku-deploy/src/procfile_deploy.rs:23-37` (`detect_runtime`).

**Interfaces:**
- `BUNDLED_RUNTIMES` from Task 2 lives in this file.
- `pub fn needed_runtime(app_path: &Path) -> Option<&'static str>`, markers checked in this order, first match wins: `go.mod` or any `*.go` file at the top level go; `package.json` node; `requirements.txt`, `pyproject.toml` or `setup.py` python; `Gemfile` ruby; `Cargo.toml` rust-lang. (Alphabetical, like server detection.)
- `riku_plugins::runtime::MissingRuntime { name: String }`, an error type returned by `detect` when `RIKU_RUNTIME` names a plugin that is not installed (today a plain message, runtime.rs:130).
- `detect_runtime` formats: when `RIKU_RUNTIME` names a missing bundled runtime, or no plugin matched and `needed_runtime` names a bundled runtime that is not installed: `app '{app}' needs the {name} plugin. Install it with: rk plugins install {name} (on the server: riku plugins install {name}), then push again.` A missing name that is not bundled keeps the current message. Container apps never reach this code.

- [ ] Tests: in riku-util `markers_pick_the_runtime` (each marker), `no_marker_gives_none`; in riku-plugins `missing_riku_runtime_is_a_typed_error`; in riku-deploy `missing_bundled_plugin_names_the_install_command`, `missing_custom_plugin_keeps_the_old_message`.
- [ ] Run, confirm failure, implement, `cargo test -p riku-util -p riku-plugins -p riku-deploy`.
- [ ] Commit `Name the missing runtime plugin on push`.

### Task 6: `rk server add` installs the key and the remote

**Files:** create `crates/riku-cli/src/setup/deploy_key_cmd.rs` and tests; modify `crates/riku-args/src/cli.rs` and `cmds.rs` (`Commands::DeployKey` with `add <key>`; `--key PATH` on `ServerCmd::Add`), `crates/riku-args/src/placement.rs` (`deploy-key` is Server), `crates/riku-cli/src/dispatch/mod.rs`, `crates/riku-args/src/cli.rs:248-253` and `crates/riku-cli/src/setup/init.rs:123-197` (a hidden `--keys-only` flag on `init`, an `InitOptions.keys_only` field and its dispatch mapping: with `--user-steps --keys-only`, init only runs `parse_key_args` and `add_deploy_keys`, never `persist_router` or `prepare_tree`), `crates/riku-remote/src/remote/git_remote.rs:84-94` (add `read_remotes_in(dir: &Path)` running `git -C <dir> remote -v`, reusing `parse_remotes`), `crates/riku-remote/src/remote/server_cmd.rs:74-86` (`run_at` gains `repo_dir: &Path`), create `crates/riku-remote/src/remote/key_install.rs` and tests.

**Interfaces:**
- Server: `riku deploy-key add '<key line>'`. `pub(crate) fn add_deploy_key(is_root: bool, key_line: &str, user: &DeployUser) -> Result<()>`: refuses unless `is_root`; parses with `parse_public_keys(key_line)` (public_key.rs:96) and requires exactly one key (a second line is a second key and is refused; text after the blob on the same line is a comment and is dropped; options before the type are dropped); runs riku as the deploy user through `DeployUser::run_relayed` with the hidden `init --user-steps --keys-only --add-key <key>` so the deploy user writes its own `authorized_keys` with `add_deploy_keys` (duplicates skipped). The command passes `nix::unistd::Uid::effective().is_root()`.
- Laptop: `rk server add NAME user@host [--admin admin@host] [--key PATH]` saves the entry as today, then:
  - key: `--key` or `~/.ssh/id_ed25519.pub`, else `~/.ssh/id_rsa.pub`; validated with `PublicKey::try_from` before sending.
  - with `--admin`: runs `ssh -t <admin> <cmd>` where `<cmd>` is `shlex::try_join(["sudo", "riku", "deploy-key", "add", <key line>])`; stdin stays free for the sudo password. On failure the entry stays and the manual command is printed.
  - without an admin: prints `Add the key on the server: sudo riku deploy-key add '<key line>'` and `or rerun with --admin <your login>@<host>`, with the host filled in.
  - `fn add_git_remote(repo_dir: &Path, server: &Server) -> Result<RemoteOutcome>`: when `repo_dir` is a git repo with no remote pointing at the server (`git_remote::read_remotes_in(repo_dir)`) and no remote named `riku`, runs `git -C <repo_dir> remote add riku <user@host>:<app>`, where `app` is the repo root's directory name validated as an app name. An invalid name or an existing remote is reported and skipped.

- [ ] Tests: `deploy_key_add_refuses_non_root` (`is_root = false`); `deploy_key_add_refuses_two_keys`; `key_with_options_lands_as_type_and_blob`; `remote_command_quotes_the_key` (a comment with `;` and `$(id)` stays one argument after `shlex::split`, Review Focus 4); `server_add_without_admin_prints_the_server_command`; `remote_added_in_a_repo` and `existing_riku_remote_is_kept` (temp repos passed as `repo_dir`).
- [ ] Run, confirm failure, implement, `cargo test -p riku-cli -p riku-remote -p riku-args -p riku-client` (the rk dependency guard must pass).
- [ ] Commit `Install the deploy key from rk server add`.

### Task 7: Guidance text and docs

**Files:** `crates/riku-cli/src/setup/guidance.rs:84,94-95` and `guidance_tests.rs:11-12`, `crates/riku-cli/src/apps/create.rs:90`, `crates/riku-cli/src/apps/list.rs:15,29`, `crates/riku-dashboard/assets/js/render/panels.js:44`, `crates/riku-remote/src/quickstart/mod.rs:100-122`, `scripts/install.sh:131-138`, `docs/docs/plugins.md`, `docs/docs/installation.md`, `docs/docs/quick-start.md`, `docs/docs/cli.md`, `docs/CODEMAP.md`, `docs/superpowers/ROADMAP.md`.

- Guidance says processes come from riku.toml or a Procfile, or the runtime's start command; no Procfile requirement.
- No `your-server` or `your-app` placeholder: where riku cannot know the host, it prints `rk server add <name> <user>@<host> --admin <login>@<host>` as the next step instead of a fake remote line. The quickstart says `rk server add` when no server is registered.
- install.sh: the server branch ends at `riku init`; the laptop branch shows `rk server add myserver riku@edge.example --admin you@edge.example`.
- Docs: `riku plugins install <name>`, embedded runtimes, the java and clojure check, `riku deploy-key add`, router detection.

- [ ] Test `guidance_has_no_placeholder_or_procfile_requirement` (replaces the assertion at `guidance_tests.rs:11-12`).
- [ ] Run, confirm failure, implement, `cargo test -p riku-cli guidance`.
- [ ] Commit `Fix first run guidance and docs`.

### Final gate

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `./scripts/run-docker-tests.sh`.
- [ ] Search the branch diff for the confidential names before any push.
