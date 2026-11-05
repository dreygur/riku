# Rootless container engine

Addendum to docs/superpowers/specs/2026-10-30-container-services-design.md.
That spec says riku drives one Docker compatible CLI and probes capability,
never the engine name. This one says which engine that CLI talks to, how
root setup makes it rootless, and how the router stops being a path to root.

Refreshed 2026-11-05 against `feat/container-integration` at ffd7909, after
phase 3a (PR #73). Claims about today's code carry a file:line or a commit.
Claims about engines, kernels and init systems come from upstream docs or
memory and are marked unverified until the real host check runs.

riku has one user, so there is no migration and no backward compatibility;
reinstall and redeploy.

## Status against the code (2026-11-05)

| Section | Status | Evidence |
|---|---|---|
| Rootless engine (Why 1, Engine env, Root init, Init systems, Rootful mode, Ports) | still needed | nothing in crates/ sets `XDG_RUNTIME_DIR` or `DOCKER_HOST`, reads `info` or `ip_unprivileged_port_start`, or detects an init system |
| Router helper (Why 2) | prerequisites done, helper still needed | sockets, static files, ACME webroot and Caddy sites already live outside the deploy home (42d11f4, 117b33c, c087210, c69aa1f); nginx still loads the deploy user's `~/.riku/nginx/*.conf` (crates/riku-nginx/src/host.rs:25) |
| Caddy out of the deploy group | done | c087210; crates/riku-cli/src/setup/caddy_host.rs:1-17 |
| nginx include root owned, stray conf check, catch all | done | 4294a66, f3fee94; crates/riku-cli/src/setup/nginx_host.rs:55-72, crates/riku-nginx/src/host_check.rs:36, crates/riku-nginx/src/default_server.rs:18 |
| One root owned binary | still needed | crates/riku-cli/src/setup/binary.rs:13 installs to `~/.local/bin` |
| `riku update` | done (removed) | 6a5307a |
| Legacy container plugin retired | done | bb4aa56 deleted crates/riku-plugin-container |
| Image import onto the driver | open, see Open questions | crates/riku-deploy/src/container_runtime.rs:18-31, 128-160 |
| Footprint baseline | updated below | optz-runtime-report.md and check-5-and-benchmark-report.md in the 3a ledger dir |
| doctor | still needed | crates/riku-cli/src/doctor/ has no engine mode, helper or init system check |

## Why

The deploy user is meant to be unprivileged. Two things make it root
equivalent today:

1. A rootful engine. Whoever talks to it can start a container that mounts
   `/`. riku never adds the deploy user to the docker group (owner decision;
   the group change was dropped at 4294a66), so today the owner must do that
   by hand, which makes the deploy user root.
2. nginx. Its master runs as root and parses every
   `<riku root>/nginx/*.conf`, a directory the deploy user owns (see
   "Router helper").

Rootless is pointless without fixing the second, so both ship in the same
release (owner decision).

## Goals

1. The engine runs rootless by default, as the deploy user: container root
   maps to the deploy user's uid, other container uids to its subuid range.
2. Root `riku init` configures it on systemd hosts for docker and podman,
   and on OpenRC hosts for podman, and gives every process that runs the
   engine the same env.
3. Rootful only when the owner sets it, on OpenRC when docker is the only
   engine, or on a host with another init system. doctor flags it.
4. Containers stay an add-on. A host with no engine gets no rootless setup
   and no engine checks, and init and doctor say nothing about engines.
5. The footprint the README states is kept (see "Footprint").
6. The deploy user never writes a file a root process parses as router
   config.

## Non goals

- No OCI runtime, engine API, containerd or per engine driver (owner,
  2026-11-03). The CLI driver in crates/riku-container/src/engine/ stays.
- Installing anything. init configures the engine, uidmap, pasta or
  slirp4netns, fuse-overlayfs already present and names what is missing.
- Changing the KNOWN_ENGINES order (crates/riku-container/src/engine/detect.rs:17).
- Rootless docker on OpenRC. Deferred until users ask (owner, 2026-11-04).
- Procfile worker isolation. Its cgroup root `/sys/fs/cgroup/riku/apps`
  (crates/riku-supervisor/src/cgroups/mod.rs:20) is created and delegated
  by nothing in init. Separate fix.
- Router plugins. A third party router plugin keeps running as the deploy
  user; the helper covers the built in nginx and Caddy routers only.

## Anti goals

- Adding the deploy user to the docker group, or riku running any grant.
- Changing or deleting rootful volumes.
- Quietly running rootful because rootless setup failed. init refuses.
- Branching on the engine's or the distribution's name. Engine binary names
  appear only in messages, as detect.rs:24-25 already requires.

## Decisions (owner, 2026-11-04, updated 2026-11-05)

1. Router: the root router helper below, shipped with rootless. init
   refuses to set rootless mode unless the helper is installed.
2. Init systems: rootless is supported on systemd for docker and podman, and
   on OpenRC for podman only. Rootless docker on OpenRC is deferred until
   users ask; with docker as the only engine there, init falls back to
   rootful and prints the grant, and doctor says why. Any other init system
   runs rootful with a doctor warning saying why.
3. Rootful fallback: init and doctor print the exact grant command for the
   engine, with a plain warning that it makes the deploy user root
   equivalent. riku never runs it.
4. Supervisor: on systemd rootless hosts it runs as a `systemd --user` unit
   under the lingering deploy user's manager. The system unit stays for
   rootful mode. On OpenRC it runs under `supervise-daemon` as the deploy
   user.
5. Root owned state lives in `/var/lib/riku/`, which root init already
   creates root owned 0755 (crates/riku-cli/src/setup/var_root.rs:43-72).
   The legacy container plugin is gone (bb4aa56).
6. Containers stay an add-on. When no engine is installed, riku does no
   rootless setup and no engine checks.
7. `riku router-helper` is a subcommand of the one riku binary.
8. Binary growth is capped at 10 percent of the baseline in "Footprint",
   rounded up to 0.5 MiB.
9. One root owned binary at `/usr/local/bin/riku`. The deploy user runs it
   and cannot change it. Updates need root: install the new binary as root,
   then `sudo riku init` (`riku update` is gone, 6a5307a).
10. An absent `engine.mode` means `rootless`. The router helper is installed
    on every host with a router. On init systems other than systemd and
    OpenRC the operator starts the helper by hand.

## How the engine runs today

- Spawning. Every engine call is `Command::new(program)` with the caller's
  env inherited and nothing added: bounded.rs:163, streamed.rs:34,
  login.rs:30, events.rs:39; logs go through bounded's `capture_capped`
  (logs.rs:15). The probes take a bare program path, not an `Engine`
  (probe.rs:20, 26, 44) and run in parallel since ffd7909.
- Detection. `<riku root>/CONTAINER_ENGINE`, a name or absolute path
  (crates/riku-config/src/container_engine.rs:1-20), else the first of
  KNOWN_ENGINES on `PATH` (detect.rs:34-75), `PATH` read from the process
  env (crates/riku-deploy/src/containers/engine_open.rs:44). `settings.toml`
  `[engine]` holds timeouts and refuses unknown keys
  (crates/riku-config/src/settings.rs:86-101).
- Probing. `Engine::open` runs `<cmd> --help` for the 17 REQUIRED_COMMANDS
  (probe.rs:14), then `version --format json` (identity.rs:79). Nothing
  reads whether the engine is rootless.
- Supervisor use. The events stream runs only while riku containers exist,
  and SIGHUP from a container deploy, restart or scale triggers a listing
  (f3fee94, c69aa1f). Container logs come from one log thread making short
  bounded `logs --since` calls (8a48b95, c205335). An unreachable engine is
  retried every 60 s (crates/riku-supervisor/src/daemon/container_tick.rs:11).
- Paths that bypass the driver: crates/riku-deploy/src/container_runtime.rs
  runs bare `docker` or `podman` (lines 18-31, 47, 66) and
  `ssh <host> "<engine> load"` (lines 128-160) for the laptop side
  `riku container` command (`Placement::Local`,
  crates/riku-args/src/placement.rs:27). See Open questions.

## Engine env

The engine runs in four kinds of process: the supervisor; `git push`
deploys (sshd, forced command, `riku git-receive-pack`, then the hook's
`riku git-hook`, crates/riku-cli/src/setup/git_hook.rs:27-40); `rk`
forwarded commands over the same forced command; and root init's steps run
as the deploy user with `env_clear` and a four name allowlist
(crates/riku-cli/src/setup/deploy_user.rs:30, 114-115).
container_engine.rs:3-4 already notes that git push deploys run without the
operator's environment. So riku computes the engine env itself, and unit
files carry none of it.

- `Engine` gains an env list, built by `open_engine` and applied by every
  spawn above, the probes included.
- `XDG_RUNTIME_DIR=/run/user/<uid>` for the running uid. Open fails naming
  the directory unless it exists, is owned by that uid and has mode 0700.
- `DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus` when that socket
  exists (podman's systemd cgroup manager uses it, unverified).
- `settings.toml` gains `engine.host`, passed as `DOCKER_HOST`, and
  `engine.mode = "rootless" | "rootful"`. An absent key means `rootless`.
  init writes `host` only when the engine fails `info` without it and
  answers with `unix://$XDG_RUNTIME_DIR/docker.sock`. Data, not a name check.
- In rootless mode `open_engine` refuses an engine whose `info` does not
  report rootless. Docker reports `name=rootless` in `SecurityOptions`,
  podman `host.security.rootless` (both unverified). Both shapes are parsed
  as data, as stats.rs does for `stats`. `info` joins REQUIRED_COMMANDS.

Because the env comes from the uid and `settings.toml`, the supervisor, the
git hook and ssh started commands get the same values without any session
setup. The deploy user can edit `settings.toml`; setting rootful gains it
nothing without a rootful socket it cannot reach.

## Root init

Today root init does, in order (crates/riku-cli/src/setup/root_init.rs:99-158):
create the deploy account, stage a root owned temp copy of the binary and
run `riku init --user-steps` from it as the deploy user (which installs
`~/.local/bin/riku`, `~/.riku` and the git hook), read the router back,
check prerequisites, create `/run/riku-sockets` and the `riku-sockets` group
(socket_group.rs), create `/var/lib/riku` with the ACME and static
directories (var_root.rs), set up the router host, and install the systemd
units.

After this work, all checks run before any change, each failure names what
to install or fix, and the order is:

1. Install `/usr/local/bin/riku` (see "One root owned binary"), then run the
   user steps from it. The temp stage goes away.
2. Router, sockets and `/var/lib/riku`, as today. Root writes
   `/var/lib/riku/host.toml` with the detected init system.
3. Router helper, on every host with a router, engine or not, because the
   nginx hole does not depend on containers. It goes before any engine
   step, so no step writes `engine.mode = "rootless"` on a host without it.
4. Engine and init system. Resolve the engine as today. No engine on `PATH`
   and no `CONTAINER_ENGINE` file: stop here, write no `engine.*` key and
   print nothing about engines. A `CONTAINER_ENGINE` that does not resolve
   is still an error. Neither systemd nor OpenRC, OpenRC with no engine but
   docker, or `engine.mode = "rootful"` set by the owner: go to "Rootful
   mode".
5. subuid and subgid: add `<user>:<start>:65536` when the user has no range,
   with a start that overlaps no entry, through `usermod
   --add-subuids/--add-subgids` when present, else an atomic file write.
   Refuse without `newuidmap` and `newgidmap`.
6. User namespaces: run `unshare --user true` as the deploy user.
7. Network: refuse unless `pasta` or `slirp4netns` is on `PATH`.
8. Storage: refuse unless the kernel is 5.13 or newer, or `fuse-overlayfs`
   is present and `/dev/fuse` exists (5.13 is the owner's figure; upstream
   gives 5.11 for docker, unverified).
9. cgroup v2: refuse unless `/sys/fs/cgroup/cgroup.controllers` exists.
10. Session, delegation and services: per init system, below.
11. Write `engine.mode` and `engine.host` as the deploy user through
    `DeployUser`, never by root into that tree (root_init.rs:13-14).
    Refused unless the helper answers `status`.
12. Verify as the deploy user: `open_engine` reports rootless, and a
    container run with `--memory 64m` shows that limit in `stats`.

## Init systems

Detected once, by capability: systemd when `/run/systemd/system` is a
directory, OpenRC when `openrc-run` is on `PATH` and `/run/openrc` is a
directory, else other. The result goes to `/var/lib/riku/host.toml` and
selects a row of data (service templates, enable and start argv), not a
code path per distribution. doctor reads the same file.

What needs systemd today:

| Piece | systemd use | Without systemd today |
|---|---|---|
| supervisor at boot | system unit (system_service.rs:17-70, 142-177) or user unit (user_service.rs:12-97) | none; `--no-systemd` skips it (root_init.rs:145-146) |
| supervisor restart | `Restart=always` (system_service.rs:35) | none |
| nginx reload | path unit on one trigger file plus a reload service with no start limit (router_units.rs:52-89, 6e79b09) | `nginx -s reload` as the deploy user (crates/riku-nginx/src/reload.rs:16), which signals a root master and is expected to fail with EPERM (seen on a real host in the 3a Docker check) |
| confirmed reload | unit run times (crates/riku-deploy/src/router/reload/nginx_unit.rs:1-15) | exit status of the direct reload (crates/riku-deploy/src/router/reload.rs:59-69) |
| Caddy reload | none | `caddy reload` (crates/riku-deploy/src/router/reload.rs:39-57) |
| socket dir after reboot | tmpfiles entry (socket_group.rs:29-31, 95) | none |
| doctor service check | `systemctl` (crates/riku-cli/src/doctor/checks.rs:121-151) | warns "start manually" |

The router helper replaces both nginx rows and the Caddy row on every init
system.

### systemd

- `loginctl enable-linger <user>`, so the user manager, `/run/user/<uid>`
  and the user bus exist without a login.
- `/etc/systemd/system/user@.service.d/riku-delegate.conf` with
  `Delegate=cpu cpuset io memory pids`. Without it only memory and pids are
  delegated and `--cpus` fails (docker rootless docs, unverified).
- Docker: run `dockerd-rootless-setuptool.sh install` as the deploy user
  (shipped with the engine's rootless extras; init does not install it) and
  enable the user `docker.service`. Podman: nothing more.
- Supervisor: `~/.config/systemd/user/riku.service`, written by the deploy
  user step (today's user_service.rs, pointed at `/usr/local/bin/riku`),
  enabled by root with `systemctl --user --machine=<user>@ enable --now
  riku` (needs systemd 248 or newer, unverified). `After=docker.service`
  when docker is the engine.
- The user manager must carry the deploy user's `riku-sockets` membership,
  which root init adds (socket_group.rs:63). A manager started before the
  group change keeps the old groups (unverified); init restarts
  `user@<uid>.service` after adding the group.
- init disables the system `riku.service` when switching to rootless, and
  re-enables it when the mode goes back to rootful.

### OpenRC

Podman only. With docker as the only engine, init goes to "Rootful mode".

Two `openrc-run` scripts, written by root init into `/etc/init.d/`, plus the
helper's (see "Starting the helper"):

- `riku-session`, root, no daemon (`start` and `stop` only):
  - `checkpath -d -m 0700 -o <user>:<group> /run/user/<uid>`. It refuses a
    directory that exists with another owner or mode.
  - `checkpath -d -m 2750 -o <user>:riku-sockets /run/riku-sockets`, the
    job systemd-tmpfiles does on systemd hosts.
  - cgroups: init requires `rc_cgroup_mode="unified"` in `/etc/rc.conf` and
    refuses otherwise, naming the line. `start` creates
    `/sys/fs/cgroup/riku-user`, enables `+cpu +memory +pids +io` in the
    root's `cgroup.subtree_control`, chowns the directory and its
    `cgroup.procs`, `cgroup.subtree_control` and `cgroup.threads` to the
    user, and creates the leaf `riku-user/supervisor`. Podman's containers
    are expected in cgroups it creates under `riku-user` (unverified).
- `riku`, the supervisor. `need riku-session riku-router-helper`, `use` the
  router's service. `supervise-daemon` runs `/usr/local/bin/riku
  supervisor` as the user; `start_post`, as root, writes its pid to
  `riku-user/supervisor/cgroup.procs`.
- Podman needs no daemon. Without a user bus it must use its cgroupfs
  manager (fallback unverified). Init step 12 proves a limit takes effect.
- init runs `rc-update add <script> default` and `rc-service <script>
  start` for each.

### Supervisor unit hardening per mode

- Rootful, system unit: unchanged (`NoNewPrivileges=true`,
  `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp=true`,
  `ReadWritePaths` on `~/.riku` and the four system paths, `MemoryMax=512M`,
  `CPUQuota=50%`; system_service.rs:41-56).
- Rootless, systemd user unit: no `NoNewPrivileges` (podman's `newuidmap`
  is setuid) and no mount sandboxing (in a user manager it needs an extra
  user namespace, unverified; podman writes `~/.local/share/containers` and
  `/run/user/<uid>`). `MemoryMax` and `CPUQuota` stay only if the real host
  check shows containers run outside the unit's cgroup.
- Rootless, OpenRC: no sandbox; limits come from the delegated cgroup only.

## Rootful mode

Set by the owner (`engine.mode = "rootful"`), or by init on OpenRC when
docker is the only engine, or on an unsupported init system. Never set on a
host with no engine. riku sets no rootless env and skips the rootless check.
init and doctor print the grant, keyed by the engine's binary name in the
message only:

- docker: `usermod -aG docker <user>`, then restart the supervisor.
- podman, nerdctl, finch: no grant. Their rootful mode needs root; the
  message says to use docker or a supported init system.

Every printed grant is followed by: "This makes <user> root equivalent:
anything that runs as <user>, including every app, can become root."
doctor repeats the warning, with the reason when init chose the mode: "this
init system is not supported for rootless", or "rootless docker on OpenRC
is not supported yet; install podman to run rootless".

## Ports

Routed copies publish on 127.0.0.1 at a port taken by binding
`127.0.0.1:0` (crates/riku-deploy/src/containers/ports.rs:12-23), above
1024, so rootless binds them. A non routed port keeps its compose host port
(crates/riku-deploy/src/containers/spec.rs:44-57), on loopback or, with
`riku.public: "true"`, all interfaces. `"80:80"` there fails rootless
unless the owner lowers `net.ipv4.ip_unprivileged_port_start`. Deploy
refuses before stopping anything and names the port and the sysctl. init
does not change it.

## Limits and stats

- No container gets `--memory` or `--cpus` today (no such flag in
  crates/riku-container/src/engine/run_args.rs), so rootless breaks no
  current limit. Delegation is for limits added later and for init's
  verification.
- A failing `stats` call is only logged and the old samples kept
  (crates/riku-supervisor/src/containers/listing.rs:83-90), so missing
  delegation shows as stale numbers with no error. doctor reports it.

## Router helper

### The problem today

Root init writes `/etc/nginx/conf.d/riku.conf`, root owned
(crates/riku-cli/src/setup/nginx_host.rs:55-72), holding
`include <riku root>/nginx/*.conf;` (crates/riku-nginx/src/host.rs:25-40)
and, when nothing else is the port 80 default, riku's catch all
(crates/riku-nginx/src/default_server.rs:18). The directory it includes
belongs to the deploy user, and nginx's root master opens every path a
config names, so `access_log /etc/cron.d/x;` in any file there creates a
root owned file with content the requester shapes. Everything that guards
that directory runs as the deploy user and can be skipped by writing the
file directly:

- the write, `nginx -t`, rename sequence (crates/riku-nginx/src/install.rs:1-7),
  whose check accepts any output saying "syntax is ok"
  (crates/riku-nginx/src/validate.rs:66-67);
- the stray `.conf` check (crates/riku-nginx/src/host_check.rs:36);
- the snippet grammar, allowlist and caps, rechecked after filter plugins
  (crates/riku-nginx/src/app_snippet.rs:1-20).

The deploy user also controls when root reloads: it writes the trigger file
that `riku-nginx.path` watches, and `riku-nginx-reload.service` runs
`systemctl reload nginx` with no start limit (router_units.rs:52-89).
TLS keys are read by the master from `<nginx root>/<app>.crt` and `.key`
in the deploy tree (crates/riku-nginx/src/ssl.rs:35-36).

What is already out of the deploy home, read by the nginx worker or the
`caddy` user and never opened as config by root, stays as it is: uWSGI
sockets in `/run/riku-sockets` (42d11f4), published static files in
`/var/lib/riku/static/<app>` and the ACME webroot `/var/lib/riku/acme`
(117b33c, c69aa1f), all deploy user owned with group `riku-sockets`, 2750.

### Design

`riku router-helper` is a root process. The deploy user sends it route data
over a unix socket. It validates the data, renders the config with the
existing `riku-nginx` and `riku-caddy` renderers, installs it into root
owned files under `/etc/riku`, checks it with the real router as root,
reloads, and replies. The deploy user no longer writes any file the router
parses, and no request carries a file path.

Files:
- `/etc/riku` itself: root owned, no group or other write, as caddy_host.rs
  already requires (caddy_host.rs:111-131, nginx_host.rs:96-122).
- `/etc/riku/nginx/<app>.conf`, root owned, directory 0755, files 0644. The
  include becomes `include /etc/riku/nginx/*.conf;` plus the catch all as
  today. `riku-nginx.path`, `riku-nginx-reload.service`, the trigger file
  and the stray conf check go away.
- `/etc/riku/certs/<app>/{fullchain.pem,key.pem}`, root owned, directory
  0700, key 0600.
- `/etc/riku/caddy/<app>.caddy`: the directory exists (c087210) but is
  owned by the deploy user, group `caddy`, 2750. It becomes root owned,
  group `caddy`, 2750, files 0640.
- `/var/cache/riku/nginx/<app>`, the proxy cache. The helper picks it; the
  `RIKU_NGINX_CACHE_PATH` setting (crates/riku-nginx/src/context_cache.rs:44)
  goes away, since the master creates it as root.

Paths the rendered config names are built by the helper from the checked
app name and riku-config constants, never taken from the request: unix
backends `<SOCKET_ROOT>/<app>@<n>.sock` (0dfd7ea), static locations
`<STATIC_ROOT>/<app>/root` and `<STATIC_ROOT>/<app>/paths/<n>`
(crates/riku-nginx/src/static_site.rs:7-11), the ACME location
`ACME_WEBROOT` (crates/riku-config/src/lib.rs:56-77).

### Socket

`/run/riku-router.sock`, owner root, group the deploy user's primary group,
mode 0660. It sits directly in `/run` because systemd creates a missing
parent of a `ListenStream` path as root:root, which the deploy user could
not traverse. The deploy user's primary group holds only the deploy user:
the nginx worker joins `riku-sockets` instead (42d11f4) and `caddy` joins
nothing (c087210). The helper also checks `SO_PEERCRED` and refuses any uid
other than the deploy user's and root.

### Protocol

One request per connection, one connection served at a time; others wait in
the listen backlog, so concurrent deploys queue. The client gives up after
60 s. A request is one JSON object of at most 1 MiB followed by EOF; the
reply is one JSON object, then the helper closes.

Requests:
- `{"v":1,"op":"apply","app":"<name>","route":{...}}`
- `{"v":1,"op":"remove","app":"<name>"}`
- `{"v":1,"op":"status"}` (doctor: version, router, installed apps)

Reply: `{"ok":true,"reloaded":true}` or
`{"ok":false,"stage":"schema|render|check|reload","error":"<text>"}`.
`check` carries the router's own message. On `check` or `reload` failure the
previous files are restored before replying, so a refused config never stays
live. `ok` means the router accepted the reload, which replaces
nginx_unit.rs and the direct reload on every init system.

### Route schema (v1)

It extends `riku_util::RoutePlan` (crates/riku-util/src/route_plan.rs:17-22:
domains, `HttpsPolicy`, backends), which Caddy and router plugins already
take. Unknown fields are refused. Every string is checked with the
directive breaking rule host.rs:20 already uses, plus the per field rules
below.

```json
{
  "domains": ["example.com"],
  "https": "only | also_plain_http",
  "backends": [{"tcp": {"host": "127.0.0.1", "port": 34567}}, {"unix_copy": 1}],
  "listen": {"bind": "0.0.0.0", "port": 80},
  "tls": {"cert_pem": "...", "key_pem": "..."},
  "static_paths": ["/static/"],
  "document_root": true,
  "cache": {"time_s": 3600, "size_gb": 1, "expiry_s": 86400,
            "control": "public", "redirects": false, "any": false},
  "cloudflare_acl": false,
  "allow_git_folders": false,
  "wsgi": false,
  "snippet": "add_header X-Frame-Options DENY;"
}
```

- `domains`: riku-util `Domain` rules. `backends`: TCP hosts loopback only;
  `unix_copy` is a copy number the helper turns into a socket path.
- `tls`: PEM text, parsed by the helper as one certificate chain and one
  key that match, then written by root. The helper never opens a path for
  TLS material. acme.sh keeps running as the deploy user
  (crates/riku-nginx/src/ssl.rs:93-152); the deploy side reads its output
  and sends the text.
- `static_paths[n]` is the location prefix of the n-th published static
  path; `document_root` says whether `<STATIC_ROOT>/<app>/root` is served.
- The fields map the env keys the nginx generator reads now
  (`RIKU_DOMAINS`, `RIKU_HTTPS_ONLY`, `RIKU_BACKENDS`, `RIKU_UPSTREAM_PORT`,
  `RIKU_BIND_ADDRESS`, `RIKU_NGINX_EXTERNAL_PORT`, `RIKU_NGINX_PORTMAP`,
  `RIKU_NGINX_SOCKET`, `RIKU_NGINX_STATIC`, `RIKU_NGINX_STATIC_PATHS`,
  `RIKU_NGINX_DOCUMENT_ROOT`, `RIKU_NGINX_CACHE_*` except the path,
  `RIKU_NGINX_CLOUDFLARE_ACL`, `RIKU_NGINX_ALLOW_GIT_FOLDERS`,
  `RIKU_NGINX_WSGI`, `RIKU_NGINX_INCLUDE_FILE`, `RIKU_ACME_ROOT_CA`; see
  crates/riku-nginx/src/context.rs and template.rs). The builder maps each
  remaining key the generator reads, or drops it with a deploy error naming
  it.

The deploy side builds the route from the app's ENV, runs the same checks
for an early error, and sends it. The helper trusts nothing in it.

### Snippet

The `snippet` field is the final text, after plugin filters. The helper
runs the checks app_snippet.rs runs today itself: the grammar
(crates/riku-nginx/src/snippet_grammar.rs), `ALLOWED_DIRECTIVES`
(app_snippet.rs:40) and the caps (crates/riku-nginx/src/snippet_limits.rs).
The file location check stays on the deploy side, since the helper receives
text, not a path. Caddy has no snippet; a `snippet` for a Caddy host is a
schema error.

### Starting the helper

- systemd: `riku-router-helper.service`, `Type=simple`, running
  `/usr/local/bin/riku router-helper` as root. The helper creates
  `/run/riku-router.sock` itself and stays running; there is no socket unit
  and no idle exit (removed 2026-11-06). `NoNewPrivileges=true` and `ProtectHome=read-only`. A
  `ProtectSystem=strict` sandbox must still let `nginx -t` open its log and
  pid paths, so its `ReadWritePaths` are set against a real nginx.
- OpenRC: `/etc/init.d/riku-router-helper`, `supervise-daemon`, root, before
  `riku`. The helper creates the socket itself with the same owner and mode
  and stays running.
- Other init systems: same binary; the operator starts it. doctor checks
  the socket.

### Caddy

Caddy runs as the `caddy` user. It imports `/etc/riku/caddy/*.caddy`
(crates/riku-caddy/src/host.rs:10-12), and the deploy user reloads it with
`caddy reload` (crates/riku-deploy/src/router/reload.rs:39-57), which posts
to the admin API on localhost with no auth. Any local process that reaches
that port controls every route, the TLS keys and whatever `caddy` can read
or write. That is not root, but it is the front door. The deploy group and
home grants are already gone (c087210).

With the helper:
- the helper, as root, writes the site files (see Files);
- the Caddyfile's global options set `admin unix//run/caddy/admin.sock`
  (syntax unverified), owned by `caddy`, 0600. init refuses, with the lines
  to add, when the Caddyfile already has a global block it cannot merge;
- the helper runs `caddy reload --config /etc/caddy/Caddyfile --address
  unix//run/caddy/admin.sock` as root (flag unverified).

crates/riku-caddy/src/lib.rs:1-5 still says `<riku root>/caddy` and that a
path unit reloads Caddy; both are stale since c087210 and router_units.rs:5.

### Rejected

A root validator that copies riku's nginx text after an allowlist check:
the security would rest on an allowlist over a language nginx keeps
extending. A root inotify watcher: no reply channel, so no confirmed
reload, and it races the deploy user's writes.

## One root owned binary

### Today

The deploy user step of root init installs riku at `~/.local/bin/riku`
(crates/riku-cli/src/setup/binary.rs:13, 47-91, called from
crates/riku-cli/src/setup/init.rs:73) by copy and rename, so a running
supervisor keeps its inode (binary.rs:21). Root runs that step from a root
owned temp copy (root_init.rs:46-65, 81-94). The deploy user owns the
installed file, so anything running as the deploy user, including every
app, can replace the binary the supervisor, the git hook and every forced
command run. That path is written into:

- the system unit `ExecStart` (system_service.rs:34, 151) and the user unit
  (user_service.rs:26, 38);
- every deploy key's forced command (crates/riku-util/src/ssh_keys.rs:45),
  the path from `get_user_install_directory` (deploy_keys.rs:96);
- the git hooks, which hard code `$HOME/.local/bin/riku`
  (git_hook.rs:33, crates/riku-cli/src/apps/create.rs:55) or try it before
  `/usr/local/bin/riku` (crates/riku-cli/src/git/repo.rs:20-25);
- the supervisor relaunch, which prefers `~/.local/bin/riku` after
  `$RIKU_BIN` (crates/riku-deploy/src/supervisor_ctl.rs:36-63);
- the dashboard's remote script (crates/riku-remote/src/remote/dashboard/remote_script.rs:32)
  and `scripts/dashboard-tunnel.sh:23`;
- the shell rc `PATH` lines (binary.rs:93-165), doctor's install check
  (crates/riku-cli/src/doctor/checks.rs:85-119) and init's guidance
  (crates/riku-cli/src/setup/guidance.rs:34).

`scripts/install.sh:71-77` already picks `/usr/local/bin` when run as root.

### Design

- Root init copies the running binary to `/usr/local/bin/riku`, root owned,
  mode 0755, through the same stage and rename (stage file in
  `/usr/local/bin`), then runs the user steps from that path. The temp
  stage in root_init.rs goes away. The user steps install no binary.
  Non root `riku init` requires `/usr/local/bin/riku` and otherwise says to
  run `sudo riku init`.
- Every reference above names `/usr/local/bin/riku`. The supervisor
  relaunch uses `/usr/local/bin/riku`, then `current_exe`; `~/.local/bin`
  is never consulted.
- Init no longer appends a `PATH` line. The forced command, the units and
  the hooks use the absolute path, and `/usr/local/bin` is in the default
  `PATH` of sshd and sudo's `secure_path` on Debian and Alpine (unverified
  elsewhere).
- `rk` never names a server path: it runs commands through the deploy key's
  forced command. The dashboard script uses the absolute path.

## Footprint

The README today (README.md:48-61) states: one Rust binary, no Docker
required; 1 CPU core, 256 MB RAM, about 10 MB disk for riku itself; on a
4 core x86_64 Ubuntu server, idle with no apps, a 9.2 MB stripped binary,
about 10 MB supervisor RSS, about 22 MB supervisor plus nginx (master and 4
workers), about 9.3 MB of disk for the binary and `~/.riku`; one
`docker events` process of about 32 MB only while riku containers exist.

Baseline, measured on that server (3a ledger dir):

| Measure | Value | Source |
|---|---|---|
| stripped `riku`, opt-level "z" | 9,198,584 B | optz-runtime-report.md, c69aa1f |
| supervisor RSS, idle, no apps | 10,228 kB | check-5-and-benchmark-report.md, opt-level 3 build |
| supervisor plus nginx, idle, no apps | 22,272 kB | same |
| binary plus riku state on disk, no apps | about 9.3 MB | README.md:55-56, 86db3cc |

The RSS rows predate the opt-level "z" profile, which measured about
650 kB less supervisor RSS (optz-runtime-report.md). The plan's first task
re-measures all of them at one commit.

Binary limit: the baseline plus 10 percent rounded up to 0.5 MiB, which is
919,859 B rounded up to 1,048,576 B, so 10,247,160 B (Decision 8).

How each part affects it:

- No engine installed: nothing engine related is set up or resident. The
  router helper is the only new piece on such a host.
- Router helper: resident, one more process. Its idle RSS counts toward the
  base.
- One binary on disk, at `/usr/local/bin/riku`, so disk use does not grow.
- Binary size: the helper reuses `riku-nginx`, `riku-caddy`, `serde_json`
  and `nix`, already linked. No new runtime dependency.
- Supervisor: the same process under a user unit or `supervise-daemon`.
- Lingering on systemd: keeps a `systemd --user` manager resident, only on
  hosts with an engine in rootless mode, so it is container add-on cost,
  measured and reported apart from the base.
- The engine: rootless docker runs dockerd, containerd and RootlessKit;
  podman has no daemon. Add-on cost, and the docs say so.

## doctor

On every host: `/usr/local/bin/riku` is root owned and not group or world
writable; no forced command, unit or hook names another path; the router
helper socket answers `status`; no file the router loads is writable by the
deploy user; init system from `host.toml`. With no engine installed, doctor
prints nothing about engines. With an engine:

- Engine name, version and mode. Rootful is a warning with the reason and
  the grant command.
- Supervisor running under the expected unit or script.
- subuid and subgid range, `newuidmap`, userns probe, pasta or slirp4netns,
  overlay or fuse-overlayfs, cgroup v2, delegated memory and cpu.
- `/run/user/<uid>` owner and mode; lingering on systemd.
- `info` reports rootless with the computed env.
- `riku.public` ports below `ip_unprivileged_port_start`.

## Test strategy

Unit tests, no engine:
- env: runtime dir missing, wrong owner, wrong mode; bus present or not;
  `engine.host` passed as `DOCKER_HOST`. The fake engine
  (crates/riku-container/src/engine/fake.rs) records each spawn's env.
- `settings.toml` `mode` and `host`; `info` parsing for both shapes.
- subuid range choice; init system detection from a fake root.
- router helper: schema refusals (unknown field, nginx syntax in a value,
  non loopback TCP host, mismatched cert and key); snippet checks applied to
  filtered text; rendered output equal to today's generator for the same
  input, paths aside; restore on check failure; peer uid refusal.

Real host check, before release, on systemd with docker, systemd with
podman, and OpenRC (Alpine VM) with podman, plus a host with no engine, an
OpenRC host with docker only, and a runit or sysvinit VM:
- init from clean; refusals on cgroup v1, missing network helper, missing
  `newuidmap`.
- OpenRC: `riku-session` recreates `/run/user/<uid>` and
  `/run/riku-sockets` after reboot; the supervisor sits in
  `riku-user/supervisor` and is respawned after `kill -9`.
- docker only OpenRC and runit or sysvinit: init sets rootful, prints the
  grant, the warning and the reason, and runs nothing.
- no engine: init and doctor print nothing about engines; footprint within
  the plan's limits.
- deploy through git push and through `rk` in a session without
  `XDG_RUNTIME_DIR`; uWSGI apps still reach nginx through `riku-sockets`
  under the user manager.
- a supervisor restart leaves containers running; `--memory` shows in
  `stats`; containers are not in the supervisor unit's cgroup.
- `riku.public` port 80 refused with the sysctl named.
- a `.conf` with `access_log /etc/cron.d/x;` written by the deploy user is
  never loaded; a filter adding `access_log` is refused; the Caddy admin
  port on localhost is closed.

## Open questions

For the owner, listed with the plan's recommendation in
docs/superpowers/plans/2026-11-04-rootless-engine.md ("Open questions").
Left to the builder, to confirm on the real host: every point marked
unverified above.
