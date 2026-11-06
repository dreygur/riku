//! Agent identity and scope, and the per minute rate limit.

use std::io::{Error, Read, Result, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nix::fcntl::{Flock, FlockArg};

use super::state::{open_private, Access};
use super::types::AgentScope;

const UNKNOWN_AGENT: &str = "unknown-agent";
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

/// The request times in `content` that are still inside the window. A time
/// in the future counts as `now`, so a planted value expires like any other.
fn recent_requests(content: &str, now: u64) -> Vec<u64> {
    content
        .lines()
        .filter_map(|line| line.parse::<u64>().ok())
        .map(|ts| ts.min(now))
        .filter(|ts| now.saturating_sub(*ts) < RATE_WINDOW_SECS)
        .collect()
}

#[allow(clippy::question_mark)]
fn replace_content(file: &mut std::fs::File, content: &str) -> Result<()> {
    if let Err(e) = file.seek(SeekFrom::Start(0)) {
        return Err(e);
    }
    if let Err(e) = file.set_len(0) {
        return Err(e);
    }
    file.write_all(content.as_bytes())
}

/// Records one request by `agent_id` and says whether it is within the
/// scope's per minute limit. `state` is the directory from
/// [`super::state::agent_dir`]; an error means the counters cannot be kept
/// safely, and the caller must refuse the request.
///
/// The counter file is locked exclusively from the read to the write, so
/// concurrent requests count one after another and none sees a half written
/// file.
#[allow(clippy::question_mark)]
pub fn check_rate_limit(
    state: &Path,
    agent_id: &str,
    scope: &AgentScope,
    now: u64,
) -> Result<bool> {
    let path = state.join(format!("rate-{}.log", agent_id.replace('@', "_")));
    let file = match open_private(&path, Access::Update) {
        Ok(file) => file,
        Err(e) => return Err(e),
    };
    let mut locked = match Flock::lock(file, FlockArg::LockExclusive) {
        Ok(locked) => locked,
        Err((_, errno)) => return Err(Error::from(errno)),
    };
    let mut content = String::new();
    if let Err(e) = locked.read_to_string(&mut content) {
        return Err(e);
    }
    let mut timestamps = recent_requests(&content, now);
    let allowed = timestamps.len() < scope.rate_limit() as usize;
    if allowed {
        timestamps.push(now);
    }
    let updated: String = timestamps.iter().map(|t| format!("{t}\n")).collect();
    match replace_content(&mut locked, &updated) {
        Ok(()) => Ok(allowed),
        Err(e) => Err(e),
    }
}

/// [`check_rate_limit`] at the current time.
pub fn check_rate_limit_now(state: &Path, agent_id: &str, scope: &AgentScope) -> Result<bool> {
    check_rate_limit(state, agent_id, scope, unix_now())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
