# Supported Runtimes

Riku supports multiple programming languages and frameworks. This guide covers deployment for each runtime.

---

## Python

### Detection

Riku detects Python apps by the presence of:
- `requirements.txt` - Standard pip
- `pyproject.toml` + `poetry.lock` - Poetry
- `pyproject.toml` + `uv.lock` - uv

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
git push riku master
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

### Package Managers

**npm (default):**
```bash
riku config:set myapp NODE_PACKAGE_MANAGER=npm
```

**yarn:**
```bash
riku config:set myapp NODE_PACKAGE_MANAGER=yarn
```

**pnpm:**
```bash
riku config:set myapp NODE_PACKAGE_MANAGER=pnpm
```

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

Riku detects Rust apps by the presence of `Cargo.toml`.

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

## Static Sites

### Detection

Riku detects static sites when no runtime is detected but nginx can serve files directly.

### Configuration

```bash
riku config:set myapp NGINX_STATIC_PATHS=/:public
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
riku config:set myapp NGINX_CATCH_ALL=index.html
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
