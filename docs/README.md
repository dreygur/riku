# Riku Documentation

Welcome to the Riku documentation! This directory contains comprehensive guides for using and extending Riku.

## Quick Start

New to Riku? Start here:

1. **[Installation Guide](INSTALL.md)** - Install Riku on your server
2. **[Environment Variables](ENV.md)** - Configure your deployments
3. **[FAQ](FAQ.md)** - Common questions and answers

## Documentation Index

### Core Documentation

| Document | Description |
|----------|-------------|
| [INSTALL.md](INSTALL.md) | Installation instructions for all platforms |
| [ENV.md](ENV.md) | Complete environment variable reference |
| [FAQ.md](FAQ.md) | Frequently asked questions |
| [PLUGINS.md](PLUGINS.md) | Plugin system guide |
| [../README.md](../README.md) | Main project README |

### Architecture Documentation

| Document | Description |
|----------|-------------|
| [../ARCHITECTURE.md](../ARCHITECTURE.md) | System architecture overview |
| [../API.md](../API.md) | API reference |
| [../SYSTEMD.md](../SYSTEMD.md) | Systemd service configuration |

### Plans and Design

The `plans/` directory contains design documents and implementation plans for future features.

## For Users

### Getting Started
1. Install Riku following [INSTALL.md](INSTALL.md)
2. Set up your SSH key
3. Deploy your first app with `git push`

### Configuration
- Use `ENV` file for environment variables
- Use `Procfile` to define processes
- Use `SCALING` file to scale workers
- See [ENV.md](ENV.md) for all available options

### Common Tasks

```bash
# Deploy an app
git remote add riku deploy@server:appname
git push riku master

# View logs
riku logs appname

# Scale workers
echo "web=4" > SCALING
git add SCALING && git commit -m "scale"
git push riku master

# Set environment variables
riku config:set appname KEY=value

# Restart app
riku restart appname
```

## For Developers

### Building from Source

```bash
git clone https://github.com/piku/piku.git
cd piku
cargo build
```

### Running Tests

```bash
# Run all tests
cargo test

# Run deployment tests
./tests/deploy/test-all.sh

# Run Clippy lints
cargo clippy
```

### Project Structure

```
piku/
├── src/                    # Rust source code
│   ├── cli/               # Command-line interface
│   ├── config/            # Configuration handling
│   ├── deploy/            # Deployment logic
│   ├── nginx/             # Nginx configuration
│   ├── supervisor/        # Process supervisor
│   └── util/              # Utility functions
├── templates/              # Nginx templates
├── tests/                  # Test scripts
├── docs/                   # Documentation
└── examples/               # Example applications
```

### Key Components

- **Supervisor** (`src/supervisor/`) - Process management daemon
- **Deploy** (`src/deploy/`) - Runtime-specific deployment logic
- **Nginx** (`src/nginx.rs`) - Nginx configuration generation
- **Config** (`src/config.rs`) - Configuration and paths

## For Contributors

### How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes
4. Add tests for new features
5. Run `cargo test` and `cargo clippy`
6. Submit a pull request

See [CONTRIBUTING.md](../CONTRIBUTING.md) for detailed guidelines.

### Documentation Guidelines

When adding documentation:

1. Use Markdown format
2. Keep lines under 80 characters when possible
3. Include code examples
4. Update the index (this file) if adding new docs
5. Use relative links for cross-references

## Environment Variables Reference

See [ENV.md](ENV.md) for the complete list. Quick reference:

### Runtime
- `PIKU_AUTO_RESTART` - Auto-restart on deploy

### Node.js
- `NODE_VERSION` - Node.js version
- `NODE_PACKAGE_MANAGER` - npm/yarn/pnpm

### Network
- `BIND_ADDRESS` - Worker bind address
- `DISABLE_IPV6` - Disable IPv6

### Worker Management
- `RIKU_WORKER_TIMEOUT` - Worker timeout
- `RIKU_WORKER_GRACE_PERIOD` - Graceful shutdown
- `RIKU_MAX_RESTARTS` - Max restart attempts
- `RIKU_WORKER_PROCESSES` - Process scaling

### Nginx
- `NGINX_SERVER_NAME` - Domain name
- `NGINX_HTTPS_ONLY` - HTTPS redirect
- `NGINX_CACHE_*` - Caching options
- `NGINX_STATIC_PATHS` - Static file paths

## Troubleshooting

### Common Issues

**App won't start:**
```bash
riku logs appname
riku restart appname
```

**Nginx errors:**
```bash
sudo nginx -t
sudo systemctl reload nginx
```

**Permission denied:**
```bash
# Ensure you're the deploy user
su - deploy
```

### Getting Help

1. Check the [FAQ](FAQ.md)
2. Review example apps in `examples/`
3. Open an issue on GitHub
4. Check logs with `riku logs appname`

## Additional Resources

- **GitHub Repository**: https://github.com/piku/piku
- **Original Piku**: https://github.com/piku/piku (Python version)
- **Rust Documentation**: https://doc.rust-lang.org/
- **Nginx Documentation**: https://nginx.org/en/docs/

## License

This documentation is part of the Riku project and is available under the MIT License.
