# Server settings

Server wide settings live in `~/.riku/settings.toml` (under `$RIKU_ROOT` when
that is set). The file is optional. Without it every setting has the default
shown below, so an existing server behaves as before.

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

Older versions kept these in `ROUTER` and `CONTAINER_ENGINE` files in the
riku root. Those files are still read when the key is not set here. An
invalid settings.toml makes every router lookup fail with the file's error,
rather than quietly falling back to nginx.

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

When riku changes a key itself (`riku init`, `riku doctor --fix`) it rewrites
the file from its parsed form: every key is kept, comments are not.

## Migrating

`riku doctor --fix`, run as the deploy user, moves the old homes into the new
ones:

- the `ROUTER` and `CONTAINER_ENGINE` files into `router.name` and
  `engine.program` (a key already set here wins), then removes the files;
- each app's `RIKU_*` keys from its ENV file into its server override
  (`riku settings`). A key moves only when the override gives back exactly
  the value it had, so the app runs with the same settings; a key whose value
  would change (for example `RIKU_HTTPS_ONLY=1`, which is not `true` or
  `false`) stays in ENV with a warning. ENV is first copied to `ENV.bak`, and
  every other line is kept as it was.

Running it again changes nothing. Downgrading riku after the migration is not
supported: older versions refuse the new settings.toml sections and no longer
find the keys that moved out of ENV.
