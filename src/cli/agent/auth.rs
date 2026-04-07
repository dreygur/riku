/// Agent authentication: identity resolution, scope parsing, rate limiting, audit logging.
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::AgentScope;

/// Get agent identity from SSH key comment or environment
pub fn get_agent_identity() -> Option<String> {
    // Try environment variable first (set by SSH forced command)
    if let Ok(id) = std::env::var("RIKU_AGENT_ID") {
        return Some(id);
    }

    // Try to extract from SSH key comment via SSH_CONNECTION
    // This would typically be set in the forced command in authorized_keys
    if let Ok(cmd) = std::env::var("SSH_ORIGINAL_COMMAND") {
        // Extract agent ID from command if present
        if cmd.contains("--agent-id=") {
            return cmd
                .split("--agent-id=")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.to_string());
        }
        return Some("ssh-agent".to_string());
    }

    Some("unknown-agent".to_string())
}

/// Get agent scope from SSH key restrictions or environment
pub fn get_agent_scope() -> AgentScope {
    // Try environment variable first (set by SSH forced command)
    if let Ok(scope) = std::env::var("RIKU_AGENT_SCOPE") {
        return AgentScope::from_str(&scope);
    }

    // Parse authorized_keys to find scope from command restriction
    // Format: command="riku agent --scope staging",no-port-forwarding ssh-rsa AAAA... comment
    if let Some(scope) = parse_scope_from_authorized_keys() {
        return scope;
    }

    AgentScope::Readonly
}

/// Parse agent scope from authorized_keys file
fn parse_scope_from_authorized_keys() -> Option<AgentScope> {
    let auth_keys_path = dirs::home_dir().map(|h| h.join(".ssh/authorized_keys"));

    if let Some(path) = auth_keys_path {
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                // Look for command restriction with scope
                if line.contains("riku agent") && line.contains("--scope") {
                    if let Some(scope_start) = line.find("--scope ") {
                        let scope_str = &line[scope_start + 8..];
                        let scope = scope_str.split_whitespace().next()?;
                        return Some(AgentScope::from_str(scope));
                    }
                }
            }
        }
    }
    None
}

/// Check rate limit for agent
pub fn check_rate_limit(agent_id: &str, scope: &AgentScope) -> bool {
    let rate_file = Path::new("/tmp/riku-agent-rates");
    let agent_file = rate_file.join(format!("{}.log", agent_id.replace('@', "_")));

    // Create rate directory if not exists
    let _ = fs::create_dir_all(rate_file);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let window = 60; // 1 minute window
    let limit = scope.rate_limit();

    // Read existing timestamps
    let mut timestamps: Vec<u64> = Vec::new();
    if let Ok(content) = fs::read_to_string(&agent_file) {
        for line in content.lines() {
            if let Ok(ts) = line.parse::<u64>() {
                if now - ts < window {
                    timestamps.push(ts);
                }
            }
        }
    }

    // Check if over limit
    if timestamps.len() >= limit as usize {
        return false;
    }

    // Add current timestamp
    timestamps.push(now);
    let content: String = timestamps.iter().map(|t| t.to_string() + "\n").collect();
    let _ = fs::write(&agent_file, content);

    true
}

/// Log agent action to audit log
pub fn log_agent_action(agent_id: &str, action: &str, app: &str, success: bool) {
    let audit_file = Path::new("/tmp/riku-agent-audit.log");

    let status = if success { "success" } else { "failed" };
    let log_line = format!(
        "{} [AI] {} {} {} {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        agent_id,
        action,
        app,
        status
    );

    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_file)
        .map(|mut f| {
            use std::io::Write;
            f.write_all(log_line.as_bytes())
        });
}
