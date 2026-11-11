# riku.toml

`riku.toml` holds an app's riku settings: its domains, runtime, worker and
health settings, and the processes to run. It is optional. Commit it next to
your code and push; riku reads it from the checkout on every deploy.

```toml
[routing]
domains = ["shop.example", "www.shop.example"]
https_only = true
static_paths = ["/static:public/static"]

[runtime]
name = "node"
version = "22"

[workers]
timeout = 60
max_restarts = 5

[health]
url = "/health"
interval = 10

[processes]
web = "node server.js"
worker = "node jobs.js"
release = "node migrate.js"

[cron]
backup = "0 2 * * * ./scripts/backup.sh"
```

An app without a `riku.toml` deploys as before, from its Procfile and ENV.

## Keys

Keys are `section.name`. Each maps to the `RIKU_*` variable riku used to read
from the app's ENV file.

| Key | Type | Replaces |
| --- | --- | --- |
| `routing.domains` | list of strings | `RIKU_DOMAINS` |
| `routing.https_only` | bool | `RIKU_HTTPS_ONLY` |
| `routing.document_root` | string, relative path | `RIKU_NGINX_DOCUMENT_ROOT` |
| `routing.static` | bool | `RIKU_NGINX_STATIC` |
| `routing.static_paths` | list of strings | `RIKU_NGINX_STATIC_PATHS` |
| `routing.allow_git_folders` | bool | `RIKU_NGINX_ALLOW_GIT_FOLDERS` |
| `routing.mode` | `"domain"`, `"port"` or `"path"`, see [App URLs](routing.md) | `RIKU_ROUTING_MODE` |
| `routing.port` | port number, 1024 and above | `RIKU_ROUTING_PORT` |
| `routing.bind_address` | string, server only | `RIKU_BIND_ADDRESS` |
| `runtime.name` | string | `RIKU_RUNTIME` |
| `runtime.version` | string, needs `runtime.name` | `RIKU_<NAME>_VERSION`, such as `RIKU_NODE_VERSION` |
| `workers.timeout` | whole number | `RIKU_WORKER_TIMEOUT` |
| `workers.grace_period` | whole number | `RIKU_WORKER_GRACE_PERIOD` |
| `workers.max_restarts` | whole number | `RIKU_MAX_RESTARTS` |
| `health.url` | string | `RIKU_HEALTH_CHECK_URL` |
| `health.interval` | whole number | `RIKU_HEALTH_CHECK_INTERVAL` |
| `health.timeout` | whole number | `RIKU_HEALTH_CHECK_TIMEOUT` |
| `health.retries` | whole number | `RIKU_HEALTH_CHECK_RETRIES` |
| `limits.max_memory_mb` | whole number, server only | `RIKU_MAX_MEMORY_MB` |
| `limits.max_cpu_seconds` | whole number, server only | `RIKU_MAX_CPU_SECONDS` |
| `limits.max_file_size_mb` | whole number, server only | `RIKU_MAX_FILE_SIZE_MB` |
| `limits.max_open_files` | whole number, server only | `RIKU_MAX_OPEN_FILES` |
| `limits.max_processes` | whole number, server only | `RIKU_MAX_PROCESSES` |
| `limits.isolation_max_memory_mb` | whole number, server only | `RIKU_ISOLATION_MAX_MEMORY_MB` |
| `limits.isolation_max_pids` | whole number, server only | `RIKU_ISOLATION_MAX_PIDS` |
| `limits.isolation_cpu_quota_us` | whole number, server only | `RIKU_ISOLATION_CPU_QUOTA_US` |
| `limits.isolation_cpu_period_us` | whole number, server only | `RIKU_ISOLATION_CPU_PERIOD_US` |
| `limits.isolation_root` | absolute path, server only | `RIKU_ISOLATION_ROOT` |
| `containers.watch_services` | list of strings | `RIKU_WATCH_SERVICES` |
| `secrets.ghcr_username` | string, server only | `RIKU_GHCR_USERNAME` |
| `secrets.ghcr_token` | string, server only | `RIKU_GHCR_TOKEN` |

### cgroup limits and the sandbox

The `limits.isolation_*` keys are per worker cgroup v2 limits. Setting
`isolation_max_memory_mb` (`memory.max`), `isolation_max_pids` (`pids.max`)
or `isolation_cpu_quota_us` (`cpu.max`, as quota per
`isolation_cpu_period_us`, default 100000) puts each of the app's workers in
its own cgroup under the supervisor's, with those limits. Nothing else
changes: the worker keeps the host filesystem and network, so a web worker
still answers the router. `isolation_cpu_period_us` alone limits nothing
and asks for no cgroup.

The supervisor can only create these cgroups when its systemd unit has
`Delegate=yes`, which `riku init` writes. Without it the supervisor prints a
cgroup diagnostic at startup, and a worker that sets one of these limits
refuses to start rather than run unlimited.

The full sandbox is a separate switch, the server only key
`limits.isolation_root`, set with
`riku settings set <app> limits.isolation_root=<dir>`. The repo file cannot
set it, and `riku config set` refuses `RIKU_ISOLATION_ROOT` like every other
key in the table above. The value must be an absolute path other than `/`,
with no `..` part, made only of letters, digits and `. - _ /`. The
supervisor refuses to start a sandboxed worker whose root is not an existing
directory. It starts the worker inside a `pivot_root` to
that directory, which must hold everything the worker needs, with its own
PID, mount and network namespaces. The network namespace leaves the worker
only its own loopback, so a sandboxed worker cannot reach the router, a
database or anything else on the host network. Use it only for self
contained workers, never for `web`. A sandboxed worker also gets its own
cgroup, with the limits above when they are set.

An unknown key, a value of the wrong type, or a malformed file stops the
deploy with an error naming the file, the line and the key. The error never
prints the value.

## What the repo file may not set

The repo file is pushed by whoever can push, so it is held to stricter rules:

- **Server only keys** (marked above: limits, isolation, binding and
  secrets) are refused. Set them on the server with `riku settings set`.
- **Text values** may only hold letters, digits and `. - _ / : @ + *`.
- **`routing.document_root`** must be a relative path with no `..`.

## Processes

`[processes]` lists the processes to run, like a Procfile: `web`, `worker`,
`release` (run once per deploy) and so on. Names hold letters, digits, `_`
and `-`. `[cron]` lists scheduled jobs as a five field cron schedule followed
by the command; schedules run in UTC.

An app declares its processes in `riku.toml` or in a Procfile, not both: a
deploy that finds both stops and asks you to keep one. `web` and a wsgi kind
(`wsgi`, `jwsgi`, `rwsgi`) may not be declared together. Container apps run
what their compose file or Dockerfile says, so a container app with
`[processes]` or `[cron]` in `riku.toml` is refused.

## Server settings: riku settings

Values that belong to the server, not the repo, go in the app's server
override at `~/.riku/envs/<app>/riku.toml`. Edit it with `riku settings`
(or `rk settings` from your laptop):

```bash
riku settings set myapp routing.domains=shop.example,www.shop.example
riku settings set myapp secrets.ghcr_token=ghp_xxx limits.max_memory_mb=1024
riku settings get myapp routing.domains
riku settings show myapp          # secrets shown as ***
riku settings unset myapp limits.max_memory_mb
```

`set` and `unset` check the file before writing it and redeploy the app. The
override may set every key but cannot declare processes.

## Precedence

For each key, later wins:

1. `riku.toml` in the app repo
2. the same `RIKU_*` variable still in the app's ENV file (from older
   versions; deploys warn about it)
3. the server override (`riku settings`)

`riku config set` now refuses these `RIKU_*` keys and names the `riku
settings` key to use instead. `riku doctor --fix` moves existing ones out of
ENV into the server override; see [Server settings](settings.md#migrating).
