# Go Example Application for Riku

A simple Go web application using the Echo framework.

## Quick Start

```bash
# Copy this example
cp -r examples/golang ~/my-go-app
cd ~/my-go-app

# Initialize git
git init
git add .
git commit -m "Initial commit"

# Deploy (replace with your server)
git remote add riku deploy@your-server:my-go-app
git push riku master
```

## Files

- `server.go` - Main application code
- `go.mod` - Go module definition
- `Procfile` - Process definition for Riku
- `ENV` - Environment variables
- `SCALING` - Worker scaling configuration

## Configuration

### Environment Variables

Edit `ENV` to configure your app:

```bash
# Domain name
NGINX_SERVER_NAME=example.com

# Enable HTTPS
NGINX_HTTPS_ONLY=true

# Worker settings
BIND_ADDRESS=127.0.0.1
```

### Scaling

Edit `SCALING` to scale workers:

```bash
web=2
```

## Local Development

```bash
# Install dependencies
go mod download

# Run locally
PORT=8080 go run server.go

# Build for production
GOOS=linux GOARCH=amd64 go build -o server
```

## Deployment Notes

Riku will automatically:
1. Detect the Go runtime (via `go.mod` or `Godeps/`)
2. Build the Go binary
3. Deploy with the configured workers

## Troubleshooting

**Build fails:**
```bash
# Check Go version
go version

# Update dependencies
go mod tidy
```

**App won't start:**
```bash
# Check logs
riku logs my-go-app

# Restart app
riku restart my-go-app
```

## See Also

- [Riku Documentation](../../docs/)
- [Environment Variables](../../docs/ENV.md)
- [Go Deployment Guide](../../docs/FAQ.md)
