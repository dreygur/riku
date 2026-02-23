# Installation

## TL;DR

To install Riku on your server, `ssh` in as `root` and run:

```bash
# Download the latest release binary
curl -LO https://github.com/dreygur/riku/releases/latest/download/riku-linux-amd64.tar.gz
tar -xzf riku-linux-amd64.tar.gz
chmod +x riku

# Run init (automatically installs riku to ~/.local/bin)
./riku init

# Create deploy user
sudo adduser --disabled-password --gecos '' deploy
sudo su - deploy

# Add your SSH public key
riku setup ssh ~/.ssh/id_rsa.pub
```

After running `riku init`, the binary will be installed to `~/.local/bin/riku` and you can use `riku` from anywhere.

## Installation Methods

There are several ways to install Riku:

1. **Release binary** - Download pre-built binary (recommended)
2. **Build from source** - Follow the manual installation guide below
3. **Cloud-init** - Automatic VPS setup (see `cloud-init` repository)
4. **Ansible** - Use the Ansible playbook (coming soon)
5. **Docker** - Run Riku in a container (experimental)

## System Requirements

### Minimum Requirements
- **CPU**: 1 core (500 MHz+)
- **RAM**: 256 MB (512 MB recommended)
- **Storage**: 50 MB for Riku + app dependencies
- **OS**: Linux (Debian/Ubuntu/RHEL/Arch)

### Required Software
- **Rust** (for building from source)
- **Git** (for deployments)
- **Nginx** (for reverse proxy)
- **SSH** (for remote access)

### Optional Software
- **Node.js** (for Node.js apps)
- **Python 3** (for Python apps)
- **Ruby** (for Ruby apps)
- **Go** (for Go apps)

## Manual Installation

### Step 1: Install Rust

Riku is written in Rust. You need Rust to build it:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Step 2: Build Riku

```bash
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
```

### Step 3: Install the Binary

```bash
sudo cp target/release/riku /usr/local/bin/
sudo chmod +x /usr/local/bin/riku
```

### Step 4: Create Deploy User

Riku requires a separate user account:

```bash
sudo adduser --disabled-password --gecos '' deploy
sudo su - deploy
```

### Step 5: Initialize Riku

```bash
riku setup init
```

This creates the directory structure:
```
~/.riku/
├── apps/              # Application code
├── envs/              # Environment variables
├── repos/             # Git repositories
├── logs/              # Application logs
├── nginx/             # Nginx configurations
├── cache/             # Nginx cache
├── workers/           # Worker configurations
├── workers-available/ # Available worker configs
├── workers-enabled/   # Enabled worker configs
└── plugins/           # Plugins
```

### Step 6: Set Up SSH Access

**On your local machine**, generate an SSH key if you don't have one:

```bash
ssh-keygen -t ed25519 -C "riku@your-email.com"
```

Copy your public key to the server:

```bash
ssh-copy-id deploy@your-server
```

Or manually add it:

```bash
# On the server
riku setup ssh ~/.ssh/id_rsa.pub
```

### Step 7: Verify Installation

```bash
riku --help
```

You should see the Riku help message.

## Platform-Specific Instructions

### Ubuntu/Debian

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y git nginx curl build-essential

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and install Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
sudo cp target/release/riku /usr/local/bin/

# Create user
sudo adduser --disabled-password --gecos '' deploy
sudo su - deploy

# Initialize
riku init
```

### CentOS/RHEL

```bash
# Install EPEL repository
sudo yum install -y epel-release

# Install dependencies
sudo yum install -y git nginx curl gcc

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and install Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
sudo cp target/release/riku /usr/local/bin/

# Create user
sudo adduser -m deploy
sudo su - deploy

# Initialize
riku init
```

### Arch Linux

```bash
# Install dependencies
sudo pacman -S git nginx curl rust

# Build and install Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release
sudo cp target/release/riku /usr/local/bin/

# Create user
sudo useradd -m deploy
sudo su - deploy

# Initialize
riku init
```

## Post-Installation

### Enable Nginx

```bash
# On your local machine (as root or with sudo)
sudo systemctl enable nginx
sudo systemctl start nginx
```

### Configure Firewall

Allow SSH and HTTP/HTTPS:

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow OpenSSH
sudo ufw allow 'Nginx Full'
sudo ufw enable

# CentOS/RHEL (firewalld)
sudo firewall-cmd --permanent --add-service=ssh
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --reload
```

### Set Up DNS

Point your domain to your server's IP address:

```
A record: your-domain.com → your.server.ip
A record: *.your-domain.com → your.server.ip
```

## Testing Your Installation

Create a test app:

```bash
# Create test directory
mkdir ~/test-app && cd ~/test-app
git init

# Create a simple app
echo 'web: python3 -m http.server $PORT' > Procfile
echo 'NGINX_SERVER_NAME=test.example.com' > ENV

# Commit and deploy
git add .
git commit -m "test"
git remote add riku deploy@your-server:test-app
git push riku master
```

## Upgrading Riku

To upgrade to the latest version:

```bash
# Pull latest changes
cd ~/riku
git pull

# Rebuild
cargo build --release

# Install new binary
cp target/release/riku /usr/local/bin/

# Restart supervisor (if running as user service)
systemctl --user restart riku
```

## Uninstalling

To remove Riku:

```bash
# Stop services
systemctl --user stop riku
systemctl --user disable riku

# Remove binary
sudo rm /usr/local/bin/riku

# Remove Riku data (WARNING: deletes all apps!)
rm -rf ~/.riku

# Remove user (optional)
sudo userdel -r deploy
```

## Troubleshooting

### Riku command not found

Ensure `/usr/local/bin` is in your PATH:

```bash
export PATH=$PATH:/usr/local/bin
```

### Permission denied errors

Make sure you're running as the deploy user:

```bash
su - deploy
```

### Build fails

Ensure you have the latest Rust:

```bash
rustup update
```

### Nginx configuration errors

Test nginx configuration:

```bash
sudo nginx -t
```

## Getting Help

- **Documentation**: See `docs/` directory
- **Issues**: Open an issue on GitHub
- **Examples**: Check `examples/` for sample apps

## Next Steps

After installation:
1. Read `docs/ENV.md` for environment variable configuration
2. Check `docs/PLUGINS.md` for extending Riku
3. Deploy your first app!
