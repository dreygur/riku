# URL on First Push Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every app gets a URL it can open on its first push: its own domain, a fallback domain built from the server IP, a port of its own, or a path under a shared port.

**Architecture:**
- Server wide routing defaults live in a new `[routing]` section of settings.toml. Per app choices are new `[routing]` keys in riku.toml.
- The deploy side resolves both into one `AppRouting` value under a server wide routing lock. It records the result (mode, picked port, certificate outcome) in a riku owned state file, so later steps read the state instead of re-resolving.
- The route sent to the root helper grows an optional plain HTTP port that answers any host, and a separate request for the shared path site. nginx and Caddy each map those.
- `RIKU_NGINX_PORTMAP` and `RIKU_NGINX_EXTERNAL_PORT` go away.

**Tech Stack:** Rust workspace (riku-config, riku-deploy, riku-route, riku-nginx, riku-caddy, riku-util, riku-cli), Tera templates, nginx, Caddy.

**Spec:** `docs/superpowers/specs/2026-11-09-developer-experience-design.md`, sections "settings.toml", "riku.toml", "URL on first push" and "Calls made by Claude".

## Global Constraints

- `.claude/CLAUDE.md`:
  - No `?` operator. Tests live in sibling `_tests.rs` files. Files stay around 200 lines.
  - Doc comments on public items. Module docs carry the security model.
  - No emoji, em dashes or hyphenated compounds in prose.
- Native file tools only. No python or perl.
- CODEMAP and ROADMAP change in the same commit as the code.
- `route_build.rs` (267 lines), `init.rs` (273) and `ssl.rs` (399) are already over the size limit. New code goes in new modules.
- No service name (sslip.io, nip.io) in Rust code. The pattern appears only in the settings.toml text that init writes and in docs.
- Confidential names: use `prod`, `edge.example`, `myapp`. Grep the diff before every commit.
- riku.toml is a trust boundary: unknown keys and bad values refuse the push, naming the key and the line.
- The route crossing to the root helper keeps `deny_unknown_fields`. Every new field passes the gate in `crates/riku-route/src/check.rs` before rendering.
- **Helper skew.** A new optional field carries `#[serde(default, skip_serializing_if = "Option::is_none")]`, so an older helper still accepts routes that do not use it. A new request type is sent only when it is needed. The helper is a long running unit that only `riku init` and `doctor --fix` restart (`crates/riku-cli/src/setup/helper_install.rs:82`).
- Branch `feat/url-first-push` off main. Per task, run `cargo test -p <crate>`. The full suite, clippy, Docker e2e and the Linux run happen once, before the final review.

## Decisions this plan makes (spec left them open or silent)

1. **Key names.**
   - settings.toml `[routing]`: `fallback_domain` (text pattern), `server_ip` (IPv4 only), `shared_port` (int, default 8080), `port_range` (text `"A-B"`, default `"8100-8199"`).
   - riku.toml `[routing]`: `mode` (`"domain"`, `"port"`, `"path"`) and `port` (int, narrowed to 1 to 65535 in `values.rs`, with the key and the line in the error). Env names `RIKU_ROUTING_MODE` and `RIKU_ROUTING_PORT`.
2. **A server with no `[routing]` section** behaves exactly as today. That covers an upgraded server that has not run `doctor --fix`: no fallback, no implicit port mode and no new listeners. The push adds one line: "run riku doctor --fix to give apps without a domain a URL". Init writes the section on a fresh install. `doctor` reports a missing section and `--fix` writes it.
3. **Default mode, only when `[routing]` exists.**
   - Mode unset: `domain` if the app has `domains` or the fallback expands, else `port`.
   - `port` set with mode `domain` (or with mode unset and a domain) means the app answers on its domain and on the port.
   - Mode `path` with `port` or with `domains` refuses the push: "path mode takes no domains or port".
4. **Server IP detection.**
   - Use a UDP socket "connected" to `192.0.2.1:9` (TEST-NET-1; no packet is sent) and read its local address.
   - Only a public IPv4 address is written as `server_ip`.
   - With a private, loopback, link-local or IPv6 result, init leaves `server_ip` out, which turns the fallback off. It prints the detected address and the line to add by hand.
   - On NAT cloud VMs the operator adds the public address. A private address would give a URL that does not work from the internet and would waste ACME attempts.
   - A hand-set `server_ip` that is not IPv4 is refused when the settings load, naming the key.
5. **Routing state.**
   - Riku owns `<riku root>/routing/state.toml`. It has one entry per app: `mode`, `port` (picked or set), `tls` (`issued`, `failed_at = <unix seconds>`, `by_caddy`), and `routed_by` (`state` or `legacy`).
   - It is written atomically, and every read, pick and write happens under a server wide `flock` on `<riku root>/routing/lock`.
   - A picked port is used only while the resolved mode includes a port and riku.toml gives none. It is freed when the mode changes or the app gains a domain without `port`.
   - Destroy removes the entry. Riku.toml always wins over state; there is no override write.
6. **Ports in use.**
   - Refuse a port that another app holds in the state file, the Caddy admin port 2019, or `shared_port`.
   - Also refuse any port in LISTEN in `/proc/net/tcp` or `/proc/net/tcp6`, unless the state says this app already holds it. That covers sshd wherever it is configured, databases, pinned app `PORT`s and other daemons.
   - After a reload, the helper checks that `open_port` is in LISTEN (polling up to 2 s). If it is not, the helper restores the old files and reports a failure. This matters because `nginx -t` does not bind and a reload can report success when a listen fails.
7. **Ports 80 and 443 are refused in port mode.** This deviates from the spec, which lists them as allowed.
   - An `open_port` of 80 with `server_name _` would get no traffic, because init's `listen 80 default_server; return 444;` site wins.
   - 443 needs a certificate, and a bare IP cannot get one.
   - Serving `http://IP/` later would mean making exactly one app the `default_server` in place of the 444 site; that is out of scope.
   - The owner confirmed this deviation on 2026-11-10.
8. **Path mode.**
   - The prefix is stripped: `/<app>/x` reaches the app as `/x`, with `X-Forwarded-Prefix: /<app>`, and `/<app>` redirects to `/<app>/`.
   - The shared site file is `_paths.conf` / `_paths.caddy`. Task 5 first checks that `AppName` refuses a leading `_`; if it does not, the file is named `riku-paths`, which `AppName` already refuses for some other reason that Task 5 cites.
   - Path mode is refused for container apps in this plan.
9. **Certificates, in this order on nginx:**
   1. Apply the HTTP route.
   2. Issue the certificate.
   3. Apply again with TLS.

   This also fixes an existing bug: a first deploy of an `https_only` domain issues before any site serves the challenge, so it always falls back to self-signed today.
   - The fallback domain uses `issue_single`. That skips the issue when an unexpired pair exists, never uses a self-signed certificate, and after a failure does not retry until the next push and at least one hour later (`tls.failed_at`). Rollout publishes never issue.
   - With a pair, the fallback site serves HTTP and HTTPS with no redirect, so a later renewal failure cannot take the app offline. Without one, it serves HTTP and the push says so.
   - On Caddy the fallback domain gets `http://` and `https://` addresses, and the push says Caddy issues HTTPS in the background.
10. **Router plugins** keep getting a `RoutePlan` with the resolved domains, fallback included. Port and path mode are refused under a plugin router: "router plugin '<name>' supports domain mode only".
11. **`RIKU_NGINX_EXTERNAL_PORT` needs no migration.** `apply_route_keys` (`workers/web_env.rs:139-143`) rewrites it to 80 on every deploy, so no server can hold a custom value today.

## Review Focus

1. **An app name that is not a valid DNS label** (`my_app`) in the fallback: no fallback, and the push says why. Task 3 test `fallback_skipped_for_app_name_not_a_dns_label`.
2. **Two apps deploying at once in port mode** get different ports. Task 2 test `concurrent_picks_get_different_ports` (two threads, one shared riku root).
3. **An app switching mode** (domain to port, port to path, and back): the old listener and the old path entry are gone, and a picked port is freed. Task 5 test `switching_out_of_path_mode_drops_the_path_entry`; Task 4 test `gaining_a_domain_frees_the_picked_port`.
4. **An upgraded server without `[routing]`:** a domainless app routes exactly as before and opens no new port. Task 3 test `no_routing_section_keeps_todays_route`.
5. **A multi domain app:** the push lists every URL, not the raw `a,b` printed today (a bug in `crates/riku-deploy/src/deploy_finish.rs:22`). Task 3 test `announce_lists_each_domain`.

---

### Task 1: Server routing settings and IP detection

**Files:**
- Create: `crates/riku-config/src/settings/routing.rs` (+ `routing_tests.rs`)
- Create: `crates/riku-cli/src/setup/server_ip.rs` (+ `server_ip_tests.rs`)
- Modify: `crates/riku-config/src/settings/mod.rs:40,51,87` (field, accepted section), `crates/riku-cli/src/setup/init.rs` (append the section after `persist_router`, because `save_key` rewrites the file without comments), `crates/riku-cli/src/doctor/settings_check.rs`, `migrate_settings.rs`

**Interfaces (produces):**
- `pub struct RoutingSettings { pub fallback_domain: Option<FallbackPattern>, pub server_ip: Option<Ipv4Addr>, pub shared_port: u16, pub port_range: PortRange, pub present: bool }`. `present` is false when the section is absent; decision 2 keys off it. `Default` gives no fallback, no IP, 8080, 8100 to 8199, and `present: false`.
- `pub struct FallbackPattern(String)` with `TryFrom<&str>`: it must contain `{app}`, and other characters must be hostname characters or `{ip}`.
- `pub fn expand(&self, app: &str, ip: Ipv4Addr) -> Result<Domain, String>`. The error says why, for the push output.
- `pub struct PortRange { pub first: u16, pub last: u16 }` with `TryFrom<&str>`, refusing first > last or first < 1024.
- `pub fn detect_server_ip() -> Option<IpAddr>`, and `pub fn public_v4(ip: IpAddr) -> Option<Ipv4Addr>`.
- `pub fn routing_section(ip: Option<Ipv4Addr>) -> String`: the text init and doctor append. The comments are as in the spec, and the `server_ip` line is left out when `ip` is `None`.

- [ ] Tests first:
  - `routing_section_absent_gives_defaults_not_present`
  - `fallback_pattern_without_app_is_refused`
  - `port_range_reversed_is_refused`
  - `expand_fills_app_and_ip` (pattern `{app}.{ip}.example`, `myapp`, `203.0.113.10`)
  - `ipv6_server_ip_is_refused_naming_the_key`
  - `unknown_routing_key_names_the_key`
  - `private_and_loopback_are_not_public_v4`
  - `init_keeps_routing_comments_after_router_save`
- [ ] Implement, then `cargo test -p riku-config -p riku-cli`. Commit with the CODEMAP lines.

### Task 2: riku.toml keys, routing state and port rules

**Files:**
- Modify: `crates/riku-config/src/app_settings/keys.rs:86-107` (`routing.mode` Text, `routing.port` Int), `values.rs:79-90` (mode words, port range)
- Create: `crates/riku-deploy/src/router/app_routing.rs`, `routing_state.rs`, `port_rules.rs`, `listening.rs` (each with `_tests.rs`)

**Interfaces (produces):**
- `pub enum RoutingMode { Domain, Port, Path }`
- `pub struct AppRouting { pub domains: Vec<String>, pub fallback: bool, pub port: Option<u16>, pub mode: RoutingMode, pub legacy: bool }`.
  - `domains` keeps `_` as today (`route_build.rs:77`).
  - `legacy` is true when settings have no `[routing]` (decision 2).
- `pub fn resolve(app: &AppName, env: &HashMap<String, String>, settings: &RoutingSettings, state: &mut RoutingState, plugin_router: bool) -> Result<AppRouting>`. It is pure apart from `state`.
- `pub struct RoutingState`, with these functions:
  - `load(paths)`, `save(&self, paths)`
  - `entry(app)`, `set_entry(app, Entry)`, `remove(app)`
  - `pub fn with_routing_lock<T>(paths: &RikuPaths, f: impl FnOnce(&mut RoutingState) -> Result<T>) -> Result<T>`: flock, load, call, save.
- `pub fn check_port(app: &AppName, port: u16, state: &RoutingState, settings: &RoutingSettings, listening: &BTreeSet<u16>) -> Result<()>`
- `pub fn pick_port(app, state, settings, listening) -> Result<u16>`
- `pub fn listening_ports() -> BTreeSet<u16>`, which parses `/proc/net/tcp{,6}`. Its parser is a pure function, tested on fixture text.

- [ ] Tests first:
  - `mode_outside_three_words_is_refused_with_line`
  - `port_above_65535_is_refused_with_line`
  - `no_domain_and_fallback_resolves_to_domain_mode`
  - `no_domain_no_fallback_resolves_to_port_mode`
  - `no_routing_section_resolves_legacy`
  - `path_mode_with_domains_or_port_is_refused`
  - `port_mode_under_plugin_router_is_refused`
  - `port_held_by_other_app_is_refused_naming_it`
  - `listening_port_is_refused`
  - `own_listening_port_is_allowed`
  - `port_443_and_80_are_refused`
  - `exhausted_port_range_is_refused`
  - `picked_port_is_lowest_free`
  - `concurrent_picks_get_different_ports`
  - `proc_net_tcp_parser_reads_listen_rows_only`
- [ ] Implement, then `cargo test -p riku-config -p riku-deploy`. Commit.

### Task 3: Domain mode with the fallback, certificate order, push URLs

**Files:**
- Create: `crates/riku-deploy/src/router/route_domains.rs` (domains from `AppRouting`, replacing `nginx_domains` at `route_build.rs:70-77`. The `<app>.example.com` placeholder stays only for `legacy`, and for the port mode case until Task 4 lands.)
- Modify:
  - `route_build.rs`, to the new signature `build_route(app: &AppName, env: &HashMap<String, String>, routing: &AppRouting, backends, paths, router) -> Result<Option<Route>>`. Its callers change with it: `caddy.rs:39`, `nginx_site.rs:89` (`nginx_route`, also used by `check_nginx_site` at :151), `renew.rs`, and `router.rs:176` (`route_plan` for plugins).
  - `route_build.rs:189-196`, so the TLS pair is loaded for `AlsoPlainHttp` too when one exists.
- Modify: `router.rs:143` `apply_to`. It resolves under `with_routing_lock`, records the entry, and is the one place the four callers meet: `configure` :106, `apply` from `web_rollout.rs:57`, `containers/routes.rs:34`, `containers/copy_restart.rs:26`.
- Modify: `nginx_site.rs:62-125`, to apply HTTP, issue, then apply with TLS (decision 9), for every domain app with no pair. Issuing happens only from `configure` (the deploy), never from a rollout publish.
- Create: `crates/riku-nginx/src/ssl_single.rs` (`pub fn issue_single(paths, app, domain: &str) -> Result<bool>`)
- Modify: `crates/riku-nginx/src/route_site.rs:46-61` (a `plain_and_tls: bool` flag), `templates/nginx_portmap.conf.tera` (a 443 block), `crates/riku-route/src/nginx_render.rs:64-75` (set the flag from `AlsoPlainHttp` plus a pair)
- Modify: `crates/riku-deploy/src/deploy_finish.rs:21-39`. `announce` reads the app's state entry and settings, not just `env`, and prints one URL per domain plus the certificate outcome.

**Interfaces:**
- Consumes `AppRouting`, `resolve`, `with_routing_lock` and the state `tls` field (Task 2), and `RoutingSettings` (Task 1).

- [ ] Tests first:
  - `fallback_domain_routes_app_without_domains` (nginx and Caddy)
  - `fallback_skipped_for_app_name_not_a_dns_label`
  - `no_routing_section_keeps_todays_route`
  - `nginx_applies_http_before_issuing` (order recorded through a mock issuer)
  - `failed_fallback_certificate_serves_http_and_says_so`
  - `failed_issue_is_not_retried_within_an_hour`
  - `unexpired_pair_is_reused_without_issuing`
  - `issued_fallback_serves_both_without_redirect` (the rendered text has `listen 0.0.0.0:443 ssl` and no `return 301`)
  - `fallback_route_carries_fullchain`
  - `https_only_does_not_apply_to_fallback`
  - `renew_covers_the_fallback_domain`
  - `plugin_route_plan_gets_the_fallback_domain`
  - `announce_lists_each_domain`
  - `no_domain_no_fallback_says_how_to_add_one`
  - Update `route_build_tests.rs:69,105,109,130,218,255`.
- [ ] Implement, then `cargo test -p riku-deploy -p riku-nginx -p riku-route -p riku-caddy`. Commit.

### Task 4: Port mode replaces the nginx port map keys

**Files:**
- Modify: `crates/riku-route/src/schema.rs`. Add `open_port: Option<u16>` (serde default, skipped when `None`). `domains` may be empty only when `open_port` is set. `listen` stays, for wsgi and for Task 5.
- Modify: `check.rs`. `open_port` must be 1024 or above, and not 2019. Caddy takes `open_port`; its refusals of a custom `listen` and of `SiteKind::Portmap` stay as they are.
- Modify: `crates/riku-util/src/route_plan.rs:21` (`RoutePlan.open_port`, same serde attributes), `crates/riku-route/src/caddy_apply.rs:81` (`plan_of` copies it), `crates/riku-caddy/src/site.rs:22-50` (an extra `http://:<port>` address)
- Modify: `crates/riku-nginx/src/route_site.rs` and the templates (a server block `listen <open_port>; server_name _;` with the same locations)
- Modify: `crates/riku-route/src/nginx_apply.rs:72-108` and `caddy_apply.rs:98`. After a reload, the bind check from decision 6, which restores the old files on failure.
- Modify: `route_build.rs:240` (port mode builds a route with no domain on Caddy), `route_domains.rs` (drop the placeholder for port mode)
- Modify: `workers/web_env.rs:17,104-143` (stop writing the two keys; `drop_route_keys` removes them), `web_rollout.rs:43` (doc), `router/route_env.rs:17-39` (delete `listen`), `route_build.rs:101` (`SiteKind::Portmap` is chosen from a TCP backend, not the env key), `crates/riku-nginx/src/backends.rs:94-102`
- Modify: `crates/riku-util/src/legacy_env.rs:39` (drop the `NGINX_EXTERNAL_PORT` row), `crates/riku-cli/src/apps/dump_state/builder.rs:23-24`
- Modify: `deploy_finish.rs`. Port URLs are `http://<server_ip>:<port>`, or `http://<this server's address>:<port>` when there is no `server_ip`, followed by "open port N in the firewall" and "a bare IP gets HTTP only".

- [ ] Tests first:
  - `port_mode_route_has_open_port_and_no_domain`
  - `route_without_open_port_serializes_without_the_field`
  - `open_port_below_1024_is_refused_by_gate`
  - `caddy_takes_open_port`
  - `nginx_renders_catch_all_server_on_open_port`
  - `domain_and_port_render_both`
  - `failed_bind_restores_old_site` (bind check with a mock probe)
  - `picked_port_is_reused_next_deploy`
  - `gaining_a_domain_frees_the_picked_port`
  - `stale_portmap_keys_are_ignored_and_dropped`
  - Update `web_env_tests.rs`, `workers_tests.rs:345-366`, `router_tests.rs:91`, riku-nginx `backends_tests.rs`, and `tests/integration_tests/{nginx,security,e2e}_tests.rs`.
- [ ] Implement, then `cargo test -p riku-route -p riku-nginx -p riku-caddy -p riku-deploy -p riku-util -p riku-cli`. Commit.

### Task 5: Path mode on the shared port

**Files:**
- Modify: `crates/riku-route/src/protocol.rs:30` (`Request::ApplyPaths { router: RouterKind, listen: Listen, apps: Vec<PathApp> }`, `pub struct PathApp { pub app: String, pub backends: Vec<RouteBackend> }`, both with `deny_unknown_fields`), `answer.rs:32-46` (dispatch)
- Create: `crates/riku-route/src/paths_check.rs` (every `app` passes `AppName`, no duplicates, backends follow the backend rule, `listen` follows the listen rule), `paths_apply.rs` (write `_paths.conf` / `_paths.caddy`, reload, bind check, restore on failure; an empty list removes the file)
- Create: `templates/nginx_paths.conf.tera`, and `render_paths` in `crates/riku-caddy/src/site.rs`
- Create: `crates/riku-deploy/src/router/paths.rs`. `pub fn publish_paths(paths: &RikuPaths, router: Router, settings: &RoutingSettings) -> Result<()>` runs under the routing lock:
  - It takes the path mode apps from the state file, never by re-resolving each app.
  - It reads each one's backends with `current_backends` (`router/backends.rs:145`).
  - It sends `ApplyPaths` only when the list is non-empty or the state records that a paths site was published.
  - It is called after `apply_to` records an entry whose mode changed into or out of `Path`, and from destroy (`router/remove.rs:33`) after the entry is removed.
- Modify: `deploy_finish.rs` to print `http://<server_ip>:<shared_port>/<app>/` and "the app must handle the base path /<app>".

- [ ] Tests first:
  - `path_request_with_unknown_field_is_refused`
  - `path_app_name_starting_with_underscore_is_refused`
  - `nginx_paths_strip_prefix_and_send_forwarded_prefix`
  - `caddy_paths_strip_prefix`
  - `bare_app_path_redirects_to_slash`
  - `empty_path_list_removes_the_site`
  - `no_path_apps_sends_no_request`
  - `switching_out_of_path_mode_drops_the_path_entry`
  - `destroy_drops_the_path_entry`
  - `container_app_in_path_mode_is_refused`
  - `nginx_paths_conf_passes_nginx_t`, in the `nginx_syntax_tests.rs` style
- [ ] Implement, then `cargo test -p riku-route -p riku-nginx -p riku-caddy -p riku-deploy`. Commit.

### Task 6: Docs, guidance and the final gate

**Files:**
- `docs/docs/` routing page: the three modes, the fallback pattern, port rules, firewall, TLS behavior and NAT
- `docs/docs/quick-start.md`, `docs/docs/cli.md`
- `crates/riku-cli/src/setup/guidance.rs`: init prints the detected IP and whether the fallback is on
- `docs/CODEMAP.md`
- `docs/superpowers/ROADMAP.md`: the 2b status, and mark the first deploy `https_only` self-signed bug as fixed

- [ ] Search docs and examples for `RIKU_NGINX_PORTMAP` and `RIKU_NGINX_EXTERNAL_PORT` and replace them with the riku.toml keys.
- [ ] Gate:
  - `cargo fmt --check`, `cargo clippy --workspace` (0 warnings), `cargo test --workspace`
  - `./scripts/run-docker-tests.sh`
  - a manual push in each mode against nginx and Caddy in the Docker test image (`curl -H Host:`, `curl :<port>`, `curl :8080/myapp/`)
  - confidential grep of the diff and the log
- [ ] Final Opus review of the whole branch, then one fix round.
