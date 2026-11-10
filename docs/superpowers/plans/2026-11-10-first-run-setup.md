# First Run Setup Implementation Plan (Project 2a)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Problem:** A new server today installs with a warning when no router exists, runtime plugins come from GitHub's `main` branch with no integrity check, a deploy that needs a missing runtime says "run riku install-plugins", `rk server add` cannot reach the server's deploy account, and the guides print `your-server` placeholders and require a Procfile.

**Goal:** Project 2a of the spec: init picks the router the developer brought, runtime plugins install on demand from a pinned release with checksums compiled into riku, a missing plugin is named with the exact command, `rk server add` can install the laptop key through an admin login and add the git remote, and the guides stop lying.

**Architecture:** Router choice is a pure function over what `find_program` sees, called by init before anything is written. Runtime installs download the asset for this riku's version tag and compare it with a sha256 table generated at build time from `plugins/`. A marker table in riku-plugins maps repo files to the bundled runtime they need; the deploy and `rk deploy` both use it. Key install is a new root only server command fed by `rk server add --admin`.

**Tech Stack:** Rust workspace; `sha2` (already in Cargo.lock through a dependency) as a direct dependency of riku-cli and its build script; `std::io::IsTerminal` for prompts.

**Spec:** `docs/superpowers/specs/2026-11-09-developer-experience-design.md`, Project 2 sections "Router detection", "Runtime plugins on demand", "Laptop setup", "Guidance text". Project 2b (URL on first push: domain, port, path modes, fallback domain and TLS, port rules) is a separate plan.

## Owner decisions for this plan (2026-11-10)

- Plugin trust: download from the release tag of the running riku version and verify a sha256; refuse on mismatch.
- Key copy: `rk server add` uses `--admin user@host` to install the key; without it, it prints the exact command.
- A bare bundled runtime name given to `riku plugins install` installs that runtime; a local bundle path must start with `./` or `/`.

## Calls made by Claude

- Shell plugin hashes are compiled into riku by a build script, so a changed release asset cannot pass. java and clojure binaries are checked against the `.sha256` file published with the same tag, which only catches corruption; said in the docs.
- One version for the workspace (`[workspace.package] version = "0.1.2"`, the current release), so riku knows which tag to fetch.
- `rk deploy` keeps its meaning (redeploy on the server). It gains the plugin check and offer before it forwards. It does not push code.
- The server lists installed runtimes with a new forwarded command, `riku plugins runtimes`, one name per line.
- The key command is `riku deploy-key add`, root only, one public key on stdin.

## Non goals

- Anything in Project 2b.
- Signing plugins or rotating trust keys; the release pipeline stays as it is apart from publishing shell plugins.
- Installing routers for the developer.

## Global Constraints

- `.claude/CLAUDE.md` rules: no `?` (explicit match), tests in sibling `_tests.rs`, files under about 200 lines, doc comments on public items, no emoji, no em dashes, security model in module docs.
- Read `docs/CODEMAP.md` first; update it in the same commit as any module change. ROADMAP status in the same commit as the work.
- Native file tools only; no python or perl. Commits: imperative subject under 50 characters, `git commit -F <file> --no-verify`, no AI attribution.
- Confidential names never written; use `prod`, `edge.example`, `myapp`.
- Per task: `cargo check --workspace` and `cargo test -p <touched crates>`; full suite, clippy, Docker e2e once at the end.
- An existing server that re-runs nothing keeps working: router detection fails hard only on a fresh install (no `[router] name` in settings.toml and no legacy `ROUTER` file).

## Review Focus

1. Offline server, or a riku build whose version has no release: install fails with the manual command, writes nothing. Task 2.
2. A downloaded plugin whose bytes do not match the compiled hash: refused, nothing written, error names the plugin. Task 2.
3. Both nginx and Caddy installed and `sudo riku init` run without a terminal (piped over ssh): fails naming `RIKU_ROUTER=nginx|caddy`. Task 4.
4. `rk deploy` with stdin not a terminal and a plugin missing: treated as no, prints `rk plugins install <name>`. Task 6.
5. `riku plugins install node` while a `./node` directory exists in the deploy user's home: installs the node runtime, not the directory. Task 3.

---

### Task 1: One workspace version

**Files:** `Cargo.toml` (add `[workspace.package] version = "0.1.2"`), every `crates/*/Cargo.toml` (`version.workspace = true`), `crates/riku-args/src/cli.rs:13` (clap `version` from `env!("CARGO_PKG_VERSION")`).

- [ ] Test `version_matches_the_workspace` in `crates/riku-args/src/cli_tests.rs` (or the existing args test file): `Cli::command().get_version() == Some(env!("CARGO_PKG_VERSION"))` and that value is `"0.1.2"`.
- [ ] Run it, it fails; implement; `cargo check --workspace`; `cargo test -p riku-args`.
- [ ] Commit `Use one version for the workspace`.

### Task 2: Pinned, verified runtime plugin install

**Files:** Create `crates/riku-cli/build.rs` (sha256 of `../../plugins/{node,python,ruby,go,rust-lang}` into `OUT_DIR/plugin_hashes.rs` as `&[(&str, &str)]`), `crates/riku-cli/src/apps/plugin_fetch.rs` (URL, download, verify, atomic write), `plugin_fetch_tests.rs`. Modify `crates/riku-cli/src/apps/install_plugins.rs:35-140` to use it, `crates/riku-cli/Cargo.toml` (`sha2` dep and build-dep), `.github/workflows/release.yml:135-163` (also publish `plugins/<name>` as asset `riku-plugin-<name>` for the five shell plugins).

**Interfaces:**
- Produces: `pub(crate) fn asset_url(name: &str, version: &str, target: Option<&str>) -> String` = `https://github.com/dreygur/riku/releases/download/v{version}/riku-plugin-{name}` (plus `-{target}` for java, clojure); `pub(crate) fn verify(name: &str, bytes: &[u8], expected: &str) -> Result<()>`; `pub(crate) fn install_runtime(paths: &RikuPaths, name: &str) -> Result<()>` (used by Tasks 3 and 6 through `cmd_install_plugins`).
- Shell plugins verify against the compiled table; binary plugins against the `.sha256` asset of the same tag.
- Write to `<plugin_root>/.<name>.new` with mode 0755, then rename. A failed download or mismatch leaves the plugin root unchanged.
- Errors: mismatch `checksum mismatch for plugin '<name>' (release v<version>); refusing to install`; HTTP 404 `no release v<version> of plugin '<name>'; copy plugins/<name> from the riku source to <plugin_root>/<name>`; network failure names the URL.

- [ ] Tests: `asset_url_is_pinned_to_the_version` (`v0.1.2` in the URL, never `main` or `latest`); `verify_refuses_changed_bytes`; `verify_accepts_matching_bytes`; `build_table_covers_the_five_shell_plugins`; `failed_install_leaves_plugin_root_unchanged` (verify failure, no file and no `.new` left).
- [ ] Run, fail, implement, `cargo test -p riku-cli plugin_fetch`.
- [ ] Commit `Install runtime plugins pinned and verified`.

### Task 3: plugins install with a runtime name

**Files:** `crates/riku-cli/src/plugins/mod.rs:33-43` (`cmd_plugins_install`), test in its tests module or a new `plugins/install_route_tests.rs`.

**Interfaces:** Produces `fn install_target(source: &str) -> InstallTarget` with `enum InstallTarget { Runtime(&'static str), Bundle(String) }`: a name in `SHELL_PLUGINS` or `BINARY_PLUGINS` is `Runtime`; anything else goes to the bundle installer unchanged (git URLs, `./path`, `/path`). A bare name that is neither is an error naming the known runtimes.

- [ ] Tests: `bare_runtime_name_installs_the_runtime` (also with a `./node` dir present in cwd: still `Runtime`, Review Focus 5); `dot_slash_path_is_a_bundle`; `git_url_is_a_bundle`; `unknown_bare_name_is_refused`.
- [ ] Run, fail, implement, test. Commit `Route runtime names in plugins install`.

### Task 4: Router detection at init

**Files:** Create `crates/riku-cli/src/setup/router_choice.rs` + tests; modify `crates/riku-cli/src/setup/init_router.rs:31-49` (`persist_router`), `crates/riku-cli/src/setup/root_init.rs:54-104` (choose before spawning the user step, pass the answer through `RIKU_ROUTER`), `crates/riku-cli/src/setup/prerequisites.rs:39-46` (drop the router warning; detection owns it).

**Interfaces:** `pub(super) fn choose_router(installed: &[Router], requested: Option<Router>, current: Option<Router>, interactive: bool) -> RouterChoice` with `enum RouterChoice { Use(Router), Ask(Vec<Router>), Fail(String) }`. Order: `RIKU_ROUTER` wins; else an existing choice (settings or legacy file) is kept; else exactly one installed router is used; two installed and interactive asks; two and not interactive fails naming `RIKU_ROUTER=nginx` or `RIKU_ROUTER=caddy`; none fails with `no router found: install nginx (apt install nginx) or Caddy, then run riku init again`. `installed_routers() -> Vec<Router>` uses `find_program(..).is_file()` for `nginx` and `caddy`. The prompt reads one line from stdin only when `std::io::stdin().is_terminal()`.

- [ ] Tests (pure, no host state): `requested_router_wins`; `existing_choice_is_kept_without_a_router_binary` (an existing server is never failed); `single_installed_router_is_used`; `two_routers_ask_when_interactive`; `two_routers_without_a_terminal_name_riku_router` (Review Focus 3); `no_router_on_a_fresh_install_fails_with_the_install_command`.
- [ ] Run, fail, implement, `cargo test -p riku-cli router_choice`.
- [ ] Commit `Choose the router the developer installed`.

### Task 5: Name the missing runtime plugin on push

**Files:** Create `crates/riku-util/src/runtime_markers.rs` + tests (`pub fn needed_runtime(app_path: &Path) -> Option<&'static str>`: `package.json` node, `requirements.txt` or `pyproject.toml` python, `Gemfile` ruby, `go.mod` go, `Cargo.toml` rust-lang, first match in that order; in riku-util because `rk` uses it too and may not link riku-plugins, see `crates/riku-client/tests/deps.rs`); modify `crates/riku-deploy/src/procfile_deploy.rs:23-37` (`detect_runtime`) and `crates/riku-plugins/src/runtime.rs` (the `RIKU_RUNTIME` names a missing plugin error).

**Interfaces:** When no plugin detects the app and a marker names a runtime that is not installed: `app '<app>' needs the <name> plugin. Install it: rk plugins install <name>, then push again.` When `RIKU_RUNTIME` names a plugin that is not installed: the same message for that name. When a marker's plugin is installed but did not detect, the existing message stays.

- [ ] Tests: `marker_picks_the_bundled_runtime` (each of the five); `no_marker_gives_none`; in riku-deploy, `missing_runtime_plugin_names_the_install_command`; `riku_runtime_naming_a_missing_plugin_names_it`.
- [ ] Run, fail, implement, test riku-plugins and riku-deploy. Commit `Name the missing runtime plugin on push`.

### Task 6: rk deploy offers the missing plugin

**Files:** Create `crates/riku-cli/src/apps/runtimes_cmd.rs` (server command `riku plugins runtimes`: installed runtime plugin names, one per line, from `runtime::discover`); modify `crates/riku-args/src/cmds.rs` (`PluginsCmd::Runtimes`), `crates/riku-cli/src/dispatch/plugins.rs`; create `crates/riku-remote/src/remote/plugin_check.rs` + tests; modify `crates/riku-remote/src/remote/flow.rs` (before forwarding `Deploy`, run the check).

**Interfaces:** `plugin_check::missing_runtime(repo_dir: &Path, server_runtimes: &[String]) -> Option<String>` uses the riku.toml `[runtime] name` in `repo_dir` when present (read as plain TOML: rk may not link riku-config), else `riku_util::runtime_markers::needed_runtime` (Task 5). The prompt: `<name> plugin needed on <server>, install? [y/N]`; yes forwards `plugins install <name>` then the deploy; no, or stdin not a terminal, stops with `rk plugins install <name>` printed.

- [ ] Tests: `missing_runtime_from_markers`; `riku_toml_runtime_name_wins_over_markers`; `installed_runtime_is_not_missing`; `non_terminal_stdin_answers_no` (Review Focus 4); `runtimes_lists_installed_runtime_names_only` (server side, no `riku-*` hooks).
- [ ] Run, fail, implement, `cargo test -p riku-remote -p riku-cli -p riku-client` (the deps guard must still pass).
- [ ] Commit `Offer the missing plugin on rk deploy`.

### Task 7: rk server add installs the key and the remote

**Files:** Create `crates/riku-cli/src/setup/deploy_key_cmd.rs` (server: `riku deploy-key add`, refuses unless root, reads stdin, `parse_public_keys`, exactly one key, adds it through `DeployUser` like `add_deploy_keys` in `setup/deploy_keys.rs:86`), args and dispatch (`Commands::DeployKey`, placement Server); modify `crates/riku-remote/src/remote/server_cmd.rs:76-86` (`add`).

**Interfaces:** `rk server add NAME user@host [--admin admin@host] [--key PATH]` saves the entry as today, then:
- with `--admin`: runs `ssh -t <admin> sudo riku deploy-key add` with the public key (default `~/.ssh/id_ed25519.pub`, else `id_rsa.pub`) on stdin; failure is reported with the manual command, the saved entry stays.
- without `--admin`: prints `ssh <admin login> sudo riku deploy-key add < <key>`.
- inside a git repo with no remote pointing at that server: `git remote add riku <user@host>:<dir name>` after validating the dir name as an app name; an existing `riku` remote is left alone and named in the output.

- [ ] Tests: `deploy_key_add_refuses_non_root` (as the test user); `deploy_key_add_refuses_two_keys`; `server_add_prints_the_key_command_without_admin`; `server_add_adds_the_git_remote_in_a_repo` (temp git repo); `server_add_keeps_an_existing_riku_remote`.
- [ ] Run, fail, implement, `cargo test -p riku-cli -p riku-remote -p riku-client`.
- [ ] Commit `Install the deploy key from rk server add`.

### Task 8: Guidance text, docs, map

**Files:** `crates/riku-cli/src/setup/guidance.rs:84,94-95`, `crates/riku-remote/src/quickstart/mod.rs:100-122`, `scripts/install.sh:131-138`, `docs/docs/plugins.md`, `docs/docs/installation.md`, `docs/docs/quick-start.md`, `docs/CODEMAP.md`, `docs/superpowers/ROADMAP.md`.

- Guidance: no Procfile requirement (riku.toml or Procfile, or the runtime's start command); the git remote line uses the known user and host (`hostname -f` at init; else omit the line rather than print `your-server`).
- quickstart: when no registered server, say `rk server add` instead of `<user>@<your-server>`.
- install.sh: server branch ends at `riku init`; the laptop branch shows `rk server add myserver riku@<host> --admin <you>@<host>`.

- [ ] Test `guidance_has_no_placeholder_or_procfile_requirement` in `guidance_tests.rs`.
- [ ] Commit `Fix first run guidance text`.

### Final gate

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `./scripts/run-docker-tests.sh`.
- [ ] Search the branch diff for the confidential names before any push.
