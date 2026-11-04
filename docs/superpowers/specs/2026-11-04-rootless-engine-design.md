# Rootless container engine

Addendum to docs/superpowers/specs/2026-10-30-container-services-design.md.
That spec says riku drives one Docker compatible CLI and probes capability,
never the engine name. This one says which engine that CLI talks to, how
root setup makes it rootless, and how the router stops being a path to root.
Claims about today's code carry a file:line. Claims about engines, kernels
and init systems come from upstream docs or memory and are marked unverified
until the real host check runs.

## Why

The deploy user is meant to be unprivileged. Two things make it root
equivalent today:

1. A rootful engine. Whoever talks to it can start a container that mounts
   `/`. The deploy user is never added to the docker group (owner decision;
   the group change was dropped at 4294a66).
2. nginx. Its master runs as root and parses every `~/.riku/nginx/*.conf`
   (see "Router helper").

Rootless is pointless without fixing the second, so both ship in the same
release (owner decision).

## Goals

1. The engine runs rootless by default, as the deploy user: container root
   maps to the deploy user's uid, other container uids to its subuid range.
2. Root `riku init` configures it on systemd and OpenRC hosts, for docker
   and podman, and gives every process that runs the engine the same env.
3. Rootful only when the owner sets it, or on a host with another init
   system. doctor flags it.
4. Named volumes of existing compose apps are copied once into the rootless
   store, ownership shifted through the subuid map. Affected apps refuse to
   deploy until then. The rootful volumes stay as a backup.
5. The deploy user never writes a file a root process parses as router
   config.

## Non goals

- No OCI runtime, engine API, containerd or per engine driver. The CLI
  driver in crates/riku-container/src/engine/ stays.
- Installing anything. init configures the engine, uidmap, pasta or
  slirp4netns, fuse-overlayfs already present and names what is missing.
- Changing the KNOWN_ENGINES order (crates/riku-container/src/engine/detect.rs:17).
- Procfile worker isolation. Its cgroup root `/sys/fs/cgroup/riku/apps`
  (crates/riku-supervisor/src/cgroups/mod.rs:16) is created and delegated
  by nothing in init. Separate fix.

## Anti goals

- Adding the deploy user to the docker group, or riku running any grant.
- An app starting on an empty volume while its data sits in the rootful
  store.
- Changing or deleting rootful volumes.
- Quietly running rootful because rootless setup failed. init refuses.
- Branching on the engine's or the distribution's name. Engine binary names
  appear only in messages, as detect.rs:24-25 already requires.

## Decisions (owner, 2026-11-04)

1. Router: the root router helper below (Design 1), shipped with rootless.
   init refuses to set rootless mode unless the helper is installed.
2. Init systems: rootless is supported on systemd and OpenRC, for docker and
   podman, from the first release. Any other init system (runit, s6,
   sysvinit and the rest) runs rootful with a doctor warning saying why.
3. Rootful fallback: init and doctor print the exact grant command for the
   engine, with a plain warning that it makes the deploy user root
   equivalent. riku never runs it.
4. Supervisor: on systemd rootless hosts it runs as a `systemd --user` unit
   under the lingering deploy user's manager. The system unit stays for
   rootful mode. On OpenRC it runs under `supervise-daemon` as the deploy
   user.
5. Defaults, owner may override:
   - root owned state lives in `/var/lib/riku/`;
   - the volume copy uses a helper container with a digest pinned image;
   - the image import in crates/riku-deploy/src/container_runtime.rs moves
     onto the driver;
   - the legacy container plugin path is retired.

## How the engine runs today

- Spawning. Every engine call is `Command::new(program)` with the caller's
  env inherited and nothing added: bounded.rs:116-127, streamed.rs:33-38,
  login.rs:30-35, events.rs:35-45, logs.rs:33-41. No call sets any variable.
- Detection. `<riku root>/CONTAINER_ENGINE`, a name or absolute path
  (crates/riku-config/src/container_engine.rs:16-36), else the first of
  KNOWN_ENGINES on `PATH` (detect.rs:34-75), `PATH` read from the process
  env (crates/riku-deploy/src/containers/engine_open.rs:44). `settings.toml`
  `[engine]` holds timeouts and refuses unknown keys
  (crates/riku-config/src/settings.rs:86-125).
- Probing. `Engine::open` runs `<cmd> --help` for 16 commands
  (probe.rs:14-17, 35-48), then `version --format json` (identity.rs:62-93).
  Nothing reads whether the engine is rootless.
- Paths that bypass the driver: container_runtime.rs runs bare
  `docker`/`podman` locally and `ssh <host> "<engine> load"` remotely
  (container_runtime.rs:17-31, 141-146, 169-192); the legacy plugin picks
  an engine with `which` (crates/riku-plugin-container/src/main.rs:76-90).

## Engine env

The engine runs in four kinds of process: the supervisor; `git push`
deploys (sshd, forced command, then `riku git-hook`,
crates/riku-cli/src/setup/git_hook.rs:27-40); `rk` forwarded commands over
the same forced command; and root init's steps run as the deploy user with
`env_clear` (crates/riku-cli/src/setup/deploy_user.rs:122-136).
container_engine.rs:3-4 already notes that git push deploys run without the
operator's environment. So riku computes the engine env itself, and unit
files carry none of it.

- `Engine` gains an env list, built by `open_engine` and applied by every
  spawn above.
- `XDG_RUNTIME_DIR=/run/user/<uid>` for the running uid. Open fails naming
  the directory unless it exists, is owned by that uid and has mode 0700.
- `DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus` when that socket
  exists (podman's systemd cgroup manager uses it, unverified).
- `settings.toml` gains `engine.host`, passed as `DOCKER_HOST`, and
  `engine.mode = "rootless" | "rootful"`, default rootless. init writes
  `host` only when the engine fails `info` without it and answers with
  `unix://$XDG_RUNTIME_DIR/docker.sock`. Data, not a name check.
- In rootless mode `open_engine` refuses an engine whose `info` does not
  report rootless. Docker reports `name=rootless` in `SecurityOptions`,
  podman `host.security.rootless` (both unverified). Both shapes are parsed
  as data, as stats.rs:114-118 does for `stats`. `info` joins
  REQUIRED_COMMANDS.

Because the env comes from the uid and `settings.toml`, the supervisor, the
git hook and ssh started commands get the same values without any session
setup. The deploy user can edit `settings.toml`; setting rootful gains it
nothing without a rootful socket it cannot reach.

## Root init

All checks run before any change. Each failure names what to install or fix.

1. Engine and init system. Resolve the engine as today; detect the init
   system (below). Neither systemd nor OpenRC, or `engine.mode = "rootful"`
   set by the owner: go to "Rootful mode".
2. subuid and subgid: add `<user>:<start>:65536` when the user has no range,
   with a start that overlaps no entry, through `usermod
   --add-subuids/--add-subgids` when present, else an atomic file write.
   Refuse without `newuidmap` and `newgidmap`.
3. User namespaces: run `unshare --user true` as the deploy user. This
   catches a disabled sysctl or an AppArmor restriction without naming
   either.
4. Network: refuse unless `pasta` or `slirp4netns` is on `PATH`.
5. Storage: refuse unless the kernel is 5.13 or newer, or `fuse-overlayfs`
   is present and `/dev/fuse` exists (the 5.13 figure is the owner's;
   upstream gives 5.11 for docker, unverified).
6. cgroup v2: refuse unless `/sys/fs/cgroup/cgroup.controllers` exists.
7. Session, delegation and services: per init system, below.
8. Write `engine.mode` and `engine.host` as the deploy user through
   `DeployUser`, never by root into that tree (root_init.rs:11-15).
9. Install the router helper (below).
10. Verify as the deploy user: `open_engine` reports rootless, and a
    container run with `--memory 64m` shows that limit in `stats`.
11. Record volumes to move (see "Moving volumes").

## Init systems

Detected once, by capability: systemd when `/run/systemd/system` is a
directory, OpenRC when `openrc-run` is on `PATH` and `/run/openrc` is a
directory, else other. The result goes to `/var/lib/riku/host.toml` and
selects a row of data (service templates, enable and start argv), not a
code path per distribution. doctor reads the same file.

What needs systemd today, and the non-systemd path that exists:

| Piece | systemd use | Without systemd today |
|---|---|---|
| supervisor at boot | system or user unit (system_service.rs:134-170, user_service.rs:12-94) | none; `--no-systemd` skips it (root_init.rs:134-141) and the summary says run `riku supervisor` (guidance.rs:74-78) |
| supervisor restart | `Restart=always` (system_service.rs:37) | none |
| nginx reload | path and reload units (router_units.rs:46-124) | `nginx -s reload` as the deploy user (nginx_unit.rs:53-69, crates/riku-nginx/src/reload.rs:21-34); signals a root master, so expected to fail with EPERM (unverified) |
| confirmed reload (Task 1, 4f9f246) | unit run times (nginx_unit.rs:1-11) | exit status of the direct reload (crates/riku-deploy/src/router/reload.rs:62-64) |
| Caddy reload | none | `caddy reload` (reload.rs:39-57) works |
| doctor service check | `systemctl` (crates/riku-cli/src/doctor/checks.rs:121-147) | warns "start manually" |

The router helper replaces both nginx rows on every init system.

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
  user step, enabled by root with
  `systemctl --user --machine=<user>@ enable --now riku` (needs systemd 248
  or newer, unverified). `After=docker.service` when docker is the engine.
  The supervisor already retries an unreachable engine every 60 s
  (crates/riku-supervisor/src/daemon/container_tick.rs:11).
- init disables the system `riku.service` when switching to rootless, and
  re-enables it when the mode goes back to rootful.

### OpenRC

Three `openrc-run` scripts, written by root init into `/etc/init.d/`:

- `riku-session`, root, no daemon (`start` and `stop` only):
  - `checkpath -d -m 0700 -o <user>:<group> /run/user/<uid>`. It refuses a
    directory that exists with another owner or mode. `/run` is a tmpfs on
    Alpine (unverified elsewhere). If elogind made the directory, the owner
    and mode check is what counts.
  - cgroups: init requires `rc_cgroup_mode="unified"` in `/etc/rc.conf` and
    refuses otherwise, naming the line. `start` creates
    `/sys/fs/cgroup/riku-user`, enables `+cpu +memory +pids +io` in the
    root's `cgroup.subtree_control`, chowns the directory and its
    `cgroup.procs`, `cgroup.subtree_control` and `cgroup.threads` to the
    user (the kernel's delegation rule), and creates leaves
    `riku-user/supervisor` and `riku-user/dockerd`. A cgroup with
    controllers enabled cannot hold processes, hence the leaves.
- `riku-dockerd`, only when docker is the engine. `need riku-session net`.
  `supervise-daemon` runs the absolute path of `dockerd-rootless.sh`
  resolved at init, with `command_user=<user>`, env `HOME`,
  `XDG_RUNTIME_DIR` and a fixed `PATH` that includes the directories of
  `rootlesskit` and the network helper. `respawn_delay=5`,
  `respawn_max=0`. `start_post`, as root, waits up to 30 s for
  `$XDG_RUNTIME_DIR/docker.sock`, then writes the daemon's pid to
  `riku-user/dockerd/cgroup.procs`; root must do the move because it needs
  write access at the common ancestor. Children RootlessKit starts after
  that land in the delegated tree. dockerd without systemd should use the
  cgroupfs driver; if it does not pick it itself, the script passes
  `--exec-opt native.cgroupdriver=cgroupfs` (unverified). `stop` sends
  SIGTERM and waits 30 s.
- `riku`, the supervisor. `need riku-session`, `use riku-dockerd`, and the
  router's service. `supervise-daemon` runs `riku supervisor` as the
  user; `start_post` moves it into `riku-user/supervisor`.
- Podman needs no daemon. Without a user bus it must use its cgroupfs
  manager; podman is expected to fall back to it (unverified). Step 10 of
  init proves a limit takes effect, so a wrong manager fails init.
- init runs `rc-update add <script> default` and `rc-service <script>
  start` for each. ssh sessions get no `XDG_RUNTIME_DIR` without logind,
  which riku's computed env covers.

### Supervisor unit hardening per mode

- Rootful, system unit: unchanged (`NoNewPrivileges=true`,
  `ProtectSystem=strict`, `ProtectHome=read-only`, `ReadWritePaths` on
  `~/.riku`, `MemoryMax=512M`, `CPUQuota=50%`; system_service.rs:43-54).
- Rootless, systemd user unit: `NoNewPrivileges` is dropped, because
  podman's `newuidmap` is setuid. The mount sandboxing lines are dropped:
  in a user manager they need an extra user namespace (unverified) and
  podman must write `~/.local/share/containers` and `/run/user/<uid>`,
  docker's CLI `~/.docker` (login, crates/riku-deploy/src/containers/images.rs:53).
  `MemoryMax` and `CPUQuota` stay only if the real host check shows
  containers run outside the unit's cgroup; if they do not, both are
  dropped, since they would cap every container and a unit restart would
  kill them.
- Rootless, OpenRC: no sandbox; limits come from the delegated cgroup only.

## Rootful mode

Set by the owner (`engine.mode = "rootful"`) or by init on an unsupported
init system. riku sets no rootless env and skips the rootless check. Since
riku never grants access to a rootful socket, the engine stays unreachable
until the owner does. init and doctor print the grant, keyed by the
engine's binary name in the message only:

- docker: `usermod -aG docker <user>`, then restart the supervisor.
- podman, nerdctl, finch: no grant. Their rootful mode needs root, which
  riku never runs as; the message says to use docker or a supported init
  system.

Every printed grant is followed by: "This makes <user> root equivalent:
anything that runs as <user>, including every app, can become root."
doctor repeats the warning, with the init system reason when that applies.

## Ports

Routed copies publish on 127.0.0.1 at a port taken by binding
`127.0.0.1:0` (crates/riku-deploy/src/containers/ports.rs:12-28), written
into the routed binding (crates/riku-deploy/src/containers/spec.rs:44-57)
and rendered as `127.0.0.1:<port>:<container>/tcp`
(crates/riku-container/src/engine/run_args.rs:34-45). The kernel's
ephemeral range is above 1024, so rootless binds them.

A non routed port keeps its compose host port (spec.rs:49-51), on loopback
or, with `riku.public: "true"`, all interfaces
(crates/riku-container/src/types/ports.rs:5-10). `"80:80"` there fails
rootless unless the owner lowers `net.ipv4.ip_unprivileged_port_start`.
Deploy refuses before stopping anything and names the port and the sysctl.
init does not change it.

## Limits and stats

- No container gets `--memory` or `--cpus` today (run_args.rs:104-153), so
  rootless breaks no current limit. Delegation is for limits the container
  spec adds, and for init's verification.
- A failing `stats` call is only logged and the old samples kept
  (crates/riku-supervisor/src/containers/listing.rs:83-91), so missing
  delegation shows as stale numbers with no error. doctor reports it.

## Moving volumes

Today a declared volume `key` is named `<project>_<key>`
(crates/riku-container/src/compose/project.rs:199-210); `name:`,
`external:` and `driver_opts` are refused (compose/volume_decl.rs:34-49).
riku creates no volume itself: `run -v` lets the engine create it
(run_args.rs:57-70). Compose made volumes carry `com.docker.compose.project`,
which the declared name check requires
(crates/riku-deploy/src/containers/project_check.rs:46-93). The legacy
migration reuses those names (crates/riku-deploy/src/containers/migrate.rs:1-9;
4aef6ba).

The copy is a one time root command, `sudo riku migrate-volumes [<app>...]`,
not part of init. It stops the app, takes time in proportion to the data and
must be retryable per app; init is rerun on every upgrade (4294a66) and must
stay quick and never take an app down.

1. init, as root, lists rootful volumes rootless riku will need: named
   volumes mounted by containers labelled `riku.app`, and `<project>_<key>`
   for each compose app's declared keys. It writes them to
   `/var/lib/riku/volume-moves.toml` (root owned, 0644). init refuses to
   switch while any app still runs the legacy plugin
   (crates/riku-cli/src/doctor/container_check.rs:31-46): its
   `docker compose up` worker would start against the rootless engine on
   empty volumes. Those are redeployed first, under rootful.
2. Deploy, restart and the supervisor's restart of a copy refuse for an app
   on the list, naming the command.
3. Per app, the command checks the helper image is present, stops the app's
   rootful containers, and for each volume creates the rootless volume with
   the same name and labels, then streams a tar of the rootful data, read
   by root, keeping container uids, into
   `<engine> run --rm -i --network none -v <vol>:/dst <image> tar -x -p -C /dst`
   run as the deploy user. The engine's own map shifts ownership: container
   uid 0 to the deploy user, `n` to `subuid_start + n - 1`. Files with a uid
   outside the range are refused, not squashed. A rootful daemon with
   `userns-remap` is refused. After a count and size check the app leaves
   the list, and the operator redeploys (images are pulled or built again).
4. Rootful volumes are never touched.

Helper image (default, owner may override): the official `busybox` image,
pinned in riku's source by the digest of its multi architecture index,
chosen when this is built (no digest is named here). The command pulls
`busybox@sha256:<digest>` through the rootless engine. Offline, the pull
fails and the command stops before touching any container; the message says
to `<engine> load` a saved copy of that exact digest and rerun. Presence is
checked with `image inspect` (`image` is already a required command,
probe.rs:14-17).

## Router helper

### The problem

The include `/etc/nginx/conf.d/riku.conf` holds
`include <riku root>/nginx/*.conf;` (crates/riku-nginx/src/host.rs:23-34),
written by root at init (crates/riku-cli/src/setup/nginx_host.rs:151-179).
nginx's root master opens every path a config names, so
`access_log /etc/cron.d/x;` in any file there creates a root owned file
with content the requester shapes. The deploy user can write such a file
directly. Its `nginx -t` check passes any output that says "syntax is ok"
(crates/riku-nginx/src/validate.rs:64-70). `riku-nginx.path` makes root
reload on any change (router_units.rs:54-71), so it also controls when.
Plugin filters rewrite the app snippet after the allowlist check
(crates/riku-nginx/src/app_snippet.rs:129-141), and plugins live in the
deploy user's tree, so the allowlist is bypassable today too.

### Design

`riku router-helper` is a root process. The deploy user sends it route data
over a unix socket. It validates the data, renders the config with the
existing `riku-nginx` and `riku-caddy` renderers, installs it into a root
owned directory, checks it with the real router as root, reloads, and
replies. The deploy user no longer writes any file the router parses.

Files:
- `/etc/riku/nginx/<app>.conf` and `/etc/riku/caddy/<app>.caddy`, root
  owned, directories 0755, files 0644.
- `/etc/riku/certs/<app>/{crt,key}`, root owned, key 0600.
- `/var/cache/riku/nginx/<app>`, the proxy cache. The helper picks it; the
  `RIKU_NGINX_CACHE_PATH` setting goes away (the master creates it as
  root, so its path must never be app chosen).

### Socket

`/run/riku/router.sock`. `/run/riku` is root owned, mode 0750, group the
deploy user's primary group; the socket is 0660 with the same group. The
helper also checks `SO_PEERCRED` and refuses any uid other than the deploy
user's and root. The `caddy` user is removed from the deploy group (see
Caddy), so the group holds only the deploy user.

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
live. `ok` means the router accepted the reload, which replaces the unit
timestamp logic of nginx_unit.rs on every init system.

### Route schema (v1)

Unknown fields are refused. Every string is checked with the directive
breaking rule host.rs:18-20 already uses, plus the per field rules below.

```json
{
  "domains": ["example.com"],
  "https": "only | also_plain_http | none",
  "backends": [{"tcp": {"host": "127.0.0.1", "port": 34567}},
               {"unix": "/home/riku/.riku/sockets/app/web.1.sock"}],
  "listen": {"bind": "0.0.0.0", "port": 80},
  "tls": {"cert_pem": "...", "key_pem": "..."},
  "static": [{"prefix": "/static/", "path": "/home/riku/.riku/apps/app/static"}],
  "document_root": "/home/riku/.riku/apps/app/public",
  "cache": {"time_s": 3600, "size_gb": 1, "expiry_s": 86400,
            "control": "public", "redirects": false, "any": false},
  "cloudflare_acl": false,
  "allow_git_folders": false,
  "wsgi": false,
  "snippet": "add_header X-Frame-Options DENY;"
}
```

- `domains`: riku-util `Domain` rules. `backends`: riku-util `Backend`
  (crates/riku-util/src/backend.rs:9-13); TCP hosts loopback only, unix
  paths inside the deploy user's riku root.
- `tls`: PEM text, parsed by the helper as one certificate chain and one
  key that match, then written by root. Today nginx reads
  `<nginx root>/<app>.crt` and `.key` from the deploy user's tree; the
  helper never opens a path for TLS material.
- `static` and `document_root`: absolute, no `..`, inside the app
  directory. nginx workers read them as their own user, as today.
- The fields map the env keys the nginx generator reads now
  (`RIKU_DOMAINS`, `RIKU_BACKENDS`, `RIKU_BIND_ADDRESS`,
  `RIKU_NGINX_EXTERNAL_PORT`, `RIKU_NGINX_STATIC`, `RIKU_NGINX_DOCUMENT_ROOT`,
  `RIKU_NGINX_CACHE_*` except the path, `RIKU_NGINX_CLOUDFLARE_ACL`,
  `RIKU_NGINX_ALLOW_GIT_FOLDERS`, `RIKU_NGINX_WSGI`). The builder maps each
  remaining key the generator reads, or drops it with a deploy error naming
  it.

The deploy side builds the route from the app's ENV, runs the same checks
for an early error, and sends it. The helper trusts nothing in it.

### Snippet grammar and allowlist

The `snippet` field is the final text, after plugin filters. The helper
runs the 4294a66 checks itself: the grammar in
crates/riku-nginx/src/snippet_grammar.rs (printable ASCII, no backslash, no
braces, strict quoting) and `ALLOWED_DIRECTIVES` (app_snippet.rs:10-14, 33).
The file location check (inside the app directory) stays on the deploy side,
since the helper receives text, not a path. Filters keep working, but can
no longer get past the allowlist. Caddy has no snippet today; a `snippet`
for a Caddy host is a schema error.

### Starting the helper

- systemd: `riku-router-helper.socket` (`ListenStream=/run/riku/router.sock`,
  `SocketMode=0660`, `SocketGroup=<deploy group>`, `DirectoryMode=0750`,
  `Accept=no`) and `riku-router-helper.service` running
  `/usr/local/lib/riku/riku router-helper` as root, root owned binary copied
  by init (root never runs a binary from the deploy user's home,
  root_init.rs:11-15). The service exits after 60 s idle; the socket starts
  it again. `NoNewPrivileges=true` and `ProtectHome=read-only`. A
  `ProtectSystem=strict` sandbox must still let `nginx -t` open its log and
  pid paths, so its `ReadWritePaths` are set by the builder against a real
  nginx.
- OpenRC: `/etc/init.d/riku-router-helper`, `supervise-daemon`, root, before
  `riku`. The helper creates the socket itself with the same owner and
  modes and stays running.
- Other init systems: same binary; the operator starts it. doctor checks
  the socket.

### Caddy

Caddy runs as the `caddy` user (crates/riku-cli/src/setup/caddy_host.rs:3),
imports `<riku root>/caddy/*.caddy` (crates/riku-caddy/src/host.rs:10-12),
and the deploy user reloads it with `caddy reload`, which posts to the admin
API on localhost with no auth (crates/riku-caddy/src/lib.rs:4-5;
reload.rs:39-57). Any local process that reaches that port controls every
route, the TLS keys and whatever the `caddy` user can read or write. That is
not root, but it is the front door.

With the helper:
- the system Caddyfile imports `/etc/riku/caddy/*.caddy`;
- its global options block sets `admin unix//run/caddy/admin.sock` (syntax
  unverified), the socket owned by `caddy`, 0600, so only `caddy` and root
  reach it. init refuses, with the lines to add, when the Caddyfile already
  has a global block it cannot merge;
- the helper runs `caddy reload --config /etc/caddy/Caddyfile --address
  unix//run/caddy/admin.sock` as root (flag unverified);
- `caddy` leaves the deploy group, and the `g+x` grants on the home and riku
  root are undone (caddy_host.rs:82-92).

crates/riku-caddy/src/lib.rs:3-4 says a path unit reloads Caddy, while
router_units.rs:3-7 says Caddy has none; that doc comment is stale.

### Upgrade from the current include

1. `sudo riku init` installs the helper and its units or scripts, then runs
   `riku router republish` as the deploy user, which sends `apply` for every
   app. The helper installs them into `/etc/riku/nginx/` while nginx still
   loads the old include.
2. Only when every app applied, init rewrites
   `/etc/nginx/conf.d/riku.conf` to `include /etc/riku/nginx/*.conf;`,
   checks with `nginx -t` as root and reloads. On failure it restores the
   old include and stops.
3. init disables and removes `riku-nginx.path` and
   `riku-nginx-reload.service` (the removal pattern of router_units.rs:127-149).
4. Deploys refuse with "run: sudo riku init" while the old include is in
   place, as 4294a66 does for missing includes.
5. `~/.riku/nginx/*.conf` and the `.crt`/`.key` there are left; doctor lists
   them as unused.

### Rejected

Design 2, a root validator that copies riku's nginx text after an allowlist
check: the security would rest on an allowlist over a language nginx keeps
extending, while the renderer only ever emits what riku's templates say.
Design 3, a root inotify watcher doing the same: no reply channel, so no
confirmed reload, and it races the deploy user's writes.

## Retired and moved paths (defaults, owner may override)

- container_runtime.rs: local build and save go through `Engine` (`save`
  and `load` join the driver, not REQUIRED_COMMANDS). The remote side runs a
  riku subcommand through the forced command instead of
  `ssh <host> "<engine> load"`, so it gets the engine env.
- The legacy container plugin (crates/riku-plugin-container) is no longer
  shipped or installed. Legacy worker detection and migration in
  crates/riku-deploy/src/containers/ stay for rootful installs that have not
  migrated; init refuses rootless while any remain.

## Upgrade path for rootful installs

1. Install the new riku. Nothing changes until init runs.
2. Redeploy legacy compose apps while still rootful (doctor lists them).
3. `sudo riku init`: checks, router helper and republish, engine setup,
   volume list, `engine.mode = "rootless"`, supervisor moved to the user
   unit or OpenRC script. Apps with no volumes to move come up rootless on
   their next deploy.
4. `sudo riku migrate-volumes`, app by app, then redeploy each.
5. The rootful daemon and its volumes stay until the owner removes them.
   doctor lists leftover rootful riku volumes as a reminder.

## doctor

- Engine name, version and mode. Rootful is a warning with the reason and
  the grant command.
- Init system detected; supervisor and, for docker on OpenRC,
  `riku-dockerd` running.
- subuid and subgid range, `newuidmap`, userns probe, pasta or slirp4netns,
  overlay or fuse-overlayfs, cgroup v2, delegated memory and cpu.
- `/run/user/<uid>` owner and mode; lingering on systemd.
- `info` reports rootless with the computed env.
- Apps blocked on a volume move; leftover rootful riku volumes.
- `riku.public` ports below `ip_unprivileged_port_start`.
- Router helper socket answers `status`; no router config the deploy user
  can write is loaded by the router; leftover files in `~/.riku/nginx`.

## Test strategy

Unit tests, no engine:
- env: runtime dir missing, wrong owner, wrong mode; bus present or not;
  `engine.host` passed as `DOCKER_HOST`. The fake engine
  (crates/riku-container/src/engine/fake.rs) records each spawn's env, so
  every spawn path is covered.
- `settings.toml` `mode` and `host`; `info` parsing for both shapes.
- subuid range choice; uid shift edges (0, 1, last, first outside).
- init system detection from a fake root; template rendering per row.
- volume list from labelled containers and compose keys; deploy refusal for
  a listed app.
- router helper: schema refusals (unknown field, nginx syntax in a value,
  path outside the app, non loopback TCP host, mismatched cert and key);
  snippet grammar and allowlist applied to filtered text; rendered output
  equal to today's generator for the same input; restore on check failure;
  peer uid refusal.

Real host check, must pass before release, each on docker and on podman:
- systemd host: init from clean; refusals on cgroup v1, missing network
  helper, missing `newuidmap`.
- OpenRC host (Alpine VM): `riku-session` recreates `/run/user/<uid>` after
  reboot; `riku-dockerd` starts, is respawned after `kill -9`, and its
  processes sit in `riku-user/dockerd`; the supervisor sits in
  `riku-user/supervisor`.
- deploy through git push and through `rk` in a session without
  `XDG_RUNTIME_DIR`.
- a supervisor restart leaves containers running; `--memory` shows in
  `stats`; containers are not in the supervisor unit's cgroup (decides the
  `MemoryMax` question).
- `migrate-volumes` on a postgres volume: data intact, shifted owners,
  rootful volume unchanged, rerun is a no-op; offline run stops before
  stopping containers.
- `riku.public` port 80 refused with the sysctl named.
- a `.conf` with `access_log /etc/cron.d/x;` written by the deploy user is
  never loaded; a snippet filter adding `access_log` is refused; the Caddy
  admin port on localhost is closed.
- runit or sysvinit VM: init sets rootful, prints the grant and the
  warning, and runs nothing.

## Open questions

None remain from the owner. Left to the builder, to be confirmed on the
real host: every point marked unverified above, in particular the
`info` fields, podman's cgroup manager fallback, dockerd's cgroup driver
without systemd, the user unit sandboxing, and Caddy's admin socket
syntax.
