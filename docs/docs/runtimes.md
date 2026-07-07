# Supported Runtimes

Riku supports multiple programming languages and frameworks. This guide covers deployment for each runtime.

---

## Python

### Detection

Riku detects Python apps by the presence of:
- `requirements.txt` - Standard pip
- `pyproject.toml` + `poetry` binary on PATH - Poetry
- `pyproject.toml` + `uv` binary on PATH - uv

### Standard Pip

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

### Poetry

**pyproject.toml:**
```toml
[tool.poetry]
name = "myapp"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.9"
flask = "^2.0.0"

[build-system]
requires = ["poetry-core"]
build-backend = "poetry.core.masonry.api"
```

**Procfile:**
```
web: poetry run gunicorn app:app
```

### uv

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

**Procfile:**
```
web: uv run gunicorn app:app
```

### Environment Variables

```bash
PYTHON_VERSION=3.11.4
PYTHON_PACKAGE_MANAGER=pip  # or poetry, uv
```

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

Riku builds Node apps with [nub](https://nubjs.com) — a single Rust binary that
reads `npm`, `pnpm`, and `bun` lockfiles directly, so there is no package
manager to choose. Commit a lockfile and Riku installs from it:

- Lockfile present (`package-lock.json`, `pnpm-lock.yaml`, `lock.yaml`, or
  `bun.lock`) → `nub install --frozen-lockfile --prod` (fails on lockfile drift).
- No lockfile → `nub install --prod`.

If the server doesn't have nub, Riku falls back to `npm` (`npm ci` /
`npm install`) so existing deployments keep working. Install the faster path
with `npm install -g @nubjs/nub`.

### Environment Variables

```bash
NODE_VERSION=18.17.0
NODE_PACKAGE_MANAGER=npm
PORT=3000
```

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

Riku detects Ruby apps by the presence of `Gemfile`.

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
RUBY_VERSION=3.2.0
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
GO_VERSION=1.21
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
JAVA_VERSION=17
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
RUST_VERSION=1.70.0
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

Riku detects container apps by the presence of a `Dockerfile`, `Containerfile`,
or a compose file (`compose.yaml`, `compose.yml`, `docker-compose.yaml`,
`docker-compose.yml`, checked in that order). It auto-detects whether Docker or
Podman is installed and uses whichever is available (Podman preferred if both
are present).

### Dockerfile / Containerfile apps

A single-container app builds an image from the repo and runs it directly:

```dockerfile
FROM node:22-slim
COPY . /app
WORKDIR /app
RUN npm ci --production
CMD ["node", "server.js"]
```

Riku builds the image (`docker build` / `podman build`) and runs it with
`docker run --rm -p $PORT:$PORT <image>`.

### Compose apps

A compose file describes one or more pre-built images to run — no local
`Dockerfile` needed:

```yaml
# compose.yml
services:
  web:
    image: ghcr.io/your-org/your-app:latest
    ports:
      - "8080:8080"
  worker:
    image: ghcr.io/your-org/your-app-worker:latest
```

For a compose app, `riku deploy` logs in to the configured registry (see
[GHCR authentication](#ghcr-authentication) below), runs `compose pull`, and
starts the stack with `compose up` — supervised as a single riku process, the
same way any other app's `web` process is.

### GHCR authentication

If `GHCR_USERNAME` and `GHCR_TOKEN` are set in the app's env, riku logs in to
`ghcr.io` before pulling:

```bash
riku config set myapp GHCR_USERNAME=your-username GHCR_TOKEN=ghp_xxx
```

Omit these for public images — pulls work without authentication. Docker Hub
and other registries referenced in the compose file are pulled as configured
in the compose file itself; riku does not manage credentials for them.

### Auto-updating on a new image push

Riku doesn't require a `git push` to notice a new image. Set
`RIKU_WATCH_SERVICES` to the comma-separated list of compose services you want
kept in sync with their registry tag:

```bash
riku config set myapp RIKU_WATCH_SERVICES=web,worker
```

The supervisor re-checks every 60 seconds: it re-pulls each listed service and
recreates it if the image actually changed. Both steps are safe to run
repeatedly — `compose pull` skips unchanged layers, and `compose up -d` only
recreates a service whose resolved image differs — so an unwatched or
unchanged app costs nothing extra. This is outbound-only (riku polls the
registry; nothing needs to reach riku from the internet), so it works
regardless of what's fronting your apps — nginx, Caddy, or nothing at all.

See [Environment Variables](env.md#container-compose-settings) for the full
variable reference.

---

## Static Sites

### Detection

Riku detects static sites when no runtime is detected but nginx can serve files directly.

### Configuration

```bash
riku config set myapp NGINX_STATIC_PATHS=/:public
```

### Directory Structure

```
myapp/
├── public/
│   ├── index.html
│   ├── css/
│   └── js/
└── Procfile  (optional, can be empty)
```

### SPA Routing

For single-page applications:

```bash
riku config set myapp NGINX_CATCH_ALL=index.html
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
