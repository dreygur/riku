//! Agent identity and scope, rate limiting and the audit log.

use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::state::{open_private, Access};
use super::types::AgentScope;

const UNKNOWN_AGENT: &str = "unknown-agent";
const AUDIT_FILE: &str = "audit.log";
const RATE_WINDOW_SECS: u64 = 60;

/// Who is calling and what they may do. Both come from the arguments of the
/// key's forced command, which the server owner wrote. Nothing is read from
/// the environment or from the client's own request: with sshd `AcceptEnv` a
/// client can send any variable and it writes its own `SSH_ORIGINAL_COMMAND`.
#[derive(Debug, Clone)]
pub struct AgentSession {
    /// Name written to the audit log and used for the rate limit file.
    pub id: String,
    /// What the agent may run. `readonly` unless the forced command says more.
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

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// The request times in `file` that are still inside the window. A time in
/// the future counts as `now`, so a planted value expires like any other.
fn recent_requests(file: &Path, now: u64) -> Vec<u64> {
    let mut content = String::new();
    if let Ok(mut f) = open_private(file, Access::Read) {
        let _ = f.read_to_string(&mut content);
    }
    content
        .lines()
        .filter_map(|line| line.parse::<u64>().ok())
        .map(|ts| ts.min(now))
        .filter(|ts| now.saturating_sub(*ts) < RATE_WINDOW_SECS)
        .collect()
}

/// Records one request by `agent_id` and says whether it is within the
/// scope's per minute limit. `state` is the directory from
/// [`super::state::agent_dir`]; an error means the counters cannot be kept
/// safely, and the caller must refuse the request.
#[allow(clippy::question_mark)]
pub fn check_rate_limit(
    state: &Path,
    agent_id: &str,
    scope: &AgentScope,
    now: u64,
) -> std::io::Result<bool> {
    let file = state.join(format!("rate-{}.log", agent_id.replace('@', "_")));
    let mut timestamps = recent_requests(&file, now);
    let allowed = timestamps.len() < scope.rate_limit() as usize;
    if allowed {
        timestamps.push(now);
    }
    let content: String = timestamps.iter().map(|t| format!("{t}\n")).collect();
    let mut out = match open_private(&file, Access::Rewrite) {
        Ok(out) => out,
        Err(e) => return Err(e),
    };
    match out.write_all(content.as_bytes()) {
        Ok(()) => Ok(allowed),
        Err(e) => Err(e),
    }
}

/// [`check_rate_limit`] at the current time.
pub fn check_rate_limit_now(
    state: &Path,
    agent_id: &str,
    scope: &AgentScope,
) -> std::io::Result<bool> {
    check_rate_limit(state, agent_id, scope, unix_now())
}

/// Appends one line to the audit log in `state`. A log that cannot be
/// written is reported, not fatal: the action has already been decided.
pub fn log_agent_action(state: &Path, agent_id: &str, action: &str, app: &str, success: bool) {
    let status = if success { "success" } else { "failed" };
    let line = format!(
        "{} [AI] {} {} {} {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        agent_id,
        action,
        app,
        status
    );
    let written = open_private(&state.join(AUDIT_FILE), Access::Append)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = written {
        tracing::warn!("agent audit log not written: {e}");
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
