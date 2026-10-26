# Built-in Caddy router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A host set to the `caddy` router deploys, destroys, reloads and diagnoses apps through Caddy with no nginx installed. nginx stays the default.

**Architecture:** The host router choice moves from the `RIKU_ROUTER` process env into a `<riku root>/ROUTER` file, read into a `Router` enum in `riku-config`. A new `riku-caddy` crate renders one site file per app from a typed request whose domains are validated `Domain` values from `riku-util`. Deploy, destroy, supervisor reload, init, systemd units, doctor, the dashboard and `apps info` match on `Router` instead of assuming nginx.

**Tech Stack:** Rust workspace, systemd path units, Caddyfile syntax, mkdocs.

**Spec:** `docs/superpowers/specs/2026-10-26-router-neutral-caddy-design.md` (Part 2)

## Global Constraints

- No `?` in new or touched functions; explicit `match` / `return Err` (CLAUDE.md rule 16), `#[allow(clippy::question_mark)]` where clippy objects.
- Tests in sibling `_tests.rs` via `#[path]`. Touching a file with inline tests moves them out.
- Files past ~200 lines get split by concern.
- No dashes in prose, no emoji, no AI attribution in commits.
- riku never talks to the Caddy admin API. Reloads go through `systemctl reload caddy`, triggered by a root path unit.
- Breaking changes are fine (single user). No compatibility aliases.

## Decisions made in this plan

- **Router source:** `<riku root>/ROUTER` holds one name (`nginx`, `caddy`, or a router plugin name). Missing file means nginx. `riku init` writes it from `RIKU_ROUTER`; nothing else reads `RIKU_ROUTER`. Reason: git push deploys run through the ssh `command=` key and the post-receive hook, which carry no `RIKU_ROUTER`, so an env-only choice made those deploys write nginx config on a Caddy host. (Chosen by the owner.)
- **Router errors fail the deploy, for every router.** `spawn_app` today downgrades a `router::configure` error to a yellow warning, so an invalid domain would not fail the deploy as the spec requires. The warning becomes a returned error. For nginx this means a config that fails `nginx -t` now fails the deploy instead of leaving the app unreachable behind a success message.
- **Caddy only proxies TCP.** Apps whose web process is a uWSGI socket kind (`wsgi`, `jwsgi`, `rwsgi`, `php`) or a nginx static site (`RIKU_NGINX_STATIC`) fail the deploy under Caddy with a message naming the fix (a `web:` process on `$PORT`). Caddy cannot speak the uwsgi protocol.
- **File readability for the `caddy` user:** init adds `caddy` to the deploy user's primary group, sets `g+x` (traverse, no listing) on the deploy home and `<riku root>`, and `0750` on `<riku root>/caddy`. nginx needs none of this today because its master reads config as root, so there is nothing to match.
- Hostname validation lives in `riku-util` as a `Domain` newtype, so nginx can adopt it later.
- `RIKU_ACME_EMAIL` and friends are not wired into Caddy. Caddy's account email is a global option in `/etc/caddy/Caddyfile`, which the owner sets once.

## Review Focus

1. `RIKU_DOMAINS="a.com {\n}"` or `a.com;b` under Caddy: deploy fails naming the bad value, no site file is written, an existing site file for that app is left as it was.
2. An app that had domains drops `RIKU_DOMAINS`: its old site file is removed, so Caddy stops serving a stale route.
3. `ROUTER` containing `caddy\n` or ` Caddy `: parsed as Caddy. An unreadable `ROUTER` (permission error) is an error, never a silent fallback to nginx.
4. `riku init` run twice on a Caddy host: the `import` line appears once in `/etc/caddy/Caddyfile`.
5. A `wsgi:` app under Caddy: deploy fails with the TCP only message instead of writing a site that proxies to nothing.

---

### Task 1: Router choice from the ROUTER file

**Files:**
- Create: `crates/riku-config/src/router.rs`, `crates/riku-config/src/router_tests.rs`
- Modify: `crates/riku-config/src/lib.rs` (add `pub mod router;`, `caddy_root` and `router_file` fields)
- Modify: `crates/riku-deploy/src/router.rs` (drop `active_router`/`DEFAULT_ROUTER`, use `Router::load`; move inline tests to `router_tests.rs`)
- Modify: `crates/riku-cli/src/setup/init.rs` (persist the choice)

**Interfaces:**
- Produces in `riku_config::router`:
  - `pub enum Router { Nginx, Caddy, Plugin(String) }`
  - `impl Router { pub fn load(paths: &RikuPaths) -> anyhow::Result<Router>; pub fn name(&self) -> &str }`
  - `impl From<&str> for Router` (trimmed, ASCII lowercased; empty is `Nginx`)
  - `pub fn save(paths: &RikuPaths, router: &Router) -> anyhow::Result<()>` (writes `name()` plus newline with `fs::write`; riku-config has no riku crate deps and stays that way)
- Produces on `RikuPaths`: `pub caddy_root: PathBuf` (`<root>/caddy`), `pub router_file: PathBuf` (`<root>/ROUTER`).

- [ ] **Step 1: Failing tests** in `router_tests.rs`:
  - `missing_file_means_nginx`: fresh tempdir root, `Router::load` is `Ok(Router::Nginx)`.
  - `file_name_is_trimmed_and_case_folded`: file `" Caddy \n"` loads `Router::Caddy`; file `"traefik"` loads `Router::Plugin("traefik".into())`.
  - `unreadable_file_is_an_error`: `router_file` created as a directory, `load` is `Err` (stands in for a permission error, which root test runners cannot produce).
  - `save_then_load_round_trips` for `Router::Caddy`.
- [ ] **Step 2:** `cargo test -p riku-config router` fails to compile.
- [ ] **Step 3:** Implement. `load` maps `ErrorKind::NotFound` to `Nginx`, any other io error to `Err` with the path in the message.
- [ ] **Step 4:** In `riku-deploy/src/router.rs`, `configure` and `remove` call `Router::load(paths)` and `match` on it: `Nginx` keeps today's calls, `Plugin(name)` keeps today's plugin path, `Caddy` returns `Err(anyhow!("caddy router not built yet"))` until Task 3 replaces it. Update the module doc: the router comes from `<riku root>/ROUTER`.
- [ ] **Step 5:** `init`: when `RIKU_ROUTER` is set and non-empty, `save(paths, &Router::from(value))` right after the directory step and echo `      ✓ router: <name>`. Unset leaves an existing file alone.
- [ ] **Step 6:** `git grep -n RIKU_ROUTER -- crates` shows only `init.rs`. `cargo test -p riku-config -p riku-deploy` passes.
- [ ] **Step 7:** Commit `Read the host router from the ROUTER file`.

### Task 2: Domain newtype

**Files:**
- Create: `crates/riku-util/src/domain.rs`, `crates/riku-util/src/domain_tests.rs`
- Modify: `crates/riku-util/src/lib.rs`

**Interfaces:**
- Produces in `riku_util::domain`:
  - `pub struct Domain(String)` (always lowercase), `impl Domain { pub fn as_str(&self) -> &str }`, `impl Display`
  - `impl TryFrom<&str> for Domain` (`Error = anyhow::Error`)
  - `pub fn parse_domains(raw: &str) -> anyhow::Result<Vec<Domain>>`: splits on `,` and whitespace like `domains_from_env` does today, skips empties, fails on the first invalid entry naming it.

Rules: total length 1 to 253; labels 1 to 63 chars of `[a-z0-9-]`, not starting or ending with `-`; the first label may be exactly `*`; input is lowercased first; no trailing dot.

- [ ] **Step 1: Failing tests:**
  - `accepts_hostnames_and_leading_wildcard`: `"Shop.Example.com"` gives `"shop.example.com"`; `"*.example.com"` and `"app.localhost"` accepted.
  - `rejects_config_syntax_and_bad_labels`: each of `"a.com {"`, `"a.com;b"`, `"a b"`, `"-a.com"`, `"a..com"`, `"a.*.com"`, `""`, a 64 char label is `Err`, and the error text contains the input.
  - `parse_domains_fails_on_first_bad_entry`: `"a.com, b.com"` gives two; `"a.com, {x}"` is `Err` mentioning `{x}`.
- [ ] **Step 2:** `cargo test -p riku-util domain` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests pass.
- [ ] **Step 5:** Commit `Add a validated Domain type`.

### Task 3: riku-caddy crate and deploy wiring

**Files:**
- Create: `crates/riku-caddy/` (`Cargo.toml`, `src/lib.rs`, `src/site.rs`, `src/site_tests.rs`); add to workspace `Cargo.toml`
- Modify: `crates/riku-deploy/Cargo.toml`, `crates/riku-deploy/src/lib.rs` (`pub(crate) use riku_caddy as caddy;`)
- Modify: `crates/riku-deploy/src/router.rs`, `crates/riku-deploy/src/router_tests.rs`
- Modify: `crates/riku-deploy/src/supervisor_ctl.rs` (`spawn_app` returns the router error)

**Interfaces:**
- Consumes: `Router` (Task 1), `Domain`, `parse_domains` (Task 2), `RikuPaths.caddy_root`.
- Produces in `riku_caddy`:
  - `pub struct SiteRequest { pub domains: Vec<Domain>, pub upstream_port: u16, pub https_only: bool }`
  - `pub fn render_site(app: &str, site: &SiteRequest) -> String`
  - `pub fn write_site(app: &str, site: &SiteRequest, paths: &RikuPaths) -> anyhow::Result<()>` (atomic write to `caddy_root/<app>.caddy`, creates the dir)
  - `pub fn remove_site(app: &str, paths: &RikuPaths) -> anyhow::Result<()>` (missing file is fine)

Site layout (exact):

```
# riku app: shop
shop.example.com, www.shop.example.com, http://shop.example.com, http://www.shop.example.com {
	reverse_proxy 127.0.0.1:5000
}
```

The `http://` addresses appear only when `https_only` is false; Caddy then serves plain HTTP for those hosts instead of redirecting. With `https_only` the address list is just the bare domains and Caddy's automatic redirect applies. `app` is already a validated app name by deploy time.

- [ ] **Step 1: Failing tests** in `site_tests.rs`:
  - `https_only_site_lists_bare_domains`: `render_site("shop", ...)` with two domains, port 5000, `https_only: true` equals the layout above minus the `http://` addresses.
  - `plain_site_adds_http_addresses`: same request with `https_only: false` equals the layout above exactly.
  - `write_then_remove_site`: file appears under `caddy_root`, `remove_site` deletes it, a second `remove_site` is `Ok`.
- [ ] **Step 2:** `cargo test -p riku-caddy` fails.
- [ ] **Step 3:** Implement the crate.
- [ ] **Step 4: Failing tests** in `router_tests.rs` for a `fn caddy_request(env: &HashMap<String, String>) -> anyhow::Result<Option<SiteRequest>>` helper in `router.rs`:
  - `caddy_request_rejects_bad_domain`: `RIKU_DOMAINS="a.com {"` is `Err` containing `a.com {`.
  - `caddy_request_none_without_domains`: empty or missing `RIKU_DOMAINS` is `Ok(None)`.
  - `caddy_request_rejects_socket_and_static_apps`: `RIKU_NGINX_WSGI=true`, and separately `RIKU_NGINX_STATIC=true`, give `Err` containing `Caddy proxies TCP only`.
- [ ] **Step 5:** Implement `caddy_request` (port and HTTPS from the existing `upstream_port_from_env` / `https_from_env`). `configure` under `Router::Caddy`: `Ok(Some(site))` writes the site; `Ok(None)` calls `remove_site` and echoes `-----> No RIKU_DOMAINS set, app is not routed. Set one with: riku config set <app> RIKU_DOMAINS=<domain>`. `remove` under Caddy calls `remove_site`. No reload call: the `riku-caddy.path` unit (Task 4) reloads Caddy.
- [ ] **Step 6:** `spawn_app` returns `Err` from `router::configure` instead of echoing a warning. Check `cargo test -p riku-deploy` and the e2e tests still pass; any test that relied on the warning path is updated to expect the error.
- [ ] **Step 7:** Commit `Route apps through Caddy`.

### Task 4: Host wiring in init and systemd

**Files:**
- Create: `crates/riku-cli/src/setup/caddy_host.rs`, `crates/riku-cli/src/setup/caddy_host_tests.rs`
- Modify: `crates/riku-cli/src/setup/init.rs`, `system_service.rs` (split the unit writers into `router_units.rs` if it passes 200 lines), `user_service.rs`
- Test: `crates/riku-cli/src/setup/router_units_tests.rs`

**Interfaces:**
- Consumes: `Router::load` (Task 1).
- Produces:
  - `fn caddyfile_with_import(current: &str, import_line: &str) -> Option<String>` (`None` when an identical line is already present, else `current` plus the line on its own line)
  - `pub fn setup_caddy_host(paths: &RikuPaths, deploy_user: &str) -> anyhow::Result<()>`
  - `fn router_unit_after(router: &Router) -> &'static str` (`"nginx.service"`, `"caddy.service"`, or `""` for a plugin) used by both service writers for `After=` / `Wants=`.

- [ ] **Step 1: Failing tests:**
  - `import_added_once`: on `":80 {\n}\n"` returns text ending in `import /home/deploy/.riku/caddy/*.caddy\n`; on that result returns `None`.
  - `units_order_on_the_active_router`: rendering the riku system unit for `Router::Caddy` contains `After=network.target caddy.service` and no `nginx`; for `Router::Nginx` it keeps today's lines.
  - `caddy_path_unit_watches_caddy_dir`: the path unit text contains `PathModified=/home/deploy/.riku/caddy` and `Unit=riku-caddy-reload.service`; the reload service runs `/usr/bin/systemctl reload caddy`.
- [ ] **Step 2:** Tests fail.
- [ ] **Step 3:** Implement. Unit writers take the router and pick `riku-nginx.path`/`riku-nginx-reload.service` or `riku-caddy.path`/`riku-caddy-reload.service`, and enable/start the matching path unit. `setup_caddy_host` (root only): fail if `caddy` is not on PATH; read `/etc/caddy/Caddyfile` (missing file is an error telling the user to install the caddy package); write it back when `caddyfile_with_import` returns `Some`; run `usermod -aG <deploy group> caddy`; `chmod g+x` the deploy home and `riku_root`; create `caddy_root` with mode `0750`. Every command failure returns an error naming the command.
- [ ] **Step 4:** `init` under Caddy: prerequisite check looks for `caddy` instead of `nginx`, skips `generate_acme_config`, creates `caddy_root` instead of `nginx_root`, calls `setup_caddy_host` when root. The non-root warning lists "Caddy configuration" instead of "Nginx configuration".
- [ ] **Step 5:** Tests pass. The usermod/chmod calls are not unit tested; they run as root against real users. Say so in a comment on `setup_caddy_host`.
- [ ] **Step 6:** Commit `Wire Caddy into init and systemd`.

### Task 5: Router aware reload, doctor, dashboard and info

**Files:**
- Modify: `crates/riku-supervisor/src/daemon/mod.rs` (SIGHUP path)
- Modify: `crates/riku-cli/src/doctor/checks.rs` (`dependencies`, `directories`, replace `nginx()` with `router(paths)`; move it to `doctor/router_check.rs` since `checks.rs` is already 290 lines), `doctor/mod.rs`
- Modify: `crates/riku-dashboard/src/system.rs`
- Modify: `crates/riku-cli/src/apps/info.rs`, `crates/riku-cli/src/apps/dump_state.rs`
- Test: `crates/riku-cli/src/doctor/router_check_tests.rs`, existing `dump_state_tests.rs`

**Interfaces:**
- Consumes: `Router::load`, `RikuPaths.caddy_root`.
- Produces: `pub fn router(paths: &RikuPaths) -> Vec<Check>` in `doctor/router_check.rs`: nginx keeps the `nginx -t` logic; Caddy runs `caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile` and checks the import line is present; a plugin reports `Check::ok("router", "plugin <name>")`; a `ROUTER` read error is `Check::fail`.

- [ ] **Step 1: Failing tests:**
  - `caddy_router_missing_import_warns`: with a fake Caddyfile path injected (make the Caddyfile path a parameter of an inner fn), no import line gives a `Warn` whose detail names the import line to add.
  - `dump_state_reports_caddy_site`: with `ROUTER=caddy` and `caddy/<app>.caddy` present, the app entry reports that path and no nginx fields.
- [ ] **Step 2:** Tests fail.
- [ ] **Step 3:** Implement. SIGHUP: `Router::Nginx` calls `reload_nginx()` as today; Caddy and plugins do nothing (the path unit or plugin owns reloads), with a one line comment saying so. `dependencies()` requires `caddy` instead of warning about `nginx` on a Caddy host. `apps info` prints `Caddy Site:` with the path under Caddy.
- [ ] **Step 4:** `cargo test --workspace` passes.
- [ ] **Step 5:** Commit `Make reload, doctor and info router aware`.

### Task 6: Remove the shell plugin, docs, real Caddy check

**Files:**
- Delete: `examples/plugins/caddy-router/`
- Create: `docs/docs/caddy.md`; modify `mkdocs.yml` nav (next to `Nginx: nginx.md`)
- Modify: `README.md`, `PLUGIN_PROTOCOL.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `docs/docs/plugin-bundles.md`, `crates/riku-plugins/src/router/mod.rs` doc (router choice now comes from `ROUTER`, Caddy is built in, the plugin seam is for third party routers)

- [ ] **Step 1:** `caddy.md`: switching a host (`sudo RIKU_ROUTER=caddy riku init`), what init changes (Caddyfile import, group, path unit), the site file layout, `RIKU_HTTPS_ONLY` behavior, the TCP only limit, setting the ACME email in `/etc/caddy/Caddyfile`.
- [ ] **Step 2:** `git grep -n 'caddy-router\|RIKU_ROUTER=' -- . ':!docs/superpowers'` only shows init docs and `caddy.md`.
- [ ] **Step 3:** Real Caddy check (needs `brew install caddy` or a Linux box; ask the owner first): render both site variants with a throwaway test binary or `cargo test -- --nocapture` output, wrap them in a Caddyfile importing them, and run `caddy adapt --config <file> --adapter caddyfile`. Expected: exit 0, and the JSON for the `https_only: false` site shows no redirect route for the `http://` hosts. If this cannot run, report it as unverified.
- [ ] **Step 4:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast`, `mkdocs build --strict` all pass.
- [ ] **Step 5:** Commit `Document the built-in Caddy router`.
