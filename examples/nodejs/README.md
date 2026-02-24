# Node.js Example for Riku

A simple Express.js application demonstrating how to deploy Node.js apps to Riku.

## Quick Start

### 1. Deploy to Riku

```bash
# From the examples/nodejs directory
git init
git add .
git commit -m "Initial commit"

# Add your Riku server as remote
git remote add riku deploy@your-server:nodejs-example

# Deploy
git push riku main
```

### 2. Set Environment Variables (Optional)

```bash
# Set custom port
riku config:set nodejs-example PORT=3000

# Set Node environment
riku config:set nodejs-example NODE_ENV=production
```

### 3. Access Your App

```bash
# Get the app URL (if domain is configured)
riku config:get nodejs-example NGINX_SERVER_NAME

# Or access via server IP
curl http://your-server-ip
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Welcome message |
| `/health` | GET | Health check (used by Riku) |
| `/env` | GET | Environment information |
| `/echo` | POST | Echo request body |

### Example Requests

```bash
# Health check
curl http://localhost:3000/health

# Get environment info
curl http://localhost:3000/env

# Test echo endpoint
curl -X POST http://localhost:3000/echo \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello Riku!"}'
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 3000 | Server port |
| `NODE_ENV` | development | Environment mode |
| `HOSTNAME` | - | Auto-set by Riku |

### Scaling

Create a `SCALING` file to run multiple instances:

```
web=4
```

Then commit and push:

```bash
echo "web=4" > SCALING
git add SCALING
git commit -m "Scale to 4 workers"
git push riku main
```

## Project Structure

```
nodejs/
├── package.json      # Dependencies and scripts
├── server.js         # Main application
├── Procfile          # Riku process definition
└── README.md         # This file
```

## Local Development

```bash
# Install dependencies
npm install

# Run locally
npm start

# Run with custom port
PORT=3001 npm start
```

## Features Demonstrated

- ✅ Express.js web server
- ✅ Health check endpoint
- ✅ Environment variables
- ✅ Graceful shutdown
- ✅ Request logging
- ✅ Error handling
- ✅ JSON API responses
- ✅ 404 handling

## Troubleshooting

### App Won't Start

```bash
# Check logs
riku logs nodejs-example

# Check process status
riku ps nodejs-example

# Restart app
riku restart nodejs-example
```

### Port Binding Error

Make sure your app binds to `0.0.0.0` and uses the `PORT` environment variable:

```javascript
const PORT = process.env.PORT || 3000;
app.listen(PORT, '0.0.0.0');
```

### Dependencies Not Installing

Ensure `package.json` is in the app root and has valid syntax:

```bash
# Validate package.json
cat package.json | jq .
```

## Next Steps

1. Add a database (PostgreSQL, MongoDB, etc.)
2. Add authentication
3. Add file uploads
4. Add WebSocket support
5. Deploy to production with SSL

## See Also

- [Riku Documentation](https://dreygur.github.io/riku/)
- [Node.js Runtime Docs](https://dreygur.github.io/riku/runtimes/#nodejs)
- [Environment Variables](https://dreygur.github.io/riku/env/)
