# Quick Start Guide

Get Riku up and running in 5 minutes.

## Prerequisites

- A Linux server (Ubuntu, Debian, CentOS, or Arch)
- SSH access to your server
- Git installed on your local machine
- A domain name (optional, for HTTPS)

---

## Step 1: Install Riku on Your Server

SSH into your server and run:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone and build Riku
git clone https://github.com/dreygur/riku.git
cd riku
cargo build --release

# Install the binary
sudo cp target/release/riku /usr/local/bin/
```

---

## Step 2: Install a Router

Riku serves apps through nginx or Caddy. Install one before `riku init`: a
fresh init uses the router it finds, asks when both are installed, and stops
when there is none.

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install -y nginx

# CentOS/RHEL
sudo yum install -y nginx

# Start nginx
sudo systemctl enable nginx
sudo systemctl start nginx
```

---

## Step 3: Create Deploy User

```bash
# Create a dedicated user for deployments
sudo adduser --disabled-password --gecos '' riku
sudo su - riku
```

---

## Step 4: Initialize Riku

```bash
# Initialize Riku directory structure
riku init

# This creates:
# ~/.riku/apps/     - Application code
# ~/.riku/repos/    - Git repositories
# ~/.riku/envs/     - Environment variables
# ~/.riku/logs/     - Application logs
# ~/.riku/nginx/    - Certificate files for the router helper
# ~/.riku/workers/  - Worker configurations
```

---

## Step 5: Set Up SSH Access

### On Your Local Machine

Generate an SSH key if you don't have one:

```bash
ssh-keygen -t ed25519 -C "your-email@example.com"
```

### Add Your Key to the Server

From your laptop, with an admin login on the server (sudo may ask for its
password):

```bash
rk server add edge riku@edge.example --admin you@edge.example
```

This runs `sudo riku deploy-key add --user riku '<your key>'` on the server and, inside
your app's git repo, adds the `riku` remote. Without `--admin` it prints that
command so you can run it on the server yourself.

---

## Step 6: Deploy Your First App

### Create a Simple Node.js App

```bash
# On your local machine
mkdir myapp && cd myapp
git init
```

### Create a Node.js App

```bash
# package.json
cat > package.json << 'EOF'
{
  "name": "myapp",
  "version": "1.0.0",
  "scripts": {
    "start": "node server.js"
  }
}
EOF

# server.js
cat > server.js << 'EOF'
const http = require('http');
const port = process.env.PORT || 3000;
const server = http.createServer((req, res) => {
  res.writeHead(200, {'Content-Type': 'text/plain'});
  res.end(`Hello from Riku! Port: ${port}\n`);
});
server.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port ${port}`);
});
EOF

# Procfile
echo 'web: node server.js' > Procfile
```

### Deploy

```bash
# Add your Riku server as a remote
git remote add riku riku@edge.example:myapp

# Deploy
git add . && git commit -m "Initial commit"
git push riku main
```

The push creates the repository in `~/.riku/repos/` on first use.

---

## Step 7: Access Your App

The push shows one step per phase and ends with a summary that holds the
app's URL, for example:

```
remote: ┌  Deploying myapp  54ba34f "Fix login"
remote: │
remote: ◇  Runtime  python
remote: │  <build output>
remote: ◇  Build  12.4s
remote: ◇  Processes  web 1
remote: ◇  Route  caddy
remote: │
remote: └  Deployed in 18.2s
remote:
remote:    App       myapp
remote:    Commit    54ba34f  Fix login
remote:    URL       http://myapp.203.0.113.10.sslip.io
remote:    Workers   web 1
remote:    Logs      rk logs myapp
```

A failed deploy names the step that failed under the build output it
streamed, and ends with `rk logs myapp --deploy` for the full log and
the `git push` that retries it. `git push` exits non-zero.

An app with no domain gets the server's fallback name, or a port of its own
when the server has none. See [App URLs](routing.md) for the three ways an
app is reached and how to pick one.

### How It Works

When you push to Riku:

1. **Git hook triggers** - Post-receive hook extracts code to `~/.riku/apps/`
2. **Runtime detection** - Riku detects Python, Node.js, Go, etc.
3. **Dependencies install** - nub install (Node), uv (Python), etc.
4. **Nginx config** - The router helper writes `/etc/riku/nginx/<app>.conf`, which nginx loads through `/etc/nginx/conf.d/riku.conf`
5. **Supervisor starts** - If not running, supervisor auto-starts and spawns workers
6. **App running**: your app is live.

> Riku automatically handles:
> - Nginx config validation and reload
> - Supervisor auto-start on first deployment

---

## Manage the Server from Your Laptop

Register the server once and the local `riku` runs commands on it over ssh:

```bash
riku server add edge riku@edge.example
riku server default edge
riku ps myapp
```

Inside a repo whose git remote is `riku@edge.example:myapp`, the server and
the app name are picked up from the remote, so `riku restart` is enough. Riku
prints `[*] edge (riku@edge.example)` on stderr so you can see where a
command went. Use `-s <name>` for another server and `--local` to run on the
current machine. `riku destroy` always needs the app name and does not ask for
confirmation.

To open the server's dashboard in your browser, register the server with an
admin login (`--admin root@edge.example`) and run `riku dashboard -s
edge`. An existing entry has no way to be edited: remove it and add it
again.

### Using `rk` instead

`rk` is a small client that only talks to servers, so it is the lighter install
for a laptop. Install it and register the server:

```bash
curl -fsSL https://raw.githubusercontent.com/dreygur/riku/main/scripts/install.sh | sh -s -- --rk
rk server add edge riku@edge.example
rk -s edge ps myapp
```

`rk` never runs server code and has no `--local`. Without a selected server it
exits with `no server selected; add one with rk server add <name> <user@host>,
or pass -s <name>`. See [The rk Client](cli.md#the-rk-client).

The full rules are in [Remote Servers](cli.md#remote-servers).

---

## Next Steps

| Task | Command |
|------|---------|
| View logs | `riku logs myapp` |
| Scale workers | `echo "web=4" > SCALING && git push riku main` |
| Set env vars | `riku config set myapp KEY=value` |
| Restart app | `riku restart myapp` |
| Stop app | `riku stop myapp` |

---

## Configure Domain (Optional)

### Set DNS

Point your domain to your server:

```
A record: example.com → 203.0.113.10
A record: *.example.com → 203.0.113.10
```

### Configure Riku

Commit a `riku.toml` next to your code (see [riku.toml](riku-toml.md)):

```toml
[routing]
domains = ["example.com"]
https_only = true   # after getting an SSL cert
```

Or set them on the server without a commit:

```bash
riku settings set myapp routing.domains=example.com routing.https_only=true
```

---

## Troubleshooting

### App Won't Start

```bash
# Check logs
riku logs myapp

# Check if app is running
riku ps myapp

# Restart
riku restart myapp
```

### Can't Access App

```bash
# Check nginx status
sudo systemctl status nginx

# Check firewall
sudo ufw status  # Ubuntu/Debian
sudo firewall-cmd --list-all  # CentOS/RHEL

# Check if port 80 is open
sudo netstat -tlnp | grep :80
```

### Deployment Fails

```bash
# Check git remote
git remote -v

# Verify SSH access
ssh riku@edge.example

# Check Riku logs
ssh riku@edge.example "tail ~/.riku/logs/myapp/*.log"
```

---

## What's Next?

- [Installation Guide](installation.md) - Detailed installation instructions
- [FAQ](faq.md) - Common questions
- [GitHub Repository](https://github.com/dreygur/riku) - Source code and issues

---

*You've deployed your first app with Riku.*
