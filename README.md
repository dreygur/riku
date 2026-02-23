# Riku - The Rust Port of Piku

Riku is a complete Rust port of the popular [Piku](https://github.com/piku/piku) micro-PaaS. I built Riku to provide Heroku-like git push deployments while maintaining full compatibility with the Piku ecosystem.

**Credit & Acknowledgments:**

Riku stands on the shoulders of giants. I want to acknowledge and thank:

- **The Piku Team** - For creating the original Piku micro-PaaS that inspired this project. Riku implements the same concepts and workflows that made Piku great.
- **Piku Contributors** - All the developers who contributed to Piku over the years, establishing the patterns and features that Riku implements.
- **The Community** - For proving that simple, lightweight PaaS solutions have real value.

Riku is not a replacement for Piku, but rather an alternative implementation that:
- Uses Rust for better performance and zero runtime dependencies
- Maintains compatibility with Piku's directory structure and workflows
- Builds upon the excellent design decisions made by the Piku team

I encourage users to also check out the original [Piku project](https://github.com/piku/piku) and support both projects based on their needs.

## Features

- **Heroku-like deployments**: Deploy applications with `git push`
- **Multi-language support**: Python, Node.js, Ruby, Go, Java, Clojure, Rust
- **Process supervision**: Built-in Rust supervisor daemon (replaces uWSGI Emperor)
- **Nginx integration**: Automatic nginx configuration generation with caching support
- **Scaling**: Horizontal scaling with SCALING file or environment variables
- **Plugin system**: Extensible functionality through shell-based plugins
- **Cron scheduling**: Built-in cron job support via Procfile
- **Zero-downtime deployments**: Process management with graceful restarts
- **Environment variables**: Comprehensive env var support for configuration
- **Static site hosting**: Serve static files directly via nginx
- **SSL/HTTPS**: Automatic HTTPS redirect and SSL certificate support
- **AI Agent Interface**: SSH-based automation for AI agents (Claude, Cursor, Copilot)

## System Requirements

### Minimum Requirements
- **CPU**: 1 core (500 MHz+)
- **RAM**: 256 MB (512 MB recommended)
- **Storage**: 50 MB for Riku + app dependencies
- **OS**: Linux (Debian/Ubuntu/RHEL/Arch)

### Expected Resource Usage

> **Note**: Actual resource usage varies based on workload, number of apps, and traffic. The values below are estimates based on typical deployments.

#### Memory Footprint (Estimated)

| Component | Typical Range |
|-----------|---------------|
| Riku supervisor | 10-30 MB |
| Riku binary | ~8 MB |
| Per app process | 10-200 MB |
| Nginx | 5-15 MB |

**Total base usage**: ~30-60 MB (without apps)

#### Storage Usage

| Component | Typical Size |
|-----------|--------------|
| Riku binary | ~8 MB |
| System files | ~2 MB |
| Per app (code) | 1-100 MB |
| Per app (dependencies) | 10-500 MB |
| Logs (per app) | 10-100 MB |

### Why Choose Riku?

- **No Python dependency** - Single static binary
- **Smaller footprint** - Rust compilation vs Python interpreter
- **Faster startup** - Compiled binary vs interpreted code
- **Type safety** - Catch errors at compile time
- **Memory efficient** - No garbage collection overhead

> **Want to benchmark?** I encourage users to run their own benchmarks. See the `benchmarks/` directory for testing scripts. (TODO: Add benchmarking tools)

## Installation

```bash
# Clone the repository
git clone https://github.com/dreygur/riku.git
cd piku

# Build the Rust binary
cd riku
cargo build --release

# Copy the binary to your PATH
sudo cp target/release/riku /usr/local/bin/
```

## Quick Start

### Server Setup

1. Create a deploy user:
```bash
sudo adduser deploy
sudo su - deploy
```

2. Initialize the Riku environment:
```bash
riku setup init
```

3. Add your SSH public key:
```bash
riku setup ssh ~/.ssh/id_rsa.pub
```

### Application Deployment

1. Create a new application directory:
```bash
mkdir myapp
cd myapp
git init
```

2. Add your application code and create a Procfile:
```
web: python app.py
worker: python worker.py
```

3. Push to deploy:
```bash
git add .
git commit -m "Initial commit"
git remote add piku deploy@your-server.com:myapp
git push riku main
```

## Commands

### Application Management
- `riku apps` - List deployed applications
- `riku deploy <app>` - Force redeploy an application
- `riku destroy <app>` - Remove an application
- `riku logs <app> [process]` - View application logs
- `riku restart <app>` - Restart an application
- `riku stop <app>` - Stop an application

### Configuration Management
- `riku config <app>` - Show application configuration
- `riku config get <app> <key>` - Get a specific configuration value
- `riku config set <app> KEY=VALUE...` - Set configuration values
- `riku config unset <app> KEY...` - Remove configuration values
- `riku config live <app>` - Show live running configuration

### Process Management
- `riku ps <app>` - Show process counts
- `riku ps scale <app> web=2 worker=1` - Scale processes

### Running Commands
- `riku run <app> command...` - Execute commands in app context

### Setup and Maintenance
- `riku setup init` - Initialize Riku environment
- `riku setup ssh <pubkey>` - Add SSH key
- `riku update` - Update Riku binary
- `riku supervisor` - Start process supervisor daemon

## AI Agent Interface

Riku provides a secure SSH-based interface for AI agents (Claude, Cursor, Copilot, etc.) to perform deployment and management tasks.

### Quick Start

```bash
# Generate SSH key for AI agent
ssh-keygen -t ed25519 -C "cursor-agent" -f ~/.ssh/riku-cursor

# Add to server with scope restriction
cat ~/.ssh/riku-cursor.pub | ssh deploy@server \
  "mkdir -p ~/.ssh && echo 'command=\"riku agent --scope staging\",no-port-forwarding,no-pty' >> ~/.ssh/authorized_keys"

# Test connection
ssh -i ~/.ssh/riku-cursor deploy@server "riku agent --intro"
```

### Agent Commands

```bash
# Discovery
riku agent --intro          # Show permissions and scope
riku agent --schema         # Full command reference (JSON)
riku agent --agent-help     # Show help

# Execution (all output is JSON)
riku agent apps             # List applications
riku agent deploy myapp     # Deploy application
riku agent ps myapp         # Process status
riku agent logs myapp       # View logs
riku agent restart myapp    # Restart application
riku agent config:get myapp KEY  # Get config value
riku agent config:set myapp KEY=val  # Set config
```

### Permission Scopes

| Scope | Permissions |
|-------|-------------|
| `readonly` | View only: apps, logs, ps, config:get |
| `staging` | Deploy + readonly |
| `production` | Full access including destroy, stop |

### Example: AI Agent Workflow

```bash
# AI agent connects and checks status
ssh agent-key@server "riku agent --json ps myapp"
# → {"success":true,"data":{"app":"myapp","workers":2,"running":true}}

# AI deploys new version
ssh agent-key@server "riku agent --json deploy myapp"
# → {"success":true,"data":{"job_id":"deploy-123","status":"completed"}}

# AI verifies deployment
ssh agent-key@server "riku agent --json logs myapp --lines 10"
```

For full documentation, see [docs-site/docs/ai-agents.md](docs-site/docs/ai-agents.md).

## Supported Runtimes

### Python
- Requirements: `requirements.txt`
- Poetry: `pyproject.toml` with Poetry
- uv: `pyproject.toml` with uv

### Node.js
- Requirements: `package.json`

### Ruby
- Requirements: `Gemfile`

### Go
- Requirements: `go.mod`, `Godeps`, or `.go` files

### Java
- Maven: `pom.xml`
- Gradle: `build.gradle`

### Clojure
- Tools.deps: `deps.edn`
- Leiningen: `project.clj`

### Rust
- Requirements: `Cargo.toml` with `rust-toolchain.toml`

## Configuration

### Procfile
Define processes in `Procfile`:
```
web: python app.py
worker: python worker.py
cron: 0 2 * * * /path/to/script.sh
```

### Scaling
Control process counts in `SCALING`:
```
web=2
worker=4
```

### Environment Variables
Set environment in `ENV` file in the app's environment directory.

## Architecture

### Directory Structure
```
~/.riku/
├── apps/              # Application code
├── data/              # Persistent data
├── envs/              # Environment variables
├── repos/             # Git repositories
├── logs/              # Application logs
├── nginx/             # Nginx configurations
├── cache/             # Nginx cache files
├── workers/           # Worker process configurations
├── workers-available/ # Available worker configs
├── workers-enabled/   # Enabled worker configs (symlinks)
├── acme/              # ACME/Let's Encrypt certificates
└── plugins/           # Plugin executables
```

### Process Management
The supervisor daemon monitors TOML configuration files in `workers-enabled/` and manages the corresponding processes. When configurations are added, modified, or removed, the supervisor starts, stops, or restarts processes accordingly.

### Environment Variables

Riku supports comprehensive environment variable configuration via the `ENV` file in `~/.riku/envs/<app>/ENV`:

#### Runtime Settings
```bash
PIKU_AUTO_RESTART=true      # Auto-restart workers on deploy (default: true)
```

#### Node.js Settings
```bash
NODE_VERSION=18.17.0        # Install specific Node.js version via nodeenv
NODE_PACKAGE_MANAGER=yarn   # Use yarn or pnpm instead of npm
```

#### Network Settings
```bash
BIND_ADDRESS=127.0.0.1      # IP address for workers to bind
DISABLE_IPV6=true           # Disable IPv6 in nginx
```

#### Worker Management
```bash
RIKU_WORKER_TIMEOUT=3600       # Kill unresponsive workers after N seconds
RIKU_WORKER_GRACE_PERIOD=60    # Graceful shutdown period in seconds
RIKU_MAX_RESTARTS=10           # Max restart attempts before marking as failed
```

#### Nginx Settings
```bash
NGINX_SERVER_NAME=example.com          # Domain name for your app
NGINX_HTTPS_ONLY=true                  # Redirect HTTP to HTTPS
NGINX_STATIC_PATHS=/static:public      # Serve static files directly
NGINX_INCLUDE_FILE=custom.conf         # Include custom nginx config
NGINX_ALLOW_GIT_FOLDERS=false          # Allow access to .git folders
NGINX_CATCH_ALL=index.html             # Catch-all for SPA routing
NGINX_CLOUDFLARE_ACL=true              # Restrict to Cloudflare IPs
```

#### Nginx Caching
```bash
NGINX_CACHE_PREFIXES=/api,/images      # URL prefixes to cache
NGINX_CACHE_SIZE=1                     # Cache size in GB
NGINX_CACHE_TIME=3600                  # Cache validity (seconds)
NGINX_CACHE_EXPIRY=86400               # Cache expiry (seconds)
```

#### ACME/SSL
```bash
ACME_ROOT_CA=letsencrypt.org           # Certificate authority
```

> **Note:** For uWSGI-specific variables from the original Piku, see `docs/ENV.md` for deprecated alternatives. Riku uses a custom supervisor, so uWSGI variables are not applicable.

### Nginx Integration
Riku generates nginx configuration files based on application requirements and environment variables, supporting various deployment scenarios including HTTP, HTTPS, static file serving, and reverse proxy configurations.

## Plugin System

Plugins are executable files placed in `~/.riku/plugins/`. They can be invoked as subcommands to extend Riku's functionality. Plugins are shell scripts or binaries that can interact with the Riku environment and application data.

## Cron Jobs

Cron jobs can be defined in the Procfile using the `cron` prefix:
```
cron: 0 2 * * * /path/to/script.sh
```

These are managed by the supervisor's cron scheduler.

## Development

### Building from Source
```bash
git clone https://github.com/dreygur/riku.git
cd riku
cargo build
```

### Running Tests
```bash
# Run all tests
cargo test

# Run deployment tests
./tests/deploy/test-all.sh

# Quick test with a sample app
./tests/deploy/quick-test.sh myapp node
```

### Testing Deployments
```bash
# Create a test app
./tests/deploy/quick-test.sh test-node node
cd /tmp/riku-quick-test-*/
git init && git add . && git commit -m "test"
git remote add riku deploy@your-server:test-node
git push riku master
```

### Contributing
1. Fork the repository
2. Create a feature branch
3. Add your feature or fix
4. Add tests if applicable
5. Ensure `cargo test` and `cargo clippy` pass
6. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Support

For support, please open an issue in the GitHub repository.
