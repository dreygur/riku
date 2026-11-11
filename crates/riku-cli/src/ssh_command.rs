//! Builds the process argv, recovering it from `SSH_ORIGINAL_COMMAND` when
//! riku runs as an sshd forced command.
//!
//! Security model: `SSH_ORIGINAL_COMMAND` is attacker-controlled text from an
//! authenticated key holder. It is only tokenized with POSIX shell quoting
//! rules. Nothing is executed through a shell, and no globs, variables or
//! command substitutions are expanded, so each token reaches clap verbatim.

use crate::agent::types::AgentResponse;
use crate::ssh_agent;

const ORIGINAL_COMMAND_VAR: &str = "SSH_ORIGINAL_COMMAND";
const PROGRAM_NAME: &str = "riku";

/// Splits an SSH original command into argv, without the program name.
///
/// A leading `riku` word is dropped so `ssh host riku ps app` and
/// `ssh host ps app` behave the same. Fails on unbalanced quotes.
pub fn split_original(original: &str) -> anyhow::Result<Vec<String>> {
    let mut words = match shlex::split(original) {
        Some(words) => words,
        None => return Err(anyhow::anyhow!("unbalanced quotes in SSH command")),
    };
    if words.first().map(String::as_str) == Some(PROGRAM_NAME) {
        words.remove(0);
    }
    Ok(words)
}

/// Returns the argv clap should parse.
///
/// With `SSH_ORIGINAL_COMMAND` set and nonempty: when riku is started with no
/// arguments the argv comes from that variable, and when it is started as a
/// bare `riku agent` forced command the client's checked request is appended
/// (see `ssh_agent`). Otherwise it is the real process argv.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn process_argv() -> anyhow::Result<Vec<String>> {
    let original = std::env::var(ORIGINAL_COMMAND_VAR).unwrap_or_default();
    let real: Vec<String> = std::env::args().collect();
    if original.is_empty() {
        return Ok(real);
    }
    if real.len() == 1 {
        let words = match split_original(&original) {
            Ok(words) => words,
            Err(e) => return Err(e),
        };
        let mut argv = vec![PROGRAM_NAME.to_string()];
        argv.extend(words);
        return Ok(argv);
    }
    if ssh_agent::is_bare_agent(&real) {
        return agent_argv(real, &original);
    }
    Ok(real)
}

/// A refused request still answers in JSON on stdout, like every agent call.
fn agent_argv(forced: Vec<String>, original: &str) -> anyhow::Result<Vec<String>> {
    let request = match split_original(original).and_then(ssh_agent::client_request) {
        Ok(request) => request,
        Err(e) => {
            let refusal = AgentResponse::error("INVALID_PARAMETERS", &e.to_string());
            println!("{}", refusal.to_json());
            return Err(e);
        }
    };
    let mut argv = forced;
    argv.extend(request);
    Ok(argv)
}

#[cfg(test)]
#[path = "ssh_command_tests.rs"]
mod tests;
