# Riku

**Git-push deployments on a single small box — one Rust binary, no Docker required.**

[![CI](https://github.com/dreygur/riku/actions/workflows/ci.yml/badge.svg)](https://github.com/dreygur/riku/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/dreygur/riku?sort=semver)](https://github.com/dreygur/riku/releases)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org/)

Riku gives you a Heroku-style workflow on hardware you already own — a VPS, an old
laptop, a Raspberry Pi. You `git push`, Riku detects the language, builds it, runs
it under a supervisor, and wires up nginx. That's it. No control plane, no cluster,
no container runtime to babysit.

It's a Rust reimplementation of [Piku](https://github.com/piku/piku), keeping that
project's directory layout and workflow while shipping as a single static binary
with no runtime dependencies.

```bash
# on your server
curl -LO https://github.com/dreygur/riku/releases/latest/download/riku-linux-amd64.tar.gz
tar -xzf riku-linux-amd64.tar.gz && sudo ./riku init

# on your laptop
git remote add riku deploy@your-server:myapp
git push riku main      # 👈 that's the deploy
```

## What it does

- **`git push` to deploy** — language detected, built, supervised, and routed automatically.
- **Many languages, no recompile** — Python, Node, Ruby, Go, Rust, Java, Clojure, and containers, each as an installable runtime plugin.
- **Built-in process supervisor** — health checks, restarts, scaling, and cron, written in Rust (no uWSGI Emperor).
- **Zero-downtime deploys** — new processes are health-gated before old ones are retired.
- **Rollback & backups** — `riku rollback`, `riku backup`/`riku restore`.
- **Managed datastores as plugins** — attach Postgres, Redis, or a SQLite volume with `riku addon`; the connection string is injected into your app's env.
- **A plugin ecosystem** — a versioned plugin protocol, a git-native marketplace, signed bundles, and capability sandboxing (Landlock) for plugins you install.
- **Swappable router** — nginx by default; drop in a Caddy/Traefik router plugin without touching core.
- **Embedded dashboard** — `riku dashboard` serves a status UI straight from the binary, no Node runtime on the host.
- **`riku doctor`** — diagnoses nginx, systemd, permissions, disk, and certs when something's off.

## Requirements

- Linux (Debian/Ubuntu/RHEL/Arch)
- 1 CPU core, 256 MB RAM, ~50 MB disk for Riku itself

Base footprint is roughly 30–60 MB before you deploy anything (supervisor + binary +
nginx). Per-app memory is up to your apps. If you want numbers for your own hardware,
the scripts in [`benches/`](benches/) measure it rather than guess.

## Install

### From a release (recommended)

```bash
curl -LO https://github.com/dreygur/riku/releases/latest/download/riku-linux-amd64.tar.gz
tar -xzf riku-linux-amd64.tar.gz
chmod +x riku
sudo ./riku init
```

Running `riku init` as root installs the binary to `~/.local/bin`, creates a systemd
service that starts on boot, launches the supervisor, and enables nginx auto-reload.
Deploying as a user other than `deploy`? Pass `sudo RIKU_USER=myuser ./riku init`.

### From source

```bash
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
./target/release/riku init
```

## Quick start

**1. Set up the server.** Create a deploy user and initialize:

```bash
sudo adduser deploy && sudo su - deploy
riku init
ssh-copy-id deploy@your-server      # add your SSH key
```

**2. Deploy an app.** Add a `Procfile`, commit, and push:

```
web: gunicorn app:app
worker: python worker.py
```

```bash
git init && git add . && git commit -m "first deploy"
git remote add riku deploy@your-server:myapp
git push riku main
```

In a hurry? `riku quickstart` scaffolds a sample app and prints the exact
`git remote add` line to copy.

> You can keep a bare repo anywhere on the server — `git init --bare ~/projects/myapp.git`,
> push to it, and Riku symlinks it into `~/.riku/repos/` for you.

## Commands

| | |
|---|---|
| `riku apps` / `riku deploy <app>` / `riku destroy <app>` | list, redeploy, remove apps |
| `riku logs <app> [proc]` / `riku restart <app>` / `riku stop <app>` | runtime control |
| `riku ps <app> [--scale web=2 worker=1]` | inspect / scale processes |
| `riku config show\|get\|set\|unset\|live <app>` | manage environment |
| `riku run <app> <cmd…>` | run a command in the app's context |
| `riku rollback <app> [--to <sha>\|--list]` | roll back to a previous release |
| `riku backup <app>` / `riku restore <app> <file>` | snapshot / restore |
| `riku addon list\|create\|bind\|unbind\|backup\|destroy` | managed datastores |
| `riku plugins …` | install/search/scaffold/sign plugins (see below) |
| `riku dashboard [--bind … --token …]` | serve the web dashboard |
| `riku doctor` | diagnose the host |
| `riku init` / `riku update` / `riku supervisor` | setup & maintenance |

## Runtime plugins

Riku builds apps through runtime plugins in `~/.riku/plugins/`. Install the bundled
set with `riku install-plugins` (or pick some: `riku install-plugins --plugins node,python`).

| Plugin | Detects | Build tool |
|--------|---------|------------|
| `node` | `package.json` | nub (npm / pnpm / bun lockfiles) |
| `python` | `requirements.txt`, `pyproject.toml` | pip / Poetry / uv |
| `ruby` | `Gemfile` | Bundler |
| `go` | `go.mod`, `.go` files | go build |
| `rust-lang` | `Cargo.toml` | cargo build |
| `java` | `pom.xml`, `build.gradle` | Maven / Gradle |
| `clojure` | `project.clj`, `deps.edn` | Leiningen / Clojure CLI |
| `container` | `Dockerfile`, `docker-compose.yml` | Docker / Podman |

Detection runs in order — set `RUNTIME=node` to skip it. To add your own language,
drop an executable into `~/.riku/plugins/` implementing four subcommands:

| Subcommand | Does |
|---|---|
| `detect` | exit 0 if it handles this app, non-zero to skip |
| `build` | install dependencies (stdout streams to the deploy log) |
| `env` | print `KEY=VALUE` lines merged into the app environment |
| `start` | print the default start command |

Context arrives via `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`. The
full contract is in [`PLUGIN_PROTOCOL.md`](PLUGIN_PROTOCOL.md).

## Plugins & addons

Beyond runtimes, Riku has a manifest-based plugin system with a versioned protocol,
a git-native marketplace, and a server-side trust model:

```bash
riku plugins marketplace add github:dreygur/riku   # this repo is a marketplace
riku plugins search postgres
riku plugins add postgres                            # checksum + signature verified
riku plugins scaffold my-addon --type addon          # start your own
```

Installed plugins are pinned in a lockfile, can carry Ed25519 author signatures, and
run under capability sandboxing (filesystem/network limits via Landlock) based on
what their manifest declares. Managed datastores ship this way — see
[`examples/plugins/`](examples/plugins/) for working postgres, redis, sqlite-volume,
caddy-router, and webhook-notify bundles.

## Configuration

Per-app settings live in an `ENV` file (`riku config set` writes it). A few common ones:

```bash
NGINX_SERVER_NAME=example.com   # domain
NGINX_HTTPS_ONLY=true           # force HTTPS
NODE_VERSION=20.11.0            # pin a runtime version
RIKU_WORKER_TIMEOUT=3600        # kill unresponsive workers
RIKU_ROUTER=caddy               # use a router plugin instead of nginx
```

See [`docs/docs/env.md`](docs/docs/env.md) for the full reference, and `Procfile`
for `web:` / `worker:` / `cron:` process definitions.

## How it works

Everything lives under `~/.riku/` — `apps/` (source), `envs/` (config), `repos/`
(bare git), `logs/`, `nginx/`, `plugins/`, `data/`. A `git push` triggers a
post-receive hook that checks out the code, runs the matching runtime plugin to
build it, writes worker configs, and signals the supervisor. The supervisor watches
those configs and starts/stops/restarts processes to match, health-gating new
generations before cutover. nginx config is generated per app (or handed to a router
plugin). For the design in depth, see [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Development

```bash
git clone https://github.com/dreygur/riku.git && cd riku
cargo build
cargo test
cargo clippy && cargo fmt
```

Contributions welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md) and the
[Code of Conduct](CODE_OF_CONDUCT.md). The short version: fork, branch, make sure
`cargo test` and `cargo clippy` pass, open a PR.

## License

MIT — see [`LICENSE`](LICENSE).

## Credits

Riku owes everything to [Piku](https://github.com/piku/piku) and the people behind
it: the original micro-PaaS whose concepts, workflows, and directory layout Riku
reimplements in Rust. It's an alternative implementation, not a replacement — if
Piku fits your needs, use Piku. Either way, support both.
