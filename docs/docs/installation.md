# Installation

## TL;DR

To install Riku on your server, `ssh` in with a user that has `sudo` and run:

```bash
# Download the latest release binary
curl -LO https://github.com/dreygur/riku/releases/latest/download/riku-linux-amd64.tar.gz
tar -xzf riku-linux-amd64.tar.gz
chmod +x riku

# Run init as root. It installs riku to /usr/local/bin/riku, creates the
# deploy user and authorizes your SSH keys for git push
sudo ./riku init --ssh-key ~/.ssh/authorized_keys
```

The installer script does the same download and checksum in one step. It installs `riku` to `/usr/local/bin`, so it needs root:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sudo sh
sudo riku init --ssh-key ~/.ssh/authorized_keys
```

After running `sudo riku init`, the binary is installed to `/usr/local/bin/riku`, owned by root so the deploy user cannot change it. Units, git hooks and deploy keys all name that path. A plain `riku init` (single-user mode, no system service) needs it to exist already, and is refused on a host where `sudo riku init` already ran (the system `riku.service` exists, or the deploy account is another user): it would build a second install next to the system one. Run `sudo riku init` again there instead.

`sudo riku init` also installs and starts `riku-router-helper.service`, a small root service on `/run/riku-router.sock`. It writes the nginx or Caddy site of each app (`/etc/riku/nginx`, `/etc/riku/caddy`, certificates in `/etc/riku/certs`) and reloads the router. See [Nginx](nginx.md#the-router-helper).

When docker is installed and the deploy user cannot open its socket, `sudo riku init` asks on the terminal whether to add the deploy user to the group that owns the socket, which is root-equivalent on the host. The default is no; see [settings.md](settings.md#router-and-engine).

### Upgrading riku

1. Install the new binary: the installer above, or `sudo cp riku /usr/local/bin/riku` from the release tarball.
2. Run `sudo riku init`. It rewrites the units and the router helper, sets the owner and mode of the router directories again, and restarts the supervisor when it runs another binary or the unit text changed. Its `Service` line says whether it did.

## The rk laptop client

`rk` is a separate binary for your laptop (Linux or macOS). It runs riku commands on your servers over ssh and never runs server code. The server still needs the full `riku` from the steps above.

Install it with the install script and the `--rk` flag:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh -s -- --rk
```

The script downloads the matching release build (see [Release builds](#release-builds)), checks it against the release `.sha256`, and installs to `~/.local/bin` (or `/usr/local/bin` as root). `RIKU_INSTALL_DIR` and `RIKU_VERSION` override the directory and release tag. Set them on the `sh` side of the pipe, after `sudo` when there is one, or they never reach the script: `curl -fsSL .../install.sh | sudo RIKU_VERSION=v0.1.3 sh`. Without `--rk` the same script installs `riku`, which it only does as root, into `/usr/local/bin`.

When the `.sha256` cannot be fetched (a network error, a release without one) or the host has neither `sha256sum` nor `shasum`, the script stops before installing anything. `RIKU_SKIP_CHECKSUM=1` makes it warn and install the unverified download instead: `curl -fsSL .../install.sh | sudo RIKU_SKIP_CHECKSUM=1 sh`. A checksum that is fetched and does not match always stops the install.

First use:

```bash
rk server add myserver riku@edge.example --admin you@edge.example
rk -s myserver apps
```

`rk server add` saves the server, then authorizes your public key
(`--key PATH`, else `~/.ssh/id_ed25519.pub` or `~/.ssh/id_rsa.pub`) by running
`sudo riku deploy-key add --user riku '<key>'` over the admin login (the user comes from the server address); sudo may ask for your
password. Without `--admin` it prints that command for you to run on the
server. Run inside your app's git repo, it also adds a `riku` git remote named
after the repo directory.

See [The rk client](cli.md#the-rk-client) for the full description.

## Installation Methods

There are several ways to install Riku:

1. **Release binary** - Download pre-built binary (recommended)
2. **Build from source** - Follow the manual installation guide below
3. **Cloud-init** - Automatic VPS setup (see `cloud-init` repository)
4. **Ansible** - Use the Ansible playbook (coming soon)
5. **Docker** - Run Riku in a container (experimental)

## Release builds

Every release has these Linux builds of `riku` and `rk`. The glibc builds need glibc 2.31 or newer (Debian 11, Ubuntu 20.04, Raspberry Pi OS bullseye, or later). The musl builds are static and run on any Linux, Alpine included.

| CPU | glibc build | musl build | Boards |
| --- | --- | --- | --- |
| x86_64 | `linux-amd64` | `linux-amd64-musl` | PCs, VPS |
| arm64 | `linux-arm64` | `linux-arm64-musl` | Pi 3, 4, 5, Zero 2 W on a 64 bit OS |
| armv7 | `linux-armv7` | `linux-armv7-musl` | Pi 2, 3, 4, 5 on a 32 bit OS |
| armv6 | `linux-armv6` | `linux-armv6-musl` | Pi 1, Zero, Zero W |

Asset names are `<bin>-<build>.tar.gz`, for example `riku-linux-armv6.tar.gz`. `scripts/install.sh` picks the build itself: a 64 bit kernel with a 32 bit userland gets armv7, and a host without glibc 2.31 gets musl. A 64 bit x86 kernel with a 32 bit userland has no build and is refused. To choose the C library yourself, pass `RIKU_LIBC` to `sh`, after `sudo`:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sudo RIKU_LIBC=musl sh
```

## System Requirements

### Minimum Requirements
- **CPU**: 1 core (500 MHz+)
- **RAM**: 256 MB (512 MB recommended)
- **Storage**: 50 MB for Riku + app dependencies
- **OS**: Linux (Debian/Ubuntu/RHEL/Arch)

### Supported distros

`sudo riku init` creates the deploy user and its groups with the tools the host has:

- Debian, Ubuntu, Fedora, RHEL, Arch and openSUSE: shadow-utils (`useradd`, `groupadd`, `usermod`).
- Alpine: BusyBox (`adduser`, `addgroup`, `chpasswd`) when shadow-utils is not installed. bash is not needed: the deploy user's login shell is `/bin/sh`, and the git hook and bundled plugins are POSIX `sh`. A deploy user an older init made keeps the shell it was given.
- NixOS: riku changes no users or groups. Init prints the `configuration.nix` lines to add (the deploy user, its group, `riku-sockets` and the extra groups it needs) and stops; run `nixos-rebuild switch`, then `sudo riku init` again.

With neither tool set, init stops and names what to install.

### Required Software
- **Rust** (for building from source)
- **Git** (for deployments)
- **Nginx** (for reverse proxy)
- **SSH** (for remote access)
- **python3-venv** (for Python apps)

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

### Step 3: Deploy User

Riku runs as a separate user account, `riku` by default. `sudo riku init` creates it when it is missing, so there is no account to create by hand. For another name, set `RIKU_USER` on the init command in step 4:

```bash
sudo RIKU_USER=deployer ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
```

App repos start on `main`. Init writes that as `init.defaultBranch` in the deploy user's `~/.gitconfig`, so push the branch with that name. For another branch, run init with `RIKU_DEFAULT_BRANCH`:

```bash
sudo RIKU_DEFAULT_BRANCH=master ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
```

Without `RIKU_DEFAULT_BRANCH`, init keeps a default branch the deploy user already set. Repos created before the change keep the branch they started with.

Only a push to the app repo's default branch deploys. Pushes to other branches and tags are stored in the repo and skipped with a message, and deleting a branch never deploys. If the repo's default branch does not exist yet (a repo created on `master` that only ever got `main`), the first branch pushed becomes its default.

### Step 4: Install and Initialize Riku

```bash
sudo ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
```

On a fresh install, init first picks the router from what is installed: the
only one of nginx and Caddy it finds, or a prompt when both are and init has a
terminal. Without a terminal and with both installed, it stops and asks for
`RIKU_ROUTER=nginx` or `RIKU_ROUTER=caddy`; with neither, it stops and says to
install one. `RIKU_ROUTER` always wins, and a rerun keeps the router already
stored. When none is stored but `~/.riku` already exists and nginx is
installed, init uses nginx.

It then copies the binary to `/usr/local/bin/riku`, creates the deploy user, installs the units and the router helper, and creates the directory structure in the deploy user's home:
```
~/.riku/
├── apps/              # Application code
├── envs/              # Environment variables
├── repos/             # Git repositories
├── logs/              # Application logs
├── workers/           # Worker configurations
├── workers-available/ # Available worker configs
├── workers-enabled/   # Enabled worker configs
└── plugins/           # Plugins
```

### Step 5: Set Up SSH Access

`--ssh-key` in step 4 authorized the keys in the file you gave it for `git push`. To add one key later, run on the server:

```bash
sudo riku deploy-key add 'ssh-ed25519 AAAA... me@laptop'
```

It accepts exactly one key line and keeps only the key type and data. From a laptop, `rk server add ... --admin <login>@<host>` runs it for you. If you have no key yet, generate one **on your local machine** first:

```bash
ssh-keygen -t ed25519 -C "riku@your-email.com"
```

### Step 6: Verify Installation

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
sudo apt install -y git nginx curl build-essential python3-venv python3-full

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and install Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release

# Install, create the deploy user and authorize your SSH keys
sudo ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
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

# Install, create the deploy user and authorize your SSH keys
sudo ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
```

### Arch Linux

```bash
# Install dependencies
sudo pacman -S git nginx curl rust

# Build and install Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release

# Install, create the deploy user and authorize your SSH keys
sudo ./target/release/riku init --ssh-key ~/.ssh/authorized_keys
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
A record: example.com → 203.0.113.10
A record: *.example.com → 203.0.113.10
```

## Testing Your Installation

On the server, as the deploy user, install the runtime plugins your apps need (riku detects the runtime with them):

```bash
riku install-plugins
```

Then, on your machine, create a test app:

```bash
mkdir ~/test-app && cd ~/test-app
git init
echo 'web: python3 -m http.server $PORT' > Procfile
git add .
git commit -m "test"
git remote add riku riku@edge.example:test-app
git push riku main
```

Give it a domain and push again:

```bash
ssh riku@edge.example settings set test-app routing.domains=test.example.com
git commit --allow-empty -m "domain" && git push riku main
```

## Upgrading Riku

To upgrade to the latest version:

```bash
# Pull latest changes and rebuild
cd ~/riku
git pull
cargo build --release

# Install the new binary, rewrite units and the router helper, refresh
# the built-in runtime plugins you have not changed, and restart the
# supervisor when it runs the old binary
sudo cp target/release/riku /usr/local/bin/riku
sudo riku init
```

## Uninstalling

To remove Riku:

```bash
# Stop and disable the system units
sudo systemctl disable --now riku.service riku-router-helper.service
sudo rm -f /etc/systemd/system/riku.service /etc/systemd/system/riku-router-helper.service
sudo systemctl daemon-reload

# Remove the binary and riku's root owned configuration
sudo rm /usr/local/bin/riku
sudo rm -rf /etc/riku

# Remove Riku data (WARNING: deletes all apps!)
sudo rm -rf ~riku/.riku

# Remove user (optional)
sudo userdel -r riku
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
su - riku
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
1. Read [Environment Variables](env.md) for environment variable configuration
2. Check [Plugin System](plugins.md) for extending Riku
3. Deploy your first app.
