/// Agent authentication: identity resolution, scope parsing, rate limiting, audit logging.
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::AgentScope;

const UNKNOWN_AGENT: &str = "unknown-agent";

/// Who is calling and what they may do. Both come from the arguments of the
/// key's forced command, which the server owner wrote. Nothing is read from
/// the environment or from the client's own request: with sshd `AcceptEnv` a
/// client can send any variable and it writes its own `SSH_ORIGINAL_COMMAND`.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: String,
    pub scope: AgentScope,
}

fn is_plain_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

impl AgentSession {
    /// A missing or unknown scope is read only. An id that is missing or not
    /// plain (it names a file under the rate limit directory) is
    /// `unknown-agent`.
    pub fn from_flags(scope: Option<&str>, id: Option<&str>) -> Self {
        AgentSession {
            id: match id {
                Some(id) if is_plain_id(id) => id.to_string(),
                _ => UNKNOWN_AGENT.to_string(),
            },
            scope: scope.map_or(AgentScope::Readonly, AgentScope::parse),
        }
    }
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

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
