# Quick Start Guide

Get Riku up and running in 5 minutes!

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

## Step 2: Create Deploy User

```bash
# Create a dedicated user for deployments
sudo adduser --disabled-password --gecos '' deploy
sudo su - deploy
```

---

## Step 3: Initialize Riku

```bash
# Initialize Riku directory structure
riku init

# This creates:
# ~/.riku/apps/     - Application code
# ~/.riku/repos/    - Git repositories
# ~/.riku/envs/     - Environment variables
# ~/.riku/logs/     - Application logs
# ~/.riku/nginx/    - Nginx configurations
# ~/.riku/workers/  - Worker configurations
```

---

## Step 4: Set Up SSH Access

### On Your Local Machine

Generate an SSH key if you don't have one:

```bash
ssh-keygen -t ed25519 -C "your-email@example.com"
```

### Add Your Key to the Server

```bash
# Copy your public key to the server
ssh-copy-id deploy@your-server-ip

# Or manually add it
riku setup ssh ~/.ssh/id_ed25519.pub
```

---

## Step 5: Install Nginx

Riku requires nginx as a reverse proxy:

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

## Step 6: Deploy Your First App

### Create a Simple App

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
git remote add riku deploy@your-server-ip:myapp

# Deploy
git add . && git commit -m "Initial commit"
git push riku main
```

---

## Step 7: Access Your App

Your app should now be running! Access it at:

```
http://your-server-ip
```

---

## Next Steps

| Task | Command |
|------|---------|
| View logs | `riku logs myapp` |
| Scale workers | `echo "web=4" > SCALING && git push riku main` |
| Set env vars | `riku config:set myapp KEY=value` |
| Restart app | `riku restart myapp` |
| Stop app | `riku stop myapp` |

---

## Configure Domain (Optional)

### Set DNS

Point your domain to your server:

```
A record: example.com → your-server-ip
A record: *.example.com → your-server-ip
```

### Configure Riku

```bash
# Set domain name
riku config:set myapp NGINX_SERVER_NAME=example.com

# Enable HTTPS (after getting SSL cert)
riku config:set myapp NGINX_HTTPS_ONLY=true
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
ssh deploy@your-server-ip

# Check Riku logs
ssh deploy@your-server-ip "tail ~/.riku/logs/myapp/*.log"
```

---

## What's Next?

- [Installation Guide](installation.md) - Detailed installation instructions
- [FAQ](faq.md) - Common questions
- [GitHub Repository](https://github.com/dreygur/riku) - Source code and issues
- [Original Piku](https://github.com/piku/piku) - The original Python version

---

*Congratulations! You've deployed your first app with Riku!*
