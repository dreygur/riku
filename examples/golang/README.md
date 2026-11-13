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
git remote add riku riku@edge.example:my-go-app
git push riku main
```

## Files

- `server.go` - Main application code
- `go.mod` - Go module definition
- `Procfile` - Process definition for Riku

## Configuration

### Settings

A domain and HTTPS are riku settings. Add a `riku.toml` next to `server.go`:

```toml
[routing]
domains = ["example.com"]
https_only = true
```

### Scaling

`web` runs one copy. Scale it on the server, which redeploys the app:

```bash
riku ps my-go-app --scale web=2
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
1. Detect the Go runtime (via `go.mod` or a top-level `.go` file)
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

- [Riku Documentation](../../docs/docs/)
- [Environment Variables](../../docs/docs/env.md)
- [FAQ](../../docs/docs/faq.md)
