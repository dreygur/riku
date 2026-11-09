# Developer experience: config files and the first run

Owner approved scope, 2026-11-09. Two projects, one design. The config model
runs first because the first run writes into it.

## Intent

Riku is used mostly by developers, so the first ten minutes decide whether
they stay. The target feel is Video.js: it works on the first line with no
config, config is optional and layered, and extending it needs no fork.

Today a developer goes `install.sh`, `sudo riku init`, `rk server add`, a
hand added git remote, then a push that fails for any plain app because no
runtime plugin is installed (`crates/riku-deploy/src/procfile_deploy.rs:50`).
Riku settings live in env vars mixed with the app's own secrets, and a typo
in one is silently ignored.

## Goals

- A plain Node, Python, Ruby, Go or Rust app goes live from a fresh server
  with no riku specific file in the repo.
- Every riku setting has one home, a schema, and an error naming the key and
  the file when it is wrong.
- An app gets a URL it can open on first push, with or without a domain.

## Non goals

- Traefik support. Separate work later.
- Plugin added commands, router template overrides, wiring the filter bus.
  These change the plugin contract and get their own design.
- Firewall management.

## Anti goals

- Riku installs a router or anything else on the server without the
  operator running a command or answering yes.
- A service name such as sslip.io appears in riku's code.
- An existing Heroku or Piku app with a Procfile stops deploying.

## Before both projects: two small fixes

1. `plugins scaffold --type runtime` writes a bundle directory, but runtime
   discovery only accepts plain files (`crates/riku-plugins/src/runtime.rs:77`),
   so a scaffolded runtime is never detected. Fix discovery to accept a
   runtime bundle. Starts with a failing test.
2. `docs/docs/plugins.md` disagrees with the code: post build failure is
   fatal (`crates/riku-plugins/src/manager.rs:145`), `container` is not a
   bundled plugin, `riku hello`, `riku open` and `riku tunnel` cannot work,
   `riku plugin list` lists client plugins, and gate mode does not veto
   (`crates/riku-plugins/src/events/bus.rs:71`). Make the docs match the code.

## Project 1: config model

### Three kinds of setting

| Kind | Home | Example |
|---|---|---|
| Server wide | `<riku root>/settings.toml` | router, container engine, plugin timeout, fallback domain |
| Per app riku setting | `riku.toml` in the app repo, optional, committed | domains, HTTPS only, runtime version, workers, processes, routing |
| App's own env | `~/.riku/envs/<app>/ENV` | `DATABASE_URL`, secrets |

The plugin protocol env (`RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`,
`RIKU_ROOT` and the rest) does not change. It is the plugin interface.

### settings.toml

Already exists with an `[engine]` section and strict parsing
(`crates/riku-config/src/settings.rs`). It grows into the only place for
server settings. The `<riku root>/ROUTER` file
(`crates/riku-config/src/router.rs:17`), CONTAINER_ENGINE and the other
server wide `RIKU_*` values move in.

`riku init` creates it with these defaults written out:

```toml
[routing]
fallback_domain = "{app}.{ip}.sslip.io"
server_ip = "203.0.113.10"   # detected at init
shared_port = 8080           # for path mode, see below
port_range = "8100-8199"     # for port mode with no number given
```

`fallback_domain` is a pattern. Riku expands `{app}` and `{ip}` and knows
nothing else about it. The operator can point it at nip.io or their own
wildcard domain. Removing the key turns the fallback off; an app with no
domain then gets, on push: no domain set, add `domains` to riku.toml or run
`rk config set`.

### riku.toml

Optional. With no file every value defaults. Read from the pushed commit at
deploy time.

```toml
[routing]
domains = ["shop.example.com"]
https_only = true
mode = "port"        # "domain", "port" or "path"
port = 8081          # port mode only, optional

[runtime]
name = "node"
version = "22"

[processes]
web = "node server.js"
worker = "node worker.js"

[cron]
backup = "0 2 * * * ./scripts/backup.sh"
```

The key names above are a sketch. The builder settles exact names, but the
sections and the rules below are fixed.

It is a trust boundary: it comes from a git push. Unknown keys and bad
values refuse the push with the key and the line. It can never set server
wide values, paths outside the app, or anything that reaches a shell other
than the process commands, which already run as the app today.

### Processes

- `riku.toml` `[processes]` declares processes and cron.
- Procfile is read only when riku.toml declares no processes.
- Both declaring processes refuses the push.
- Neither: the runtime plugin's start command runs as `web`, as today.

### Precedence

Built in default, then `riku.toml`, then a server side override set with
`rk config set`. The override wins so one server can differ without a
commit.

### Migration

`riku doctor --fix` moves the `ROUTER` file and server wide env values into
settings.toml, and moves known `RIKU_*` keys out of each app's `ENV` file
into server side overrides, so nothing changes behavior on the next push.
The owner accepts breaking changes; the migration is still required so
existing servers keep running.

## Project 2: first run

### Router detection

Owner decision: the developer brings the router.

- One of nginx or Caddy installed: init uses it and writes it to
  settings.toml.
- None installed: init fails with the install command for the default
  router. It does not succeed with a warning as today
  (`crates/riku-cli/src/setup/prerequisites.rs:41`).
- More than one: init asks once and stores the answer.

### Runtime plugins on demand

Owner decision: nothing installed up front, ask before installing.

- `riku init` installs no runtime plugins.
- Core holds a marker table for the five bundled runtimes only:
  `package.json` node, `requirements.txt` or `pyproject.toml` python,
  `Gemfile` ruby, `go.mod` go, `Cargo.toml` rust-lang. The table only picks
  what to fetch; the plugin's own `detect` still decides.
- `rk deploy` checks the markers on the laptop, asks the server which
  plugins it has, and when one is missing asks: node plugin needed on prod,
  install? [y/N]. Yes installs then pushes; no stops.
- Plain `git push` cannot prompt (the hook has no terminal), so it stops
  with: app needs the node plugin. Install it: `rk plugins install node`,
  then push again.
- Install uses the same source and checksum check as `riku install-plugins`.
  An offline server gets a clear error with the manual command.

### URL on first push

- Domain mode: the app's `domains`, else `fallback_domain`.
- Port mode (default when the developer picks IP only): the router listens
  on the app's port and proxies to it. Every app works unchanged.
- Path mode: the router listens on `shared_port` and routes `/<app>/` to the
  app, sending `X-Forwarded-Prefix`. The push output warns that the app must
  handle a base path.
- Port and domain can be set together; the app answers on both.
- Fallback domain TLS (owner decision): try HTTPS on every router, and when
  the certificate cannot be issued serve HTTP and say so in the push output.
  `https_only` applies to the app's own domains only, so a failed fallback
  certificate never takes the app offline. On nginx this goes through
  `crates/riku-nginx/src/ssl.rs`, which has known open problems (ROADMAP,
  "Known open problems"); the plan must account for them.

Confirmed 2026-11-09 against nginx 1.27 in Docker: one listen port proxies to
two services by path, and by `Host` header. Caddy also routes by path.

Port rules, enforced at push: a port used by another app is refused, naming
that app; ports below 1024 except 80 and 443 are refused; the router's admin
port (Caddy 2019) and the ssh port are refused. A bare IP gets HTTP only, and
the push output says so. The push output names the port to open in the
firewall.

These replace the nginx only `RIKU_NGINX_PORTMAP` and
`RIKU_NGINX_EXTERNAL_PORT` (`crates/riku-nginx/src/backends.rs:94`); routing
config stays router neutral and each router maps it.

### Laptop setup

- `rk server add NAME user@host` also copies the ssh key. Today it only
  saves the entry (`crates/riku-remote/src/remote/server_cmd.rs:76`).
- Run inside an app directory, rk adds the git remote.
- After that the developer only pushes.

### Guidance text

- The init guide stops saying a Procfile is required
  (`crates/riku-cli/src/setup/guidance.rs:94`).
- No `your-server` style placeholders where riku knows the real value
  (`guidance.rs:84`, `crates/riku-remote/src/quickstart/mod.rs:102`,
  `scripts/install.sh:131`).
- `install.sh` stops pointing at `riku quickstart` on the server.

## Constraints

- Project rules in `.claude/CLAUDE.md`: no `?`, typed errors, small files,
  tests per behavior, no python for file edits.
- The owner's server, organisation and project names are confidential.
  Neutral names only (`prod`, `edge.example`, `myapp`); search every diff
  before pushing.
- Breaking changes are allowed; the doctor migration is not optional.

## Left to the builder

- Exact key names in riku.toml and settings.toml.
- How the server IP is detected at init.
- How init asks when stdin is not a terminal and more than one router is
  installed.
- What `rk deploy` does today and how much of it changes.

## Calls made by Claude, not the owner

- Precedence order (default, riku.toml, server override).
- `port_range` assignment when port mode gives no number.
- Default `shared_port` 8080.
- Marker table limited to the five bundled runtimes.
- Server side overrides are per app, in `~/.riku/envs/<app>/riku.toml`,
  same schema as the repo file. Set with a new `riku settings set APP
  key=value` (forwarded by rk). `riku config set` (`crates/riku-args/src/cmds.rs:58`)
  keeps writing only the app's own env.
- The keys doctor moves out of `ENV` are exactly the per app `RIKU_*` keys
  riku reads today; the builder lists them from the code and the plan
  carries the list.
- Port mode: the developer's `port` (or one from `port_range`) is the
  external port the router listens on. The internal `PORT` stays auto
  assigned as today. Conflicts are checked on external ports.
- Path mode: one router config for `shared_port`, regenerated from every
  path mode app whenever one is deployed or destroyed. Per app files stay
  for domain and port mode.
- `rk plugins install node` runs the existing runtime install
  (`riku install-plugins --plugins node`) on the server over ssh, as rk
  forwards other commands. A push never installs anything.
- `[runtime] name` in riku.toml acts like `RIKU_RUNTIME`: markers are
  skipped and the missing plugin message names that runtime.
- `riku init` only creates settings.toml when missing. The router check is
  a hard failure only on a fresh install; existing servers move over
  through `riku doctor --fix`, never through a re-run of init.
