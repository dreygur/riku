# rk: a laptop only riku client

## Why

The laptop side of riku (server list, target selection, ssh forwarding, the
remote dashboard tunnel, grouped help, client plugins, quickstart) needs none
of the server code, but it lives in `riku-cli`, which depends on deploy,
supervisor, nginx, caddy, plugins and reqwest. A laptop install today carries
the whole server. The owner wants a separate small client binary, `rk`, built
from this workspace, before the container services project and as the base
for managing several servers later.

## Goals

1. A second binary, `rk`, for laptops, that manages servers and never runs
   server code.
2. `riku` keeps everything it does today, including `-s` forwarding, so an
   older laptop install keeps working.
3. Shared code lives in one place; neither binary copies the other.
4. A test proves `rk` does not link the server crates.

## Non goals

- No behaviour change for `riku` on servers.
- `container export/deploy-remote` stays in `riku` only, until the container
  services project gives both binaries an engine agnostic driver (owner
  agreed).
- No change to the ssh protocol or the server side.

## Names

- Binary `rk`, package `riku-client`. The `rk` crate name on crates.io is
  taken by an abandoned yanked crate from 2021; the binary name is free on
  Homebrew, Debian and this machine (checked 2026-10-30).

## What rk does

- Forwarded commands: the same list and behaviour as `riku` today (target
  order `-s`, repo git remote, default; `[*]` target line; quoting; app hint).
- Local commands: `server`, `plugin` (client plugins), `quickstart`, `help`,
  and `dashboard -s` (the remote tunnel only).
- No local target: `riku` falls back to running locally; `rk` has nothing to
  run locally, so a forwarded command with no server selected is an error
  listing the registered servers and how to pick one. `--local` does not
  exist in `rk`.
- Server only commands (`init`, `supervisor`, hidden internals) and `container`
  are refused with a message saying to use `riku` (on the server, or the full
  binary for `container`).
- `rk -h` shows the same grouped help, with only the groups that apply.
- Client plugins work in both binaries (owner: plugins can also be installed
  on the server), from the same code.

## Crate layout

- `riku-args`: the clap command definitions (`cli.rs`, `cmds.rs`, the
  `ContainerSubCmd` enum moved out of `container.rs`), `placement`, `help`,
  `app_hint`. Depends on clap and riku-util only.
- `riku-remote`: `remote/` (types, store, git_remote, target, route, forward,
  server_cmd, dashboard client), client plugins, quickstart. Depends on
  riku-args, riku-util, shlex, toml, nix, dirs, tempfile.
- `riku-cli`: the server handlers; depends on both new crates and keeps
  everything else.
- `riku` binary: unchanged surface. `riku-client` binary package builds `rk`.
- The token generator used by the dashboard client (today
  `riku-supervisor` `generate_token`) moves to riku-util so riku-remote does
  not depend on the supervisor.

## Verification

- A test runs `cargo tree -p riku-client` (or reads `cargo metadata`) and
  fails if riku-deploy, riku-supervisor, riku-nginx, riku-caddy, riku-plugins,
  riku-dashboard or reqwest appear.
- Existing tests keep passing unchanged for `riku`.
- `rk` gets integration tests for: forwarding with `-s`, the no server error,
  refusing `init` and `container`, `server add/list`, a client plugin.
- Release binary sizes of `rk` and `riku` measured and reported.
- One host check in the existing Docker test host: `rk` against `rikutest`
  for `apps`, `run app echo "a b"`, and `dashboard -s`.

## Packaging

- The release workflow and `scripts/install.sh` build and ship `rk` beside
  `riku`; the install script can install either. The builder reads the current
  workflow and script before changing them.

## Decisions taken by the owner

- Separate client binary, named `rk`, built before the container project.
- `container` stays out of `rk` for now.
- Client plugins in both binaries.

## Calls made by the designer

- Package name `riku-client`; crate names `riku-args`, `riku-remote`.
- `rk` has no `--local` and errors when no server is selected.
- `riku` keeps forwarding.
- The cargo tree test as the guard against heavy dependencies creeping back.
- `SshAddress` and `ServerName` (validated newtypes) live in riku-args, since
  the command definitions use them; `routing.rs` (client plugin routing)
  lives in riku-remote. No cycle: riku-remote depends on riku-args only.
- One clap tree for both binaries (goal 3). `rk` parses the full `Commands`;
  its help shows only the groups that apply, and server only commands and
  `container` are refused at dispatch with the message above.
- `rk` never runs as an ssh forced command, so it does not use
  `ssh_command::process_argv`. It may depend on riku-config for home and path
  lookup (small, repository layer); riku-config is not on the forbidden list.
- Client plugins run before forwarding, same order as `riku`; with no server
  selected a plugin gets an empty `$1`, as today.
- `rk quickstart` keeps all of quickstart's behaviour.
- Versions move in lockstep: `riku-client` carries the same version as
  `riku`. Releases add `rk-<os>-<arch>` artifacts with their own checksum
  files next to the existing ones; `scripts/install.sh` gains an option to
  install `rk` instead of `riku` (default stays `riku`).
