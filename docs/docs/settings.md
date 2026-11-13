# Server settings

Server wide settings live in `~/.riku/settings.toml` (under `$RIKU_ROOT` when
that is set). The file is optional. Without it, or without one of its
sections, every setting has the default shown below.

```toml
[router]
name = "caddy"

[engine]
program       = "podman"
call_timeout  = "30s"
stop_extra    = "30s"
pull_timeout  = "30m"
build_timeout = "30m"
```

Per app settings live in [riku.toml](riku-toml.md), not here.

## Router and engine

| Key | Default | Meaning |
| --- | --- | --- |
| `router.name` | `nginx` | The router that exposes apps: `nginx`, `caddy`, or the name of a router plugin. `riku init` writes it from `RIKU_ROUTER`. |
| `engine.program` | detected | The container engine: a name such as `podman`, or an absolute path. |

riku runs the engine as the deploy user. A rootful docker refuses that user
its socket (`permission denied ... docker.sock`), and `riku doctor` and a
container deploy then say so and stop. Two ways out, and the choice is the
operator's: add the deploy user to the `docker` group, which is
root-equivalent on the host, or use a rootless engine such as rootless
podman. Deploys and doctor change neither the group nor the socket.

`sudo riku init` asks once, on a terminal, when the engine is docker and
the deploy user cannot open its socket: `Add riku to the docker group so
it can run container apps? This gives the riku user root-equivalent access
on this host. [y/N]`. The group it names is the one that owns the socket.
Only `y` or `yes` adds the deploy user (with `usermod -aG`, or BusyBox
`addgroup`), and the service step that follows restarts a running
supervisor so it gets the group, in the same single restart an upgrade
needs; if
that fails, init prints the command to run and goes on. On NixOS it prints
the `extraGroups` line for configuration.nix instead. Any other answer, no terminal, a socket
owned by root's group, or a group riku cannot resolve changes nothing and
prints the two choices above. Podman and other engines, and a socket the
deploy user already opens, get no question.

An invalid settings.toml makes every router lookup fail with the file's
error, rather than quietly falling back to nginx.

## Routing

How an app without a domain of its own gets a URL. `riku init` writes this
section when it is missing. See
[App URLs](routing.md) for what each mode does.

```toml
[routing]
fallback_domain = "{app}.{ip}.sslip.io"
server_ip = "203.0.113.10"
shared_port = 8080
port_range = "8100-8199"
```

| Key | Default | Meaning |
| --- | --- | --- |
| `routing.fallback_domain` | off | Name pattern for an app with no domain. Must contain `{app}`; `{ip}` is `server_ip`. Remove it to turn the fallback off. |
| `routing.server_ip` | detected at init | The server's public IPv4 address. Left out when only a private address is found. |
| `routing.shared_port` | `8080` | The port path mode apps share. |
| `routing.port_range` | `"8100-8199"` | Where riku picks a port for a port mode app that sets none. |

Without the section every key has the default in the table: no fallback
domain, so an app without a domain is in port mode. `riku doctor` prints a
`routing` line saying which applies.

## Engine time limits

Every call riku makes to the container engine is bounded. When the engine is
still running at the limit, riku kills it, reaps it and reports an error
naming the engine command and the limit.

| Key | Default | Applies to |
| --- | --- | --- |
| `call_timeout` | `30s` | Quick calls: run, remove, kill, list, inspect, stats, networks, volumes, image ids and registry login. |
| `stop_extra` | `30s` | Added to a container's own stop grace period for `stop`. A container with a 30 second grace is given 60 seconds. |
| `pull_timeout` | `30m` | A whole image pull. |
| `build_timeout` | `30m` | A whole image build. |

Pulls and builds print their progress as they run. The engine's output goes
to the same terminal or log riku writes to, so a failed pull or build reports
the exit status and leaves the reason in that output.

## Format

A duration is a whole number greater than zero followed by `s`, `m` or `h`:
`"45s"`, `"10m"`, `"2h"`. Anything else is refused, as is an unknown section
or key, so a misspelt key is an error rather than a silently ignored setting.
The error names the key and the file.

`riku doctor` parses the file and fails with that error when it is invalid.
It reports `defaults` when the file does not exist.

When `riku init` changes a key itself it rewrites the file from its parsed
form: every key is kept, comments are not.
