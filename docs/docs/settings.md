# Server settings

Server wide settings live in `~/.riku/settings.toml` (under `$RIKU_ROOT` when
that is set). The file is optional. Without it every setting has the default
shown below, so an existing server behaves as before.

```toml
[engine]
call_timeout  = "30s"
stop_extra    = "30s"
pull_timeout  = "30m"
build_timeout = "30m"
```

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

The router (`ROUTER`) and container engine (`CONTAINER_ENGINE`) choices stay
in their own files in the riku root.
