# AI Agent Interface

Riku provides a secure SSH-based interface for AI agents (Claude, Cursor, Copilot, etc.) to perform deployment and management tasks.

---

## Overview

AI agents can connect to your Riku server via SSH and execute commands just like a human would. The interface is:

- **Secure** - Uses existing SSH authentication
- **Scoped** - Per-agent permissions via SSH keys
- **Audited** - All AI actions are logged
- **Structured** - JSON output for reliable parsing

---

## Quick Start

### 1. Generate SSH Key for AI Agent

```bash
# Create a dedicated SSH key for your AI agent
ssh-keygen -t ed25519 -C "cursor-agent@my-laptop" -f ~/.ssh/riku-cursor
```

### 2. Add Key to Riku Server

```bash
# Copy public key to server
cat ~/.ssh/riku-cursor.pub | ssh deploy@your-server \
  "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys"
```

### 3. Set Permissions

Edit `~/.ssh/authorized_keys` on the server to add restrictions:

```bash
# Restrict AI agent to specific commands and scope
command="riku agent --scope staging",no-port-forwarding,no-pty ssh-ed25519 AAAA... cursor-agent@my-laptop
```

### 4. Test Connection

```bash
# Test AI agent connection
ssh -i ~/.ssh/riku-cursor deploy@your-server "riku agent --intro"
```

---

## SSH Key Setup

### authorized_keys Format

Each line in `~/.ssh/authorized_keys` can include restrictions before the key:

```
command="riku agent --scope SCOPE",no-port-forwarding,no-pty,no-X11-forwarding ssh-ed25519 AAAA... comment
```

### Example authorized_keys Entries

```bash
# Read-only AI agent (monitoring, logs, config:get)
command="riku agent --scope readonly",no-port-forwarding,no-pty ssh-ed25519 AAAA... monitoring-agent

# Staging deployment AI agent (can deploy to staging)
command="riku agent --scope staging",no-port-forwarding,no-pty ssh-ed25519 BBBB... cursor-staging

# Production AI agent (full access, use with caution)
command="riku agent --scope production",no-port-forwarding ssh-ed25519 CCCC... claude-production

# Agent with specific app restriction (advanced)
command="riku agent --scope staging --app myapp",no-port-forwarding,no-pty ssh-ed25519 DDDD... limited-agent
```

### SSH Key Options Explained

| Option | Purpose |
|--------|---------|
| `command="..."` | Restrict to specific command with scope |
| `no-port-forwarding` | Prevent SSH tunneling |
| `no-pty` | No interactive shell (required for automation) |
| `no-X11-forwarding` | Disable X11 forwarding |
| `from="IP"` | Only allow from specific IP addresses |

**Recommended minimum configuration:**
```bash
command="riku agent --scope staging",no-port-forwarding,no-pty,no-X11-forwarding ssh-ed25519 ...
```

---

## Authentication & Permissions

### SSH Key Scopes

Configure permissions in `~/.ssh/authorized_keys`:

| Scope | Permissions |
|-------|-------------|
| `readonly` | `apps`, `logs`, `ps`, `config:get`, `config:show`, `stats` |
| `staging` | All readonly + `deploy`, `restart`, `run`, `config:set`, `config:unset` |
| `production` | Full access including `destroy`, `stop` |

---

## Command Reference

### Get Started

```bash
# Introduction (shows permissions and hints)
ssh deploy@server "riku agent --intro"

# Full command schema (JSON)
ssh deploy@server "riku agent --schema"

# Help for specific command
ssh deploy@server "riku agent --help deploy"
```

### Available Commands

| Command | Description | Confirmation Required |
|---------|-------------|----------------------|
| `apps` | List deployed applications | No |
| `deploy <app>` | Deploy an application | No |
| `destroy <app>` | Permanently remove an application | Yes |
| `config:get <app> <key>` | Get a configuration value | No |
| `config:set <app> KEY=value` | Set configuration values | Yes (critical keys) |
| `config:show <app>` | Show all configuration | No |
| `logs <app> [process]` | View application logs | No |
| `ps <app>` | Show process status | No |
| `restart <app> [process]` | Restart an application | No |
| `stop <app> [process]` | Stop an application | Yes (production) |
| `run <app> <command>` | Run command in app context | No |

---

## JSON Output Mode

AI agents should use `--json` flag for structured, parseable output:

```bash
# Plain text (human-readable)
ssh deploy@server "riku ps myapp"
# Output: web: 2/2 running

# JSON (AI agent mode)
ssh deploy@server "riku --json ps myapp"
# Output: {"processes": {"web": {"running": 2, "desired": 2}}}
```

### Response Format

All JSON responses follow this structure:

**Success:**
```json
{
  "success": true,
  "data": {
    "processes": {
      "web": {"running": 2, "desired": 2}
    }
  },
  "message": "Command executed successfully",
  "confirmation_required": false,
  "job_id": null
}
```

**Error:**
```json
{
  "success": false,
  "error": {
    "code": "APP_NOT_FOUND",
    "message": "Application 'myapp' not found"
  }
}
```

**Confirmation Required:**
```json
{
  "success": false,
  "confirmation_required": true,
  "action": "destroy",
  "app": "myapp",
  "risk": "high",
  "message": "This will permanently delete myapp and all its data",
  "confirm_token": "abc123xyz"
}
```

---

## Confirmation Flow

Destructive operations require human confirmation:

### Operations Requiring Confirmation

| Action | When |
|--------|------|
| `destroy` | Always |
| `stop` | Production apps |
| `config:set` | Critical keys (DATABASE_URL, SECRET_KEY) |
| `deploy` | Production (optional, configurable) |

### Example Flow

```bash
# AI attempts to destroy app
ssh deploy@server "riku --json destroy myapp"

# Server responds with confirmation required
{
  "confirmation_required": true,
  "confirm_token": "abc123xyz",
  "message": "Type 'yes' to confirm deletion of myapp"
}

# AI asks human, human confirms
# AI sends confirmation
ssh deploy@server "riku --json destroy myapp --confirm abc123xyz"
```

---

## Example Workflows

### Deploy Application

```bash
# Check current apps
ssh deploy@server "riku --json apps"

# Deploy application
ssh deploy@server "riku --json deploy myapp"

# Check deployment status
ssh deploy@server "riku --json ps myapp"

# View logs
ssh deploy@server "riku --json logs myapp"
```

### Update Configuration

```bash
# Get current config
ssh deploy@server "riku --json config:get myapp DATABASE_URL"

# Set new config (may require confirmation)
ssh deploy@server "riku --json config:set myapp DATABASE_URL=postgres://new-host/db"

# If confirmation required, response includes confirm_token
# AI asks human, then sends:
ssh deploy@server "riku --json config:set myapp DATABASE_URL=... --confirm <token>"
```

### Monitor Application

```bash
# Check process status
ssh deploy@server "riku --json ps myapp"

# Stream logs (last 100 lines)
ssh deploy@server "riku --json logs myapp --lines 100"

# Get configuration
ssh deploy@server "riku --json config:show myapp"
```

### Multi-Step Operation

```bash
# 1. Check current state
STATE=$(ssh deploy@server "riku --json ps myapp")

# 2. Stop app (requires confirmation for production)
ssh deploy@server "riku --json stop myapp"

# 3. Update config
ssh deploy@server "riku --json config:set myapp NEW_RELIC_KEY=xyz"

# 4. Restart app
ssh deploy@server "riku --json restart myapp"

# 5. Verify healthy
ssh deploy@server "riku --json ps myapp"
```

---

## Job IDs for Long Operations

Long-running operations return a job ID:

```bash
# Start deployment
ssh deploy@server "riku --json deploy myapp"
# Response: {"job_id": "deploy-123", "status": "running"}

# Check job status
ssh deploy@server "riku --json job-status deploy-123"
# Response: {"job_id": "deploy-123", "status": "completed"}
```

**Operations that return job IDs:**
- `deploy` - Full application deployment
- `destroy` - App removal with cleanup
- Large `config:set` operations

---

## Error Handling

### Common Error Codes

| Code | Meaning |
|------|---------|
| `APP_NOT_FOUND` | Application doesn't exist |
| `PERMISSION_DENIED` | Agent lacks required permission |
| `CONFIRMATION_REQUIRED` | Human confirmation needed |
| `APP_LOCKED` | Another operation in progress |
| `INVALID_COMMAND` | Unknown command |
| `INVALID_PARAMETERS` | Missing or invalid parameters |

### Handling Errors

```python
# Pseudocode for AI agent
result = ssh_run("riku --json deploy myapp")

if not result["success"]:
    error = result["error"]
    
    if error["code"] == "APP_NOT_FOUND":
        # App doesn't exist, create it first
        ssh_run("riku --json create myapp")
        
    elif error["code"] == "CONFIRMATION_REQUIRED":
        # Ask human for confirmation
        token = result["confirm_token"]
        if human_confirms():
            ssh_run(f"riku --json deploy myapp --confirm {token}")
            
    elif error["code"] == "APP_LOCKED":
        # Wait and retry
        sleep(5)
        retry()
```

---

## Audit Logging

All AI agent actions are logged:

```
2026-02-23 14:30:00 [AI] cursor-agent@rakib deployed myapp (v42)
2026-02-23 14:31:00 [AI] cursor-agent@rakib set DATABASE_URL on myapp
2026-02-23 14:32:00 [AI] cursor-agent@rakib restarted myapp
2026-02-23 14:33:00 [AI] claude-agent@workstation viewed logs myapp
```

**View audit logs:**
```bash
ssh deploy@server "riku agent --audit-log"
```

---

## Rate Limiting

AI agents are rate-limited to prevent abuse:

| Scope | Rate Limit |
|-------|------------|
| `readonly` | 60 commands/minute |
| `staging` | 30 commands/minute |
| `production` | 20 commands/minute |

Exceeding rate limit returns:
```json
{
  "success": false,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Too many requests. Try again in 30 seconds"
  }
}
```

---

## Best Practices

### For AI Agent Developers

1. **Cache the schema** - Fetch `--schema` once, cache locally
2. **Use JSON mode** - Always use `--json` for parseable output
3. **Handle confirmations** - Implement confirmation flow for destructive actions
4. **Check permissions** - Use `--intro` to know your limits
5. **Respect rate limits** - Add delays between commands
6. **Log your actions** - Keep local audit trail

### For Server Administrators

1. **Use scoped keys** - Give AI agents minimum required permissions
2. **Review audit logs** - Regularly check AI agent actions
3. **Rotate keys** - Change AI agent SSH keys periodically
4. **Monitor rate limits** - Adjust if legitimate agents hit limits
5. **Test in staging** - Verify AI agent behavior before production

---

## Security Considerations

### SSH Key Security

- Generate dedicated keys for each AI agent
- Never share AI agent keys between systems
- Rotate keys periodically
- Revoke keys immediately if compromised

### Permission Scoping

- Start with `readonly` scope
- Grant additional permissions only as needed
- Use `no-port-forwarding,no-pty` restrictions
- Consider IP-based restrictions with `from="IP"`

### Audit & Monitoring

- Review audit logs regularly
- Set up alerts for destructive actions
- Monitor rate limit violations
- Track failed authentication attempts

---

## Troubleshooting

### Permission Denied

```bash
# Check your permissions
ssh deploy@server "riku agent --intro"

# Verify authorized_keys entry
ssh deploy@server "cat ~/.ssh/authorized_keys"
```

### Command Not Found

```bash
# Get full schema
ssh deploy@server "riku agent --schema"

# Check command help
ssh deploy@server "riku agent --help <command>"
```

### Confirmation Not Working

```bash
# Confirmations expire after 5 minutes
# Request a new confirmation token
ssh deploy@server "riku --json destroy myapp"
# Use the new confirm_token immediately
```

### Rate Limited

```bash
# Wait and retry
sleep 30
ssh deploy@server "riku --json ps myapp"
```

---

## See Also

- [CLI Reference](cli.md) - All Riku commands
- [Environment Variables](env.md) - Configuration options
- [Plugin System](plugins.md) - Extending Riku
