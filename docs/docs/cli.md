# CLI Reference

Complete reference for all Riku commands.

## Usage

```bash
riku <command> [subcommand] [options]
```

`riku --help` groups the commands by where they run:

```
Local commands:
  container, server, quickstart, plugin

Server commands:
  init, dashboard, plugin-emit, supervisor

Forwarded commands (run on the selected server, or here with --local):
  apps, agent, config, deploy, destroy, logs, ps, stats, run, restart, stop,
  rollback, backup, restore, doctor, addon, update, install-plugins,
  plugins, hook
```

When a server is implied (by the repo or the default), the forwarded heading
names it: `Forwarded commands (run on edge, or here with --local):`.

## Remote Servers

The local `riku` binary can manage any number of servers over ssh. It runs the
system `ssh` and `git`; there is no ssh library inside riku. A forwarded
command such as `riku ps myapp` runs on the selected server, streams its output
back, and exits with the exit code of the remote command.

### `riku server`

The server list is local to your machine and always read here, never
forwarded.

```bash
riku server add prod riku@prod.example
riku server add edge riku@edge.example --admin root@edge.example
riku server list
riku server default edge
riku server remove edge
```

| Subcommand | Description |
|------------|-------------|
| `add <name> <user@host> [--admin <user@host>] [--key PATH]` | Register a server, authorize your public key on it and add the repo's `riku` remote (see below). A name that already exists is an error |
| `list` | Show name, address and admin login. The default is marked with `*` |
| `remove <name>` | Forget a server. Removing the default leaves no default |
| `default <name>` | Make a registered server the default |

The first server you add is not made the default. Run `riku server default
<name>` to pick one.

After saving the entry, `add` authorizes your public key for pushes: `--key
PATH`, else `~/.ssh/id_ed25519.pub`, else `~/.ssh/id_rsa.pub`. With `--admin`
it runs `ssh -t <admin> sudo riku deploy-key add --user <user> '<key>'` (the user from `<user@host>`), so sudo can ask for
the password; without one it prints that command to run on the server. Only
the key type and data are sent. Run inside a git repo, `add` also adds a
`riku` remote pointing at `<user@host>:<repo directory name>`, unless the repo
already has a `riku` remote or one pointing at that server. A failed key step
keeps the entry and prints the command to run by hand.

A name is 1 to 32 characters of `a-z`, `0-9`, `_` or `-`, starting with a
letter or digit. An address is `user@host`: the host is a hostname, an IPv4
address, or a bracketed IPv6 literal. There is no port syntax. For a server
on another port, define a `Host` alias in `~/.ssh/config` and use the alias as
the host (`riku@edge-alt`).

`--admin` is a second login on the same host with sudo rights. Only
`riku dashboard` uses it.

The list is stored in `servers.toml` under `$XDG_CONFIG_HOME/riku/`, or
`~/.config/riku/` when `XDG_CONFIG_HOME` is unset or not an absolute path
(also on macOS). The
directory is mode 700 and the file mode 600. A hand edited entry is checked
with the same rules as one typed on the command line.

### Choosing the server

For a forwarded command the first rule that applies wins:

1. `-s <name>` or `--server <name>` on the command line, for that run only.
2. The current git repo. Riku reads `git remote -v` and compares the
   `user@host` of each scp style remote (`riku@edge.example:myapp`)
   exactly against the registered addresses. Two remotes that match
   different servers are an error; choose with `-s`.
3. The default server.
4. No server: the command runs on this machine, as it did before.

`--local` skips all of this and runs here. Outside a git repo, or without
`git` installed, rule 2 finds nothing and selection moves on.

The match is a plain string comparison. There is no DNS lookup and no ssh
config alias resolution, so `riku@edge.example` and `riku@192.168.1.20` are
different servers even when they are the same machine. A remote written as an
`ssh://` URL is not matched either. Register the address your remote uses.

Before every forwarded command riku prints the target on stderr, so piped
output stays clean:

```
[*] edge (riku@edge.example)
```

There is no confirmation prompt.

### What runs where

| Where | Commands |
|-------|----------|
| Local, always | `server`, `quickstart`, `container`, `plugin` |
| Server only | `init`, `deploy-key`, `supervisor`, `plugin-emit`, and the hidden git and internal commands |
| Forwarded | `apps`, `ps`, `logs`, `stats`, `config`, `deploy`, `restart`, `stop`, `destroy`, `rollback`, `run`, `backup`, `restore`, `addon`, `doctor`, `update`, `agent`, `install-plugins`, `plugins`, `hook` |

Giving `-s` to a server only command is refused: `use --local to run it here`.
`dashboard` is the exception, see [`riku dashboard`](#riku-dashboard). A client
plugin that overrides a command still runs first, and it receives the selected
server's address as `$1`.

`run` and `logs --follow` allocate a tty on the server, so interactive
programs and Ctrl-C behave. Everything else runs without one.

Path arguments of a forwarded command resolve on the server, not on your
machine: `deploy --from`, `backup --out` and `restore <app> <file>` all name
server paths. Add `--local` to use a path on this machine.

The plugin authoring commands `plugins keygen`, `plugins sign` and
`plugins scaffold` always run locally, so a private signing key is never
created on a server. Every other `plugins` subcommand is forwarded.

### App name from the repo

When the server was picked because of a git remote, a command that takes an
app name can leave it out and use the remote's path. With the remote
`riku@edge.example:myapp`, these both act on `myapp`:

```bash
riku restart
riku config show
```

This works for `deploy`, `logs`, `restart`, `stop`, `rollback`, `backup`,
`config show`, `config live`, `stats app`, `addon bind` and `addon unbind`. A
typed app name always wins. If the repo has remotes on that server naming
different apps, no name is taken.

The app is never inferred for `destroy`, `run`, `restore`, `config get`,
`config set` and `config unset`: type it every time. `ps` without an app lists
every app, so it does not take the name either.

`riku destroy <app>` does not ask for confirmation. It deletes the app
immediately, on whichever server is selected. Check the `[*]` line.

### Quoting and the ssh command

Forwarded arguments are shell quoted, so `riku run myapp echo "a b"` passes
`a b` as one argument. The deploy key on the server no longer lets the login
shell re-split the command; riku splits it itself with shell quoting rules and
expands nothing (no globs, no variables). Plain `ssh` still works, and a
leading `riku` word is accepted:

```bash
ssh riku@edge.example ps myapp
ssh riku@edge.example riku ps myapp
```

## The `rk` Client

`rk` runs riku commands on your servers from your laptop. Start with:

```bash
rk server add edge riku@edge.example
rk -s edge apps
```

`rk` is a separate, smaller binary. It parses the same commands as `riku`, but
it never runs server code: every server command is sent over ssh to the
`riku` installed on the server. It runs only these commands itself: `server`,
`plugin` (client plugins), `quickstart`, `help`, and the remote form of
`dashboard`, which tunnels to the dashboard of the chosen server. `dashboard`
picks its server like every other command: `-s`, then the git remote of the
current repo, then the default server.

Install it with the install script, which fetches `rk-<os>-<arch>.tar.gz` from
the release and checks its `.sha256`:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh -s -- --rk
```

The server is chosen as in [Choosing the server](#choosing-the-server), minus
the last rule: `rk` has no `--local` and never falls back to this machine. When
no server is selected it stops with the first line below when the server list
is empty, or the second, which names the registered servers:

```
no server selected; add one with rk server add <name> <user@host>, or pass -s <name> (none are registered)
no server selected; add one with rk server add <name> <user@host>, or pass -s <name> (registered: edge, prod)
```

Commands that only make sense on the server (`init`, `supervisor` and the
hidden internal commands) are refused with `run it with riku on the server`.
`container` is refused with `run it with the full riku binary`.

`plugins keygen`, `plugins sign` and `plugins scaffold` are plugin author
commands and run on your machine in `rk`, exactly as in `riku`, whether or not
a server is selected. They never reach a server. The hint that tells you to
trust a new key on a server names `rk -s <server> plugins trust add`.

The server list is the same file `riku server` uses,
`${XDG_CONFIG_HOME:-$HOME/.config}/riku/servers.toml`, so both binaries share
it. Client plugins live in
`~/.riku/client-plugins`.

`riku` itself also forwards: with `-s`, a repo remote or a default server it
forwards over ssh, and without one it runs the command on the current machine.

## Application Management

### `riku apps`

List all deployed applications.

```bash
riku apps
```

**Output:**
```
myapp      web=2 worker=1    running
test-app   web=1             stopped
```

---

### `riku deploy <app>`

Force redeploy an application.

```bash
riku deploy myapp
```

This resets the working directory, detects the runtime, installs dependencies, and spawns workers.

---

### `riku destroy <app>`

Permanently remove an application and all its data. There is no confirmation
prompt, and the app name is never taken from the git remote: type it every
time. With a server selected, the app is removed from that server.

```bash
riku destroy myapp
```

This deletes:
- Application code in `~/.riku/apps/<app>/`
- Environment variables in `~/.riku/envs/<app>/`
- Git repository in `~/.riku/repos/<app>/`
- Logs in `~/.riku/logs/<app>/`
- Nginx configuration
- Worker configurations

`~/.riku/data/<app>/` and `~/.riku/cache/<app>/` are **preserved** by destroy.

For a container app, destroy first stops and removes every container and the
app network, and keeps the volumes. When the container engine does not
answer, destroy refuses and removes nothing, so no container is left running
without an app. `--force` removes riku's files and route anyway and warns
that containers may still be running:

```bash
riku destroy myapp --force
```

---

### `riku logs <app> [process]`

Tail application logs.

```bash
# All logs
riku logs myapp

# Specific process
riku logs myapp web
riku logs myapp worker
```

Logs are stored in `~/.riku/logs/<app>/` and automatically rotated at 10MB.

The supervisor writes each line behind the time it captured it, in UTC to
the second (`2026-11-12T08:14:03Z listening on :37139`); a container line
carries the engine's time for it. `riku logs` shows that time in the
server's local time before the worker name:

```
14:14:03 web.1 | listening on :37139
```

Without a process, every `*.log` file of the app is followed except
`deploy.log` (use `--deploy` for that one). With a process or service name,
its `<name>.<n>.log` files are followed, one per copy. A name with no log
files is an error that lists the names that have logs. Container services
log to the same files, written by the supervisor.

---

### `riku restart <app> [process]`

Restart the running processes of an application from the current build.
Nothing is built. To rebuild, or to apply a change to the Procfile, the
domains or other routing settings, run `riku deploy <app>`.

```bash
riku restart myapp
riku restart myapp worker   # one process kind, or one container service
```

A Procfile app's worker configs are written again from the app's current
ENV and riku settings, the env a deploy would give them (the runtime
plugin's `env` output included), so a value edited in ENV takes effect.
The plugin's `env` output is the one the last deploy stored; when ENV or
the server settings changed since, restart runs the plugin's `env` again
with the current env and stores that instead.
No build, release or preflight runs and the checkout is left as it is. Each
kind keeps its command and its number of copies. Web facing kinds are
replaced one copy at a time as on a deploy: a new copy starts, passes its
health check and joins the router before an old one stops. Other kinds are
stopped and started with the new config. An app with no running workers
(never deployed, or stopped) is refused with a pointer to `riku deploy`. Naming a
process kind (`web`, `worker`, ...) restarts only that kind's workers; a
kind the app does not run is refused with the list of kinds it does run.
`LIVE_ENV` (`riku config live`) is written only by a restart that covers
every running kind; a restart of one kind leaves it as it was and says so,
since the other kinds still run the old env.

A container app replaces the copies of the named service, or of every
service in start order, one at a time from the images they run now, without
a rebuild. A stopped service starts again. See
[Rolling replacement](runtimes.md#rolling-replacement).

`--hot` is the same as a plain restart, kept for scripts, and cannot be
combined with a service.

---

### `riku stop <app> [service]`

Stop all processes for an application.

```bash
riku stop myapp
riku stop myapp worker   # one service of a container app
```

To start a stopped Procfile app again, use `riku deploy <app>` or push a new
commit. A stopped container service starts again with `riku restart <app>`.

For a container app, stop removes the copies of the named service, or every
container of the app, and keeps the volumes. When the routed service is
among them, the app is taken off the router first. Dependents stop before
the services they depend on. Naming a service of a Procfile app is refused.

---

### `riku run <app> <command>`

Execute a command in the application's environment.

```bash
riku run myapp python manage.py migrate
riku run myapp nub run build
riku run myapp bash
```

Environment variables from `~/.riku/envs/<app>/ENV` are loaded automatically.

---

## Configuration Management

### `riku config show <app>`

Show application configuration.

```bash
riku config show myapp
```

**Output:**
```
DATABASE_URL=postgres://localhost/mydb
RIKU_DOMAINS=example.com
RIKU_WORKER_TIMEOUT=3600
```

---

### `riku config get <app> <key>`

Get a specific configuration value.

```bash
riku config get myapp DATABASE_URL
```

---

### `riku config set <app> KEY=value...`

Set configuration values.

```bash
riku config set myapp DATABASE_URL=postgres://localhost/mydb
riku config set myapp KEY1=value1 KEY2=value2
```

Multiple key-value pairs can be set in one command.

---

### `riku config unset <app> KEY...`

Remove configuration values.

```bash
riku config unset myapp DATABASE_URL
riku config unset myapp KEY1 KEY2
```

---

### `riku config live <app>`

Show live running configuration (from worker TOML files).

```bash
riku config live myapp
```

Shows the actual configuration being used by the supervisor, including defaults.

---

---

## Process Management

### `riku ps <app>`

Show process counts and status.

```bash
riku ps myapp
```

**Output:**
```
web:     2/2 running
worker:  1/1 running
cron:    1/1 running
```

For a container app, each copy is a row with the columns SERVICE
(`<service>.<n>`), STATUS, MEMORY, CPU, RESTARTS, CONTAINER ID and IMAGE.

---

### `riku ps <app> --scale web=N worker=N...`

Scale worker processes.

```bash
riku ps myapp --scale web=4 worker=2
```

This creates/updates the `SCALING` file and triggers a restart.

For a container app, each named compose service grows or shrinks without a
rebuild and without restarting the copies it keeps, and the count is
recorded in `~/.riku/envs/<app>/SCALING`. A service that mounts writable
storage, or publishes a port other than the routed HTTP one, cannot run more
than one copy, and a count of 0 is refused. See
[Scaling](runtimes.md#scaling).

**Alternative:** Create a `SCALING` file directly:
```bash
echo "web=4" > SCALING
echo "worker=2" >> SCALING
git add SCALING && git commit -m "scale up"
git push riku main
```

---

### `riku stats all`

Per-app process counts, health, and memory across every deployed app.

```bash
riku stats all
```

**Output:**
```
App: myapp
  Processes:  3/3 running
  Healthy:    3/3
  Memory:     128.40 MB
```

The numbers come from `~/.riku/stats.json`, which the supervisor writes. With
the supervisor stopped, the command falls back to listing each app and its
worker count and says so.

---

### `riku stats app <app>`

Per-process detail for one app.

```bash
riku stats app myapp
```

**Output:**
```
PROCESS                   KIND       PID        STATUS       HEALTH
---------------------------------------------------------------------------
myapp.web.0               web        41288      running      healthy
myapp.worker.0            worker     41290      running      healthy
```

---

## Releases & Resilience

### `riku rollback <app>`

Roll an app back to a previous release. See [Rollback](rollback.md).

```bash
riku rollback myapp            # previous release
riku rollback myapp --to <sha> # a specific commit SHA
riku rollback myapp --list     # list release history instead
```

| Option | Description |
|--------|-------------|
| `--to <TO>` | Roll back to a specific commit SHA (default: previous release) |
| `--list` | List the release history instead of rolling back |

---

### `riku backup <app>`

Back up an app (source + env + volumes + repo) to a `.tar.gz`. See
[Backup & Restore](backup-restore.md).

```bash
riku backup myapp
riku backup myapp --out /backups/myapp.tar.gz
```

| Option | Description |
|--------|-------------|
| `--out <OUT>` | Output path (default: `./<app>-backup-<timestamp>.tar.gz`) |

---

### `riku restore <app> <file>`

Restore an app from a backup tar.gz.

```bash
riku restore myapp ./myapp-backup-20260624.tar.gz
```

---

## Addons

Manage addon instances (managed databases, caches, …). See [Addons](addons.md).

```bash
riku addon list                    # List provisioned instances
riku addon create <PLUGIN> <NAME>  # Provision a new instance
riku addon bind <INSTANCE> <APP>   # Bind an instance to an app
riku addon unbind <INSTANCE> <APP> # Unbind an instance from an app
riku addon destroy <INSTANCE>      # Destroy an instance (unbind first)
riku addon backup <INSTANCE>       # Back up an instance
```

---

## Plugins & Marketplace

Install/list/remove manifest-based plugin bundles and manage marketplaces. See
[Marketplace](marketplace.md).

```bash
riku plugins search <query>        # Search registered marketplaces
riku plugins add <name[@market]>   # Install a plugin by name
riku plugins install <name>        # Install a shipped runtime (node, python, ruby, go, rust-lang, java, clojure)
riku plugins install <path|url>    # Install from a local path (./name) or git URL
riku plugins list                  # List installed bundles
riku plugins remove <name>         # Remove an installed bundle
riku plugins doctor                # Validate API compatibility + integrity
riku plugins marketplace add <src> # Register and clone a marketplace
riku plugins trust list            # Manage trusted publisher keys
```

Three commands sit next to each other here and manage different things:
`plugins` (above) handles manifest-based bundles, `hook` handles the
server-side lifecycle scripts, and `plugin` handles client-side CLI overrides.

---

### `riku hook <command>`

Inspect the server-side lifecycle hook plugins in `~/.riku/plugins/` (the
executables named `riku-pre-deploy`, `riku-post-deploy`, and so on). See
[Plugins](plugins.md) for what each hook does and when it runs.

```bash
riku hook list                     # List executable hook plugins
riku hook check riku-pre-deploy    # Check one exists and is executable
```

Both are read-only: installing a hook means dropping an executable into
`~/.riku/plugins/` yourself, or letting `riku install-plugins` do it.

---

### `riku plugin <command>`

Inspect client-side plugins: executables in `~/.riku/client-plugins/` named
after a riku command, which run **instead of** that command on your machine.

```bash
riku plugin list                   # List installed client plugins
riku plugin exists deploy          # Check one exists and is executable
```

Only some commands can be overridden this way: `apps`, `config`, `deploy`,
`destroy`, `logs`, `ps`, `stats`, `run`, `restart`, `stop`, and `container`.
Everything else (`init`, `backup`, `restore`, `doctor`, `rollback`, the plugin
commands themselves) always runs riku's own implementation, so a plugin cannot
shadow them.

---

## Setup Commands

### `riku quickstart [name]`

Scaffold a sample app locally and print deploy instructions. Runs on your
machine, not the server.

```bash
riku quickstart
riku quickstart myapp --runtime node
riku quickstart myapp --remote riku@example.com
```

| Option | Description |
|--------|-------------|
| `--runtime <RUNTIME>` | Runtime to scaffold: `python` or `node` (default: `python`) |
| `--remote <REMOTE>` | Deploy target for the git remote line, e.g. `riku@edge.example`. Default: the default registered server; with none, the steps say `rk server add` |

---

### `riku init`

Initialize the Riku directory structure and install the binary.

```bash
riku init
```

Creates:
- `~/.riku/apps/` - Application code
- `~/.riku/repos/` - Git bare repositories
- `~/.riku/envs/` - Environment variables
- `~/.riku/logs/` - Application logs
- `~/.riku/nginx/` - Certificate files the router helper copies
- `~/.riku/cache/` - Cache files
- `~/.riku/workers-available/` - Worker configs
- `~/.riku/workers-enabled/` - Enabled worker symlinks
- `~/.riku/plugins/` - Plugin executables
- `~/.riku/acme/` - SSL certificates

As root, also installs the riku binary to `/usr/local/bin/riku` (root owned) and generates the systemd service.

The router comes from `RIKU_ROUTER` when set, else the one already stored.
A fresh install uses the only router installed (nginx or Caddy), asks when
both are and init has a terminal, and otherwise stops with what to do. An
existing install without a stored router keeps nginx when nginx is installed.

---

### `riku deploy-key add [--user NAME] '<key>'`

Authorize one public key for `git push`. Root only, on the server.

```bash
sudo riku deploy-key add 'ssh-ed25519 AAAA... me@laptop'
```

`--user` names the deploy account (default: `RIKU_USER`, else `riku`; sudo drops `RIKU_USER`, so pass `--user` for a custom account). The account must have a `~/.riku` directory. Takes exactly one key line; a second line is refused. Options before the key
type and the comment after it are dropped. The deploy user writes the key to
its own `authorized_keys`, forced through riku, and a key already there is
skipped. `rk server add --admin` runs this for you.

---

## System Commands

### `riku supervisor`

Start the process supervisor daemon.

```bash
riku supervisor
```

The supervisor:
- Monitors worker TOML configurations
- Spawns and manages application processes
- Handles graceful shutdowns
- Performs health checks
- Auto-restarts failed processes

**Run as a systemd user service:**
```bash
systemctl --user enable riku
systemctl --user start riku
```

---

### `riku router-renew`

Send an app's current certificate files to the router helper. acme.sh calls it after each renewal. Run it as the deploy user.

```bash
riku router-renew myapp
```

The server side `riku router-helper` command is the root service behind `riku-router-helper.service`. You do not run it by hand. See [Systemd](systemd.md).

---

### `riku doctor`

Diagnose the installation (deps, dirs, systemd, nginx, disk, SSH). See
[Doctor](doctor.md).

```bash
riku doctor
sudo riku doctor   # most complete report
```

---

### `riku dashboard`

Serve the embedded web dashboard (single binary, no separate install). It can
deploy, roll back, and manage addons, not read-only, so always set a token
before exposing it beyond loopback. See [Dashboard](dashboard.md), which also
covers the separate, fuller Next.js browser dashboard.

```bash
riku dashboard
riku dashboard --bind 0.0.0.0:8088 --token <tok>
riku dashboard -s edge
```

| Option | Description |
|--------|-------------|
| `--bind <BIND>` | Address to bind (`host:port`). Default: `127.0.0.1:8088` |
| `--token <TOKEN>` | Require this token on the API (also via `RIKU_DASHBOARD_TOKEN`) |

With no server selected, the dashboard is served on this machine as above.
With one selected (`-s`, the repo's git remote, or the default), it opens that
server's dashboard in your local browser in one step:

1. The server entry needs an admin login. Without one riku stops and prints
   the command to fix it: `riku server add edge riku@edge.example
   --admin root@edge.example`. `--bind` and `--token` are refused, since
   they only apply to a dashboard served here.
2. Riku opens one ssh connection as the admin and runs the dashboard as the
   deploy user (the user part of `address`) through `sudo`. The token is new
   for each session, is never written to a file and never appears in a
   process argument list on the server.
3. If sudo asks for a password, riku asks for it on your terminal with echo
   off, once. A wrong password ends the session; it is not retried. A
   `NOPASSWD` rule or cached sudo credentials skip the prompt.
4. Riku forwards local port 8088 to the dashboard, or a free port when 8088
   is taken, prints the URL and opens the browser.

Press Ctrl-C to end the session. This stops the remote dashboard and the ssh
connection and leaves other people's sessions on the same server alone. Each
session has its own dashboard process and token.

The token is part of the URL, so it stays in your browser history and is
visible in the arguments of the `open` or `xdg-open` process for the moment it
starts. Both stay on your machine and the token stops working when the session
ends. This is accepted.

The `riku` binary must be at `/usr/local/bin/riku` on the server.

---

### `riku --help`

Show help information.

```bash
riku --help
riku <command> --help
```

---

### `riku --version`

Show version information.

```bash
riku --version
```

---

## Git Integration

### Post-receive Hook

When you `git push riku main`, the post-receive hook:
1. Checks out code to `~/.riku/apps/<app>/`
2. Detects the runtime
3. Installs dependencies
4. Generates nginx config
5. Creates worker configs
6. Notifies the supervisor

No manual intervention required.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | App not found |
| 4 | Configuration error |
| 5 | Permission denied |

---

## Environment Variables (CLI)

These affect CLI behavior:

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Set log level: `debug`, `info`, `warn`, `error` |
| `RIKU_ROOT` | Override default `~/.riku` location. Must be an absolute path without `..`; riku exits with an error otherwise. The same rule applies to `HOME` |

**Example:**
```bash
RUST_LOG=debug riku deploy myapp
```

---

## Examples

### Deploy and configure an app

```bash
# Create app directory locally
mkdir myapp && cd myapp
git init

# Add code and Procfile
echo 'web: python app.py' > Procfile
git add . && git commit -m "init"

# Add remote and deploy
git remote add riku riku@server:myapp
git push riku main

# Configure
riku config set myapp DATABASE_URL=postgres://localhost/db
riku ps myapp --scale web=2

# Monitor
riku logs myapp
riku ps myapp
```

### Manage environment variables

```bash
# Set multiple vars
riku config set myapp \
  DATABASE_URL=postgres://localhost/db \
  SECRET_KEY=supersecret \
  DEBUG=false

# Get a value
riku config get myapp DATABASE_URL

# Remove a var
riku config unset myapp DEBUG

# View all
riku config show myapp
```

### Scale and restart

```bash
# Scale up
riku ps myapp --scale web=4 worker=2

# Restart the running processes, no rebuild
riku restart myapp

# Stop for maintenance
riku stop myapp

# Start again (rebuilds from the checkout)
riku deploy myapp
```
