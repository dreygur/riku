# Supported Runtimes

Riku supports multiple programming languages and frameworks. This guide covers deployment for each runtime.

---

## Python

### Detection

Riku detects Python apps by the presence of `requirements.txt`,
`pyproject.toml` or `setup.py`.

### Package Manager

Riku builds Python apps with [uv](https://docs.astral.sh/uv/) and recommends
installing it on the server:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

With uv, a deploy installs into the app's `.venv`:

- `uv.lock` present → `uv sync --locked --no-dev` (fails when the lock is out
  of date with `pyproject.toml`, so commit the lock after changing
  dependencies).
- Otherwise `requirements.txt` → `uv pip install -r requirements.txt`, or
  `pyproject.toml` → `uv pip install -e .`, or `setup.py` → `uv pip install .`.

Without uv, Riku falls back to `pip` in a `python3 -m venv` virtualenv and
prints how to install uv. `riku doctor` warns while uv is missing. Either way
a failed install stops the deploy and shows the installer's output.

### requirements.txt

**requirements.txt:**
```txt
flask>=2.0.0
gunicorn>=20.0.0
```

**Procfile:**
```
web: gunicorn app:app
```

**Deploy:**
```bash
git push riku main
```

### pyproject.toml

**pyproject.toml:**
```toml
[project]
name = "myapp"
version = "0.1.0"
dependencies = ["flask>=2.0.0"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
```

Run `uv lock` and commit `uv.lock` for exact, repeatable installs. A Poetry
project (`poetry-core` build backend) installs the same way; Riku does not
run Poetry itself.

**Procfile:**
```
web: gunicorn app:app
```

The app's `.venv/bin` is on `PATH` for its processes, so commands need no
`uv run` or `poetry run` prefix.

---

## Node.js

### Detection

Riku detects Node.js apps by the presence of `package.json`.

### package.json

```json
{
  "name": "myapp",
  "version": "1.0.0",
  "scripts": {
    "start": "node server.js"
  },
  "dependencies": {
    "express": "^4.18.0"
  },
  "engines": {
    "node": "18.x"
  }
}
```

### Procfile

```
web: npm start
# or
web: node server.js
```

### Package Manager

Riku builds Node apps with [nub](https://nubjs.com), a single Rust binary that
reads `npm`, `pnpm`, and `bun` lockfiles directly, so there is no package
manager to choose. Commit a lockfile and Riku installs from it:

- Lockfile present (`package-lock.json`, `pnpm-lock.yaml`, `lock.yaml`, or
  `bun.lock`) → `nub install --frozen-lockfile --prod` (fails on lockfile drift).
- No lockfile → `nub install --prod`.

Riku recommends installing nub on the server:

```bash
npm install -g @nubjs/nub
```

Without nub, Riku falls back to `npm` (`npm ci` / `npm install`) and prints
how to install nub. `riku doctor` warns
while nub is missing. With neither nub nor npm on the server, the build stops
with `Node.js is not installed on this server (npm not found)`.

### Node Version

Pin the node the build must match in `riku.toml`:

```toml
[runtime]
name = "node"
version = "18.17.0"
```

The plugin gets it as `RIKU_NODE_VERSION`, set only from `[runtime]
version`. A `RIKU_NODE_VERSION` line in the app's ENV is an env var for the
app and does not pick the version. See [Environment Variables](env.md) for
what the version accepts.

### Example server.js

```javascript
const express = require('express');
const app = express();
const port = process.env.PORT || 3000;

app.get('/', (req, res) => {
  res.send('Hello from Riku!');
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port ${port}`);
});
```

---

## Ruby

### Detection

Riku detects Ruby apps by the presence of `Gemfile`. The build runs
`bundle install`; without bundler on the server it stops with
`Bundler is not installed on this server (bundle not found)`.

### Gemfile

```ruby
source 'https://rubygems.org'

ruby '3.2.0'

gem 'puma', '~> 6.0'
gem 'sinatra', '~> 3.0'
```

### Procfile

```
web: bundle exec puma -p $PORT
```

### Environment Variables

```bash
PORT=3000
```

### Example app.rb

```ruby
require 'sinatra'

set :bind, '0.0.0.0'
set :port, ENV['PORT'] || 3000

get '/' do
  'Hello from Riku!'
end
```

---

## Go

### Detection

Riku detects Go apps by the presence of:
- `go.mod` - Go modules
- `Godeps/` - Godeps
- `.go` files - Raw Go source

### Go Modules

**go.mod:**
```mod
module example.com/myapp

go 1.21

require github.com/gin-gonic/gin v1.9.0
```

**main.go:**
```go
package main

import (
    "github.com/gin-gonic/gin"
    "net/http"
    "os"
)

func main() {
    r := gin.Default()
    r.GET("/", func(c *gin.Context) {
        c.String(http.StatusOK, "Hello from Riku!")
    })
    r.Run(":" + os.Getenv("PORT"))
}
```

**Procfile:**
```
web: ./server
```

### Build

Riku automatically builds the Go binary:

```bash
go build -o server .
```

### Environment Variables

```bash
PORT=8080
```

---

## Java

### Detection

Riku detects Java apps by:
- `pom.xml` - Maven
- `build.gradle` - Gradle

### Maven

**pom.xml:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>myapp</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>3.1.0</version>
        </dependency>
    </dependencies>
</project>
```

**Procfile:**
```
web: java -jar target/myapp-1.0.0.jar
```

### Gradle

**build.gradle:**
```groovy
plugins {
    id 'org.springframework.boot' version '3.1.0'
    id 'java'
}

dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web'
}
```

**Procfile:**
```
web: java -jar build/libs/myapp-1.0.0.jar
```

### Environment Variables

```bash
PORT=8080
```

---

## Clojure

### Detection

Riku detects Clojure apps by:
- `deps.edn` - Clojure CLI
- `project.clj` - Leiningen

### Clojure CLI

**deps.edn:**
```clojure
{:paths ["src"]
 :deps {org.clojure/clojure {:mvn/version "1.11.1"}
        ring/ring {:mvn/version "1.9.6"}}}
```

**Procfile:**
```
web: clojure -M -m myapp.core
```

### Leiningen

**project.clj:**
```clojure
(defproject myapp "0.1.0"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring "1.9.6"]]
  :main myapp.core)
```

**Procfile:**
```
web: lein run
```

---

## Rust

### Detection

Riku detects Rust apps by the presence of both `Cargo.toml` **and** `rust-toolchain.toml`.

### Cargo.toml

```toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2021"

[dependencies]
actix-web = "4"
```

### Procfile

```
web: ./target/release/myapp
```

### Build

Riku builds in release mode:

```bash
cargo build --release
```

### Environment Variables

```bash
PORT=8080
```

### Example main.rs

```rust
use actix_web::{web, App, HttpServer, HttpResponse};

async fn index() -> HttpResponse {
    HttpResponse::Ok().body("Hello from Riku!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    HttpServer::new(|| {
        App::new().route("/", web::get().to(index))
    })
    .bind(("0.0.0.0", port.parse().unwrap()))?
    .run()
    .await
}
```

---

## Container

### Detection

Riku runs an app as containers when it has a compose file (`compose.yaml`,
`compose.yml`, `docker-compose.yaml`, `docker-compose.yml`, checked in that
order) or a `Containerfile` or `Dockerfile` at the app root, and
`runtime.name` is unset or `container`. Set `runtime.name` to another runtime
to keep deploying such a repo with that runtime instead.

Riku drives the engine named by `[engine] program` in
[settings.toml](settings.md), or else the
first of `docker`, `podman`, `nerdctl` and `finch` found on `PATH`, in that
order. Docker must be version 23 or newer (or an engine with a compatible
CLI), since riku reads engine output with `--format json`. Riku reads the
compose file itself and talks to the engine directly; no `compose` tool runs.
The deploy user must be able to open the engine's socket; for rootful
docker, `sudo riku init` asks whether to add it to the socket's group (see
[settings.md](settings.md#router-and-engine)).

The compose file, or a folder on its path, may be a symlink as long as its
real target stays inside the app directory (for example
`docker-compose.yml -> deploy/prod.yml`). A link that leaves the app is
refused, naming the path.

To deploy with a compose file under another name, set `COMPOSE_FILE` in the
app's env. It wins over the default names and claims the app for the container
runtime:

```bash
riku config set myapp COMPOSE_FILE=compose.prod.yml
```

The value must be a single path inside the app directory; see
[Environment Variables](env.md#compose-file).

### Dockerfile / Containerfile apps

A single-container app builds an image from the repo and runs it directly:

```dockerfile
FROM node:22-slim
COPY . /app
WORKDIR /app
RUN npm ci --production
CMD ["node", "server.js"]
```

Riku builds the image and runs it as one service named `app`. The app must
listen on port 8000 inside the container; riku also sets `PORT=8000` there,
so an app that reads `$PORT` works unchanged. Riku publishes that port on a
free 127.0.0.1 port and routes the app's domains to it. The service's restart
policy is `unless-stopped`: the supervisor starts it again whenever it exits,
until `riku stop <app>` stops it. It stays stopped until `riku restart <app>`
or a new deploy.

### Compose apps

A compose file describes one or more services. Each one is either pulled
from a registry (`image:`) or built on the server from the checked out repo
(`build:`), so one stack can mix both:

```yaml
# compose.yml
services:
  web:
    image: ghcr.io/your-org/your-app:latest
    ports:
      - "8080"
  worker:
    build: ./worker
```

On deploy riku logs in to the configured registry (see
[GHCR authentication](#ghcr-authentication) below), pulls the `image`
services, builds the `build` services, then replaces the containers of each
service that changed, in start order (see
[Rolling replacement](#rolling-replacement)). A service is unchanged, and
keeps running untouched, when all of its copies are up, there are as many as
`SCALING` asks for, and each was started with exactly what the deploy would
start it with now: the same image id (a rebuilt or newly pulled image under
the same tag counts as a change), env, command, ports, mounts, labels,
health check and the rest. The push prints `Keeping <service> (unchanged)`
for each one. Containers started by a riku before this check carry no
`riku.spec` label and are replaced once, as is every container after a riku
upgrade that changes how the spec is recorded.
Every copy is its own container, named `<app>.<service>.<n>`, on the app's
network `riku-<app>`, where the other services reach it by its service name.
Service names may not contain `.`, so no two apps can share a container name.
A top level volume `<key>` is the engine volume `<app>.<key>`, and volume
names may not contain `.` either, so no two apps can share a volume. The
compose `name` key and `COMPOSE_PROJECT_NAME` do not affect any name.
Containers of a service the compose file no longer has are removed.

A container app's Procfile is read only for its `release` line. That command
runs once per deploy, after the images are ready and before any copy is
replaced, in a one-off container from the new image of the routed service
(or else the first built service), with the app's env, network and mounts.
Before it runs, the services that service depends on (`depends_on`, followed
through their own `depends_on`) are started when none of their copies is
up, so a migration can reach its database on a first deploy. A dependency
that already runs is left alone for the release, and a service started for
it is not replaced again by the same deploy. The release still runs before
any routed copy is replaced. Its stdout and stderr print on the push output. A nonzero exit stops the
deploy, and the last lines of output go to `riku logs <app> --deploy`.

App env values reach only the services riku builds: the compose services
with a `build` section and the `app` service of a Dockerfile app. A pulled
`image:` service, such as `redis` or `postgres`, gets only its own compose
`environment`, so app secrets never reach a third party image. `RIKU_*`
keys configure riku itself and reach no container. A service's own
`environment` wins over the app env. So a `riku config set` replaces the
built services on the deploy it starts, and pulled services keep running.
The `release` command always gets the app env, even when it runs in a
pulled image.

`riku destroy` stops and removes every copy, the app network and the images
riku built for the app (`localhost/riku-<hash>`, one per `build` service),
and keeps the volumes and pulled images such as `redis:7-alpine`, which
other apps may use. An image the engine refuses to remove is a warning
naming the `image rm` command to run by hand. When the engine does not answer, destroy refuses and removes
nothing, since the containers would keep running with no app behind them.
`riku destroy <app> --force` removes riku's files and route anyway and warns
that the containers may still run.

### Ports and routing

Published ports listen on 127.0.0.1. A service gets all interfaces only with
the label `riku.public: "true"`. A port entry that names a host IP, such as
`"127.0.0.1:8080:8080"`, or a long form entry with `host_ip` or `mode`, is
refused with "use the riku.public label instead of a host IP".

One service serves the app's domains. With exactly one published TCP port in
the whole project, that port is routed. With several, label the routed
service with `riku.http` set to its published host port (`"8080:80"` with
`riku.http: "8080"`); several published ports without the label, or the
label on more than one service, fail the deploy. UDP ports are never routed.
An app with no published TCP port is taken off the router.

Each copy of the routed service gets its own free host port on 127.0.0.1,
whatever host port the compose file names, and the router lists every copy
as a backend.

### Labels

Set these on a compose service:

| Label | Meaning |
|-------|---------|
| `riku.public` | `"true"` publishes the service's ports on all interfaces. `"false"` or absent keeps them on 127.0.0.1. Any other value fails the deploy |
| `riku.http` | The published host port that carries the app's HTTP traffic, when the project publishes more than one |
| `riku.health` | A path, such as `/healthz`, that riku requests on a new copy of the routed service before it gets traffic |

Riku sets these on every container it starts, and a compose file that sets
one of them is refused:

| Label | Meaning |
|-------|---------|
| `riku.app` | The app the container belongs to |
| `riku.service` | The compose service |
| `riku.copy` | The copy number, from 1 |
| `riku.port` | The host port riku published for the routed service |
| `riku.restart` | The compose `restart` policy, which the supervisor applies. The engine's own restart policy stays off |
| `riku.spec` | A sha256 of everything the copy was started with except its image id. A deploy compares it to keep unchanged services running |

### Scaling

Copy counts live in `~/.riku/envs/<app>/SCALING`, one `<service>=<copies>`
line each. A service without a line runs one copy.

```bash
riku ps myapp --scale web=3
```

This grows or shrinks the service at once, without a rebuild, without
restarting the copies it keeps and without touching any other service, then
records the count in `SCALING` and prints what it did, such as
`Started 1 copy of web, 3 running`. A later deploy keeps that count. New copies take
the lowest free numbers; shrinking stops the highest numbers first, each
taken off the router before it stops.

One copy always works. More than one is refused, before anything changes,
for a service that:

- mounts writable storage: a named or anonymous volume, or a bind mount, that
  is not read only;
- publishes any port other than the routed HTTP one, since routers only
  balance HTTP.

So only the routed HTTP service without writable storage, and workers that
publish no port and mount no writable storage, can scale. A count of 0 is
refused; use `riku stop <app> <service>` instead.

### Health checks

A new copy has to be healthy before it counts as started:

- a compose `healthcheck` (not `disable: true`): the engine must report the
  container healthy within 120 seconds. A copy that exits or turns unhealthy
  fails at once;
- otherwise, for the routed service, a `riku.health` path: an HTTP GET on
  127.0.0.1 must answer 2xx within 60 seconds. Redirects are not followed,
  so a 3xx counts as unhealthy;
- otherwise, for the routed service, a TCP connect to its port within 60
  seconds;
- a service with neither a healthcheck nor a routed port is healthy once it
  started.

A service whose `depends_on` entry uses `condition: service_healthy` starts
only after every copy of that dependency passes its engine health check.

### Rolling replacement

Every deploy (for the services that changed), `riku restart` and image watch
update replaces copies one at a time. How depends on the service:

- The routed service without writable storage overlaps. For each old copy,
  riku starts a new one, waits for it to be healthy, adds it to the router,
  takes the old copy off the router, and only then stops the old copy. Every
  router change waits for a confirmed reload (see
  [Caddy reloads](caddy.md#reloads)). If the new copy never gets healthy, it
  is removed, the router is unchanged and the old copies keep serving. If
  the router cannot confirm a change, no old copy is stopped.
- Every other service stops first: the old copy stops, then the new one
  starts and must get healthy. Two copies of a worker or a database never run
  at once.

Services are replaced in start order. A failure stops the deploy there, and
the services after it keep their old copies.

Procfile apps get the same overlap for their `web` copies, with
`RIKU_HEALTH_CHECK_URL` (a path) or an open port as the health check. A
pinned `PORT` cannot be shared by two copies, so those apps stop the old
copy first.

### Stopping a copy: the SIGTERM contract

Riku stops a container with its compose `stop_signal` (default `SIGTERM`)
and kills it once `stop_grace_period` (default 30 seconds, rounded up to a
whole second) has passed. A Procfile copy gets `SIGTERM` from the supervisor,
then `SIGKILL` after the worker grace period (30 seconds).

An app should, on `SIGTERM`, finish the requests it is serving and exit
within the grace period. For an overlapping service the router has already
stopped sending it new requests by then.

### Storage services

A service that mounts writable storage never scales and always stops first,
since two copies must not write the same data. It is down on every deploy
that changes it and every `riku restart` that includes it, from the moment the old copy
stops until the new one is healthy: up to 120 seconds with a compose
healthcheck. Services that depend on it with `service_healthy` wait for it.
Use `riku restart <app> <service>` to restart only the services that need
it.

### Restarting and stopping one service

```bash
riku restart myapp          # every service, in start order
riku restart myapp worker   # only worker
riku stop myapp worker      # stop and remove worker's copies
riku stop myapp             # every container of the app
```

`riku restart` replaces the copies from the images they run now, without a
rebuild, and starts a stopped service again. `riku restart --hot` is the
same as a plain restart. `riku stop` takes the app off the router first when
the routed service is among the stopped ones, stops dependents before the
services they depend on, and keeps volumes. Naming a service of a Procfile
app is refused.

The supervisor watches the containers. A copy that exits is started again
according to its compose `restart` policy (`no` when unset; `unless-stopped`
for the service riku makes from a Dockerfile), with a delay of 1, 2, 4 and so
on up to 60 seconds between attempts, and not while a deploy of that app
holds its lock. A copy that `riku stop` or a scale down removed is never
started again by the supervisor, whatever its policy. A copy's restart count starts at 0 again when a
deploy or `riku restart` replaces it. Each copy's output goes to
`~/.riku/logs/<app>/<service>.<n>.log`, which `riku logs <app> <service>`
follows.

### Switching an app to containers

An app that ran as a Procfile app and gains a compose file or Dockerfile
keeps its old Procfile workers until the new containers are healthy and
routed. Riku then stops them and deletes their configs. If the containers
fail, the Procfile app keeps running as before.

### GHCR authentication

If `secrets.ghcr_username` and `secrets.ghcr_token` are set in the app's
server settings, riku logs in to `ghcr.io` before pulling:

```bash
riku settings set myapp secrets.ghcr_username=your-username secrets.ghcr_token=ghp_xxx
```

Set both or neither: a deploy with only one of them fails. The engine stores
the login in its user's shared auth file, so it outlives removing the keys,
applies to every app on the server, and the last login wins.

Omit these for public images: pulls work without authentication. Docker Hub
and other registries referenced in the compose file are pulled as configured
in the compose file itself; riku does not manage credentials for them.

### Auto-updating on a new image push

Riku doesn't require a `git push` to notice a new image. Set
`containers.watch_services` to the compose services you want kept in sync
with their registry tag:

```toml
# riku.toml
[containers]
watch_services = ["web", "worker"]
```

The supervisor re-checks every 60 seconds: it pulls each listed service's
image and, when the running copies run a different image than the tag now
names, replaces them one at a time as a deploy would. An unchanged image
changes nothing. Services built from source are skipped, and a service with
no running copies is not started. This is outbound-only (riku polls the
registry; nothing needs to reach riku from the internet), so it works
regardless of what's fronting your apps: nginx, Caddy, or nothing at all.

See [Environment Variables](env.md#container-compose-settings) for the full
variable reference.

---

## Static Sites

`routing.static = true` is the switch. With it set, nginx serves the app's
document root (`routing.document_root`, `public` by default) itself instead
of proxying to the app. `routing.static_paths` adds more `location:directory`
mappings, and works with or without `static = true`.

```toml
# riku.toml
[routing]
static = true
```

Static files are served only next to a runtime. Riku has no runtime-less
static deploy: the app must still match a runtime plugin, and it must declare
a process or use a runtime whose `start` gives a command. A repo with only
`public/index.html` and no runtime fails with `No runtime plugin matched`.

Static files need the nginx router. Under Caddy the router helper refuses
the route of an app with `static = true`, and a deploy that sets
`static_paths` stops before any copy starts, with
`routing.static_paths is nginx only`.

```
myapp/
├── public/
│   ├── index.html
│   ├── css/
│   └── js/
├── package.json
└── Procfile
```

---

## Procfile Examples

### Multiple Process Types

```
web: gunicorn app:app
worker: python worker.py
cron: 0 2 * * * ./scripts/daily.sh
```

### Language-Specific

**Python:**
```
web: gunicorn app:app -b 0.0.0.0:$PORT
```

**Node.js:**
```
web: node server.js
```

**Ruby:**
```
web: bundle exec puma -p $PORT
```

**Go:**
```
web: ./server
```

**Rust:**
```
web: ./target/release/myapp
```

---

## Troubleshooting

### Runtime Not Detected

1. Check for marker files (`requirements.txt`, `package.json`, etc.)
2. Ensure files are in the app root
3. Commit and push again

### Build Fails

1. Check build logs: `riku logs myapp`
2. Verify version compatibility
3. Check memory/disk space

### Port Binding Error

Ensure your app binds to `0.0.0.0:$PORT`:

```python
# Python
app.run(host='0.0.0.0', port=int(os.environ.get('PORT', 5000)))
```

```javascript
// Node.js
app.listen(process.env.PORT || 3000, '0.0.0.0');
```

---

## See Also

- [Environment Variables](env.md) - Runtime-specific ENV vars
- [CLI Reference](cli.md) - Deployment commands
- [Nginx Configuration](nginx.md) - Serving static files
