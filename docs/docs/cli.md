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
names it: `Forwarded commands (run on solutech, or here with --local):`.

## Remote Servers

The local `riku` binary can manage any number of servers over ssh. It runs the
system `ssh` and `git`; there is no ssh library inside riku. A forwarded
command such as `riku ps myapp` runs on the selected server, streams its output
back, and exits with the exit code of the remote command.

### `riku server`

The server list is local to your machine and always read here, never
forwarded.

```bash
riku server add solutech riku@solutech.example
riku server add edge riku@edge.example --admin root@edge.example
riku server list
riku server default solutech
riku server remove edge
```

| Subcommand | Description |
|------------|-------------|
| `add <name> <user@host> [--admin <user@host>]` | Register a server. A name that already exists is an error |
| `list` | Show name, address and admin login. The default is marked with `*` |
| `remove <name>` | Forget a server. Removing the default leaves no default |
| `default <name>` | Make a registered server the default |

The first server you add is not made the default. Run `riku server default
<name>` to pick one.

A name is 1 to 32 characters of `a-z`, `0-9`, `_` or `-`, starting with a
letter or digit. An address is `user@host`: the host is a hostname, an IPv4
address, or a bracketed IPv6 literal. There is no port syntax. For a server
on another port, define a `Host` alias in `~/.ssh/config` and use the alias as
the host (`riku@solutech-alt`).

`--admin` is a second login on the same host with sudo rights. Only
`riku dashboard` uses it.

The list is stored in `servers.toml` under `$XDG_CONFIG_HOME/riku/`, or
`~/.config/riku/` when `XDG_CONFIG_HOME` is unset (also on macOS). The
directory is mode 700 and the file mode 600. A hand edited entry is checked
with the same rules as one typed on the command line.

### Choosing the server

For a forwarded command the first rule that applies wins:

1. `-s <name>` or `--server <name>` on the command line, for that run only.
2. The current git repo. Riku reads `git remote -v` and compares the
   `user@host` of each scp style remote (`riku@solutech.example:myapp`)
   exactly against the registered addresses. Two remotes that match
   different servers are an error; choose with `-s`.
3. The default server.
4. No server: the command runs on this machine, as it did before.

`--local` skips all of this and runs here. Outside a git repo, or without
`git` installed, rule 2 finds nothing and selection moves on.

The match is a plain string comparison. There is no DNS lookup and no ssh
config alias resolution, so `riku@solutech.local` and `riku@192.168.1.20` are
different servers even when they are the same machine. A remote written as an
`ssh://` URL is not matched either. Register the address your remote uses.

Before every forwarded command riku prints the target on stderr, so piped
output stays clean:

```
[*] solutech (riku@solutech.example)
```

There is no confirmation prompt.

### What runs where

| Where | Commands |
|-------|----------|
| Local, always | `server`, `quickstart`, `container`, `plugin` |
| Server only | `init`, `supervisor`, `plugin-emit`, and the hidden git and internal commands |
| Forwarded | `apps`, `ps`, `logs`, `stats`, `config`, `deploy`, `restart`, `stop`, `destroy`, `rollback`, `run`, `backup`, `restore`, `addon`, `doctor`, `update`, `agent`, `install-plugins`, `plugins`, `hook` |

Giving `-s` to a server only command is refused: `use --local to run it here`.
`dashboard` is the exception, see [`riku dashboard`](#riku-dashboard). A client
plugin that overrides a command still runs first, and it receives the selected
server's address as `$1`.

`run` and `logs --follow` allocate a tty on the server, so interactive
programs and Ctrl-C behave. Everything else runs without one.

### App name from the repo

When the server was picked because of a git remote, a command that takes an
app name can leave it out and use the remote's path. With the remote
`riku@solutech.example:myapp`, these both act on `myapp`:

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
ssh riku@solutech.example ps myapp
ssh riku@solutech.example riku ps myapp
```

### Upgrading existing servers

Three changes break older setups.

- `ps --scale` lost its `-s` short form, because `-s` now means `--server`.
  Write `--scale`.
- Quoted arguments over ssh now arrive intact. A command that relied on the
  old splitting or glob expansion on the server behaves differently.
- Keys installed by an older riku force `riku $SSH_ORIGINAL_COMMAND`. They
  must be rewritten, and the order matters.

Do this on each server:

1. Install the new riku binary on the server, the way you installed the old
   one. **Do this first.** A migrated key passes no arguments, which an
   old binary cannot run: with the old binary and migrated keys, no command
   works over ssh.
2. As the deploy user, run `sudo -u riku riku doctor --fix`. It rewrites the
   `command=` part of each old riku key in `~/.ssh/authorized_keys`, keeping
   the key, its options and its name, and replacing the file atomically with
   mode 600. It refuses to run as root.

Until then `riku doctor` warns: `N deploy keys use the old forced command, run:
riku doctor --fix`. `riku init` also migrates the keys.

Update the server binary before forwarding from a new laptop binary as well.
Older servers reject the hidden `--app-hint` option that carries the app from
your git remote, so those commands fail until the server is updated.

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

---

### `riku restart <app>`

Restart all processes for an application.

```bash
riku restart myapp
```

Gracefully stops workers and restarts them. For zero-downtime rolling restarts, use `riku restart myapp --hot`.

---

### `riku stop <app>`

Stop all processes for an application.

```bash
riku stop myapp
```

To restart, use `riku restart <app>` or push a new commit.

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

---

### `riku ps <app> --scale web=N worker=N...`

Scale worker processes.

```bash
riku ps myapp --scale web=4 worker=2
```

This creates/updates the `SCALING` file and triggers a restart.

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
riku plugins install <path|url>    # Install from a local path or git URL
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
| `--remote <REMOTE>` | Deploy target for the git remote line, e.g. `riku@your-server` |

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
- `~/.riku/nginx/` - Nginx configurations
- `~/.riku/cache/` - Nginx cache files
- `~/.riku/workers-available/` - Worker configs
- `~/.riku/workers-enabled/` - Enabled worker symlinks
- `~/.riku/plugins/` - Plugin executables
- `~/.riku/acme/` - SSL certificates

Also installs the riku binary to `~/.local/bin/riku` and generates a systemd user service.

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

### `riku doctor`

Diagnose the installation (deps, dirs, systemd, nginx, disk, SSH). See
[Doctor](doctor.md).

```bash
riku doctor
sudo riku doctor   # most complete report
sudo -u riku riku doctor --fix   # rewrite old deploy key forced commands
```

`--fix` is the only thing doctor writes. It must run as the deploy user, not
root, and only after the new binary is installed on the server. See
[Upgrading existing servers](#upgrading-existing-servers).

---

### `riku dashboard`

Serve the embedded web dashboard (single binary, no separate install). It can
deploy, roll back, and manage addons, not read-only, so always set a token
before exposing it beyond loopback. See [Dashboard](dashboard.md), which also
covers the separate, fuller Next.js browser dashboard.

```bash
riku dashboard
riku dashboard --bind 0.0.0.0:8088 --token <tok>
riku dashboard -s solutech
```

| Option | Description |
|--------|-------------|
| `--bind <BIND>` | Address to bind (`host:port`). Default: `127.0.0.1:8088` |
| `--token <TOKEN>` | Require this token on the API (also via `RIKU_DASHBOARD_TOKEN`) |

With no server selected, the dashboard is served on this machine as above.
With one selected (`-s`, the repo's git remote, or the default), it opens that
server's dashboard in your local browser in one step:

1. The server entry needs an admin login. Without one riku stops and prints
   the command to fix it: `riku server add solutech riku@solutech.example
   --admin root@solutech.example`. `--bind` and `--token` are refused, since
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

The `riku` binary must be on the deploy user's `PATH` or at
`~/.local/bin/riku` on the server.

---

### `riku container <command>`

Container export and remote deployment (auto-detects Docker/Podman).

```bash
riku container export                       # Build + export image to a tar archive
riku container deploy-remote -a myapp -r user@host  # Build, transfer, deploy
riku container deploy-archive               # Deploy an exported tar archive to remote
riku container check-remote                 # Check remote for Docker/Podman
```

---

### `riku update`

Update the Riku binary to the latest version.

```bash
riku update
```

Downloads and replaces the current binary from the official release.

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

# Restart after config change
riku restart myapp

# Stop for maintenance
riku stop myapp

# Start again
riku restart myapp
```
