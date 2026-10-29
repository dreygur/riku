# Remote CLI and dashboard

## Why

Managing a riku server from a laptop today means `ssh riku@host riku <cmd>`
for every command, and two hand run scripts (`scripts/dashboard-tunnel.sh`,
`scripts/dashboard-web.sh`) to see the dashboard. The owner runs several
servers and wants the local `riku` binary to manage any of them, the same
way the dashboard can, without typing ssh.

More than one person may connect to the same server from different
machines, so nothing here may assume a single user.

## Goals

1. A local list of named servers with a default that can be changed.
2. Forwarded commands: `riku ps myapp` on the laptop runs on the selected
   server and streams its output back.
3. Server selection: per command flag, current git repo, default.
4. `riku dashboard -s <server>` opens the embedded dashboard of that server
   in the local browser in one step.
5. `riku -h` and `riku --help` group commands by where they run.
6. All dependencies bumped to their latest stable major version.

## Non goals

- No ssh or git library. Riku runs the system `ssh` and `git`.
- No port forwarding on deploy keys. They keep `no-port-forwarding`
  (owner decision, 2026-10-29). The forced command itself does change,
  see "Server side argument parsing".
- No shared, long running dashboard per server.
- The Next.js browser dashboard in `dashboard/` is untouched. The scripts
  in `scripts/` stay as they are.
- No server discovery by probing hosts or resolving DNS.

## Anti goals

- Sending a command to a machine the owner never registered.
- Guessing a server when the choice is ambiguous.
- Leaking the dashboard token through `ps` on the server.
- Breaking pipes: anything riku adds to output goes to stderr.

## Server list

Stored on the laptop at `${XDG_CONFIG_HOME:-~/.config}/riku/servers.toml`,
separate from `~/.riku`, which holds server state. Mode 600.

Each entry has:

- `name`: what the owner types after `-s`.
- `address`: the deploy login, `user@host`, used for forwarded commands.
- `admin`: optional `user@host` of a sudo capable login on the same host,
  used only for the dashboard. It may differ per host.

One entry may be marked default.

Commands (always local):

```
riku server add <name> <user@host> [--admin <user@host>]
riku server list
riku server remove <name>
riku server default <name>
```

Names and addresses are validated at the boundary: a name is a short
identifier, an address is `user@host` with no whitespace, no leading `-`
(so it can never be read as an ssh option), and no shell metacharacters.

## Choosing the server

First match wins:

1. `-s <name>` / `--server <name>` on the command line, for that run only.
2. The current git repo. Riku reads `git remote -v` and compares each
   remote's `user@host` exactly against the registered `address` values.
   A riku remote looks like `riku@solutech.local:myapp`. A match makes
   that server the temporary default for commands run in the repo.
   Remotes matching two different servers is an error asking for `-s`.
   No DNS lookup and no ssh config alias resolution: `solutech.local` and
   `192.168.1.20` do not match each other.
3. The default server.
4. None of the above: forwarded commands run locally, exactly as today.

`--local` skips selection and runs on this machine.

Not in a git repo, or `git` not installed, means step 2 finds nothing. It
is not an error.

### App name from the repo

When the server came from a git remote match and the command takes an app
name the owner did not type, the app is the remote's path after the colon
(`myapp` in `riku@host:myapp`, `myapp` in `riku@host:~/x/myapp.git`). A
typed app name always wins.

## Server side argument parsing

Today every deploy key forces
`command="FINGERPRINT=.. NAME=default <riku> $SSH_ORIGINAL_COMMAND"`
(`crates/riku-util/src/ssh_keys.rs:26`). The variable is expanded
unquoted, so on the server an argument containing a space is split and
`*` is glob expanded. `ssh riku@host 'run myapp echo "a b"'` is already
broken this way. It is not an injection: the shell does not reparse `;`
or `$(...)` out of an expansion.

Owner decision: fix it on the server.

- The forced command becomes `FINGERPRINT=.. NAME=default <riku>` with no
  arguments.
- When riku starts with no arguments and `SSH_ORIGINAL_COMMAND` is set, it
  splits that string with POSIX shell quoting rules (no expansion of any
  kind) and parses the result as its argv. Git pushes arrive the same
  way (`git-receive-pack 'myapp'`) and must keep working. The agent
  scope check in `crates/riku-cli/src/agent/auth.rs`, which already reads
  `SSH_ORIGINAL_COMMAND`, must agree with the new parse.
- A leading `riku` word is dropped, so `ssh riku@host riku ps myapp` and
  `ssh riku@host ps myapp` both work (owner decision: people without the
  local CLI keep typing the form the docs show today). This is safe
  because no subcommand is named `riku`; a test pins that.
- Existing keys are migrated: `riku doctor` reports old style lines,
  `riku doctor --fix` and `riku init` rewrite the `command=` part in
  place, keeping fingerprint, name, options and key. The rewrite is atomic
  (temp file plus rename, mode 600).
- Splitting needs a small shell words crate. It is the only new
  dependency in this project.

## Forwarded commands

Forwarded: apps, ps, logs, stats, config, deploy, restart, stop, destroy,
rollback, run, backup, restore, addon, doctor, update, agent,
install-plugins, plugins, hook.

Riku runs `ssh <address> <args>`, with each argument shell quoted so the
server side parse above gets back exactly what was typed. No leading
`riku`: the forced command supplies the binary. The exit code of the
remote command is riku's exit code.

`run` and `logs -f` (follow) get `ssh -t` so interactive programs and
Ctrl-C behave. Everything else runs without a tty so output pipes cleanly.

Before every forwarded command riku prints one line to stderr naming the
target, for example `[*] solutech (riku@solutech.local)`. No confirmation
prompt is added. Commands that already confirm, like `destroy`, keep
confirming, on the remote side.

Server only commands refuse with a clear message when a server is
selected: init, supervisor, and the hidden internal ones (git-hook, scp,
plugin-emit, ns-shim, dump-state, git-receive-pack, git-upload-pack).

Always local: server, quickstart, container, plugin.

Client plugins keep working and still run first: a client plugin that
overrides a command wins over forwarding, as it wins over the built in
today (`crates/riku/src/main.rs:45`). When a server is selected, `$1`
passed to a client plugin is that server's address instead of today's
empty string.

## Dashboard

`riku dashboard` with no server selected behaves exactly as today. With a
server selected (any rule above), it:

1. Requires the server's `admin` login, and says how to add one if it is
   missing.
2. Generates a fresh random token for this run only. No token file.
3. Opens one ssh connection as `admin` with OpenSSH connection sharing (a
   control socket in a private temp dir), and on it runs, as the deploy
   user through sudo, `riku dashboard --bind 127.0.0.1:0`. The deploy user
   is the user part of the server's `address`. The remote binary is the
   deploy user's `riku` on PATH, falling back to `~/.local/bin/riku`.
   The token goes over the session's stdin and is exported as
   `RIKU_DASHBOARD_TOKEN` by a shell running inside `sudo -u` (sudo's
   env_reset would strip it if exported outside), so it never appears in
   an argument list.

   Sudo password (owner decision: support it). Riku first tries
   `sudo -n`. If sudo says a password is required, riku asks for it on
   the local terminal without echo and runs `sudo -S -p ''`, sending the
   password as the first stdin line and the token as the second. The
   password is never stored, logged, or put in an argument list, and is
   dropped from memory once sent. A wrong password ends the session with
   sudo's error. Riku does not retry by itself.
4. Reads the port the remote dashboard actually bound, then adds the
   forward on the same connection (`ssh -O forward -L`). No window where
   two sessions race for a port.
5. Picks a local port: 8088 if free, otherwise any free port.
6. Opens `http://127.0.0.1:<local>/?token=<token>` in the browser and
   prints the URL.
7. On Ctrl-C or a dropped connection, stops its own remote dashboard and
   the forward. Other users' sessions are untouched.

Each session is its own dashboard process with its own token. Two users on
one host never share a port or a secret.

Server change needed: `riku dashboard` must report the address it really
bound (today it prints the requested `addr`, which is `:0`), in a line the
client can parse.

Accepted weakness: the token sits in the laptop's browser history for that
URL. The token dies with the session, so this is accepted.

## Grouped help

`riku -h` and `riku --help` list commands under three headings, in this
order:

```
Local commands:
  server, quickstart, container, plugin

Server commands:
  init, supervisor, dashboard

Forwarded commands (run on the selected server, or here with --local):
  apps, ps, logs, stats, config, deploy, restart, stop, destroy,
  rollback, run, backup, restore, addon, doctor, update, agent,
  install-plugins, plugins, hook
```

Each command keeps its current one line description. Hidden commands stay
hidden. The forwarded heading names the server that would be used right
now, when there is one.

clap has no subcommand groups, so this is a help template. A test checks
that every visible subcommand appears in exactly one group, so a new
command cannot be left out.

## Dependency bump

Every dependency to its latest stable major version, as its own first
commit, with the full suite, clippy and fmt clean before feature work
starts. Breaking API changes are fixed in that commit, not papered over.

## Order of work

1. Dependency bump.
2. Grouped help.
3. Server side argument parsing and key migration.
4. Server list and selection.
5. Forwarded commands.
6. Dashboard.

Branch `feat/remote-cli`, cut from `feat/caddy-router` (PR #71 not yet
merged).

## Calls made by the designer, not the owner

- Server list path and file format.
- Validation rules for names and addresses.
- Exact host matching only, no DNS or ssh config resolution.
- The forwarded and server only command lists, including agent,
  install-plugins, plugins and hook as forwarded.
- Client plugins keep priority over forwarding.
- Deploy user and remote binary lookup for the dashboard.
- One sudo password attempt, no automatic retry.- Target line on stderr.
- Local port 8088 with fallback.
- Token in the browser URL accepted.
- Help heading shows the current server.

## Left to the builder

- Module layout, following the repository/service/provider layering in
  `.claude/CLAUDE.md`.
- Exact output format of the bound port line.
- How the browser is opened per platform.
