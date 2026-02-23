# Riku - The Smallest PaaS

**The smallest PaaS you've ever seen (Rust edition)**

Riku is a complete Rust port of the [Piku](https://github.com/piku/piku) micro-PaaS, providing Heroku-like git push deployments to small servers.

[![Build Status](https://github.com/dreygur/riku/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/dreygur/riku/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://rustup.rs/)

---

## Quick Start

### 1. Install Riku

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
sudo cp target/release/riku /usr/local/bin/
```

### 2. Initialize Your Server

```bash
# Create deploy user
sudo adduser --disabled-password --gecos '' deploy
sudo su - deploy

# Initialize Riku
riku init

# Add your SSH key
riku setup ssh ~/.ssh/id_rsa.pub
```

### 3. Deploy Your First App

```bash
# Create your app
mkdir myapp && cd myapp
git init

# Create a simple Node.js app
echo '{"name":"test","scripts":{"start":"node server.js"}}' > package.json
echo 'console.log("Hello from port", process.env.PORT)' > server.js
echo 'web: node server.js' > Procfile

# Deploy
git add . && git commit -m "Initial commit"
git remote add riku deploy@your-server:myapp
git push riku master
```

---

## Features

| Feature | Description |
|---------|-------------|
| **Git Push Deploy** | Deploy with `git push` - just like Heroku |
| **Multi-Language** | Python, Node.js, Ruby, Go, Java, Clojure, Rust |
| **Process Supervisor** | Custom Rust supervisor (no uWSGI needed) |
| **Auto Scaling** | Scale workers with `SCALING` file |
| **Nginx Integration** | Automatic config generation |
| **SSL/HTTPS** | Built-in ACME support |
| **Plugin System** | Extend with server & client plugins |
| **Cron Jobs** | Schedule tasks in Procfile |
| **Zero Downtime** | Graceful restarts & rolling deploys |

---

## Resource Usage

| Component | Memory | Storage |
|-----------|--------|---------|
| Riku Supervisor | 10-30 MB | ~8 MB |
| Per App Process | 10-200 MB | 10-500 MB |
| Nginx | 5-15 MB | ~5 MB |

**Minimum Requirements:**
- CPU: 1 core (500 MHz+)
- RAM: 256 MB (512 MB recommended)
- Storage: 50 MB + app dependencies

---

## Documentation

| Section | Description |
|---------|-------------|
| [Installation](installation.md) | Install Riku on your server |
| [Quick Start](quick-start.md) | Deploy your first app in 5 minutes |
| [FAQ](faq.md) | Common questions answered |
| [GitHub](https://github.com/dreygur/riku) | Source code and issues |

---

## Supported Runtimes

=== "Python"
    ```bash
    # requirements.txt
    flask>=2.0.0
    
    # Procfile
    web: gunicorn app:app
    ```

=== "Node.js"
    ```bash
    # package.json
    {
      "scripts": {
        "start": "node server.js"
      }
    }
    
    # Procfile
    web: node server.js
    ```

=== "Go"
    ```bash
    # go.mod
    module example.com/myapp
    
    # Procfile
    web: ./server
    ```

=== "Ruby"
    ```bash
    # Gemfile
    source 'https://rubygems.org'
    gem 'puma'
    
    # Procfile
    web: bundle exec puma
    ```

=== "Rust"
    ```bash
    # Cargo.toml
    [package]
    name = "myapp"
    
    # Procfile
    web: ./target/release/myapp
    ```

---

## Architecture

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│ Git Client  │───▶│ Riku Server  │───▶│   Apps      │
│             │    │              │    │             │
│ git push    │    │  Supervisor  │    │  Managed by │
│ deployments │    │  (Rust)      │    │  Supervisor │
└─────────────┘    │              │    │             │
                   │  Nginx       │    └─────────────┘
                   │  (Reverse    │
                   │   Proxy)     │
                   └──────────────┘
```

---

## 🤝 Contributing

We welcome contributions! Here's how you can help:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/my-feature`)
3. **Make** your changes
4. **Add** tests for new features
5. **Run** `cargo test` and `cargo clippy`
6. **Submit** a pull request

See [CONTRIBUTING.md](https://github.com/dreygur/riku/blob/main/CONTRIBUTING.md) for detailed guidelines.

---

## 📜 License

Riku is released under the [MIT License](https://opensource.org/licenses/MIT).

---

## Links

- **GitHub Repository**: [github.com/dreygur/riku](https://github.com/dreygur/riku)
- **Original Piku**: [github.com/piku/piku](https://github.com/piku/piku)
- **Rust Documentation**: [doc.rust-lang.org](https://doc.rust-lang.org/)
- **Report Issues**: [GitHub Issues](https://github.com/dreygur/riku/issues)

---

## Why Riku?

| Riku (Rust) | Original Piku (Python) |
|-------------|------------------------|
| Single binary | Requires Python runtime |
| No runtime dependencies | Python 3 required |
| Memory safe | Garbage collected |
| Compile-time errors | Runtime errors |
| ~30 MB memory footprint | ~100+ MB footprint |
| Fast startup | Slower startup |

**Riku stands on the shoulders of giants.** We thank the Piku team for creating the original micro-PaaS that inspired this Rust port.

---

*Ready to deploy? Check out the [Quick Start Guide](quick-start.md)!*
