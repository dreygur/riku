//! Merges an agent's ssh request into its forced command.
//!
//! An agent key in `authorized_keys` carries a forced command such as
//! `riku agent --scope staging --agent-id cursor`. sshd runs that and puts
//! what the client asked for in `SSH_ORIGINAL_COMMAND`.
//!
//! Security model: the forced argv is trusted, the administrator wrote it.
//! The client's words are attacker text from a key holder. They may only name
//! the command, its arguments, `--confirm` and the read-only info flags.
//! Scope, identity and target options are refused, so they can only ever come
//! from the forced command. Words after `--` reach clap as plain arguments.

const FORCED_OPTIONS: [&str; 2] = ["--scope", "--agent-id"];
const CLIENT_FLAGS: [&str; 3] = ["--intro", "--schema", "--agent-help"];

fn is_forced_option_with_inline_value(word: &str) -> bool {
    FORCED_OPTIONS.iter().any(|option| {
        word.strip_prefix(option)
            .is_some_and(|rest| rest.starts_with('='))
    })
}

/// Whether `forced` is `riku agent` with only scope and identity, meaning
/// the command itself has to come from the client.
pub fn is_bare_agent(forced: &[String]) -> bool {
    if forced.get(1).map(String::as_str) != Some("agent") {
        return false;
    }
    let mut rest = forced[2..].iter();
    while let Some(word) = rest.next() {
        if FORCED_OPTIONS.contains(&word.as_str()) {
            if rest.next().is_none() {
                return false;
            }
        } else if !is_forced_option_with_inline_value(word) {
            return false;
        }
    }
    true
}

/// Checks the client's words and returns the ones to append to the forced
/// argv.
pub fn client_request(words: Vec<String>) -> anyhow::Result<Vec<String>> {
    let mut words = words.into_iter().peekable();
    if words.peek().map(String::as_str) == Some("agent") {
        words.next();
    }

    let mut request = Vec::new();
    while let Some(word) = words.next() {
        if word == "--" {
            request.push(word);
            request.extend(words);
            break;
        }
        if word == "--confirm" {
            let token = match words.next() {
                Some(token) if !token.starts_with('-') => token,
                _ => return Err(anyhow::anyhow!("--confirm needs a token")),
            };
            request.push(word);
            request.push(token);
            continue;
        }
        if !word.starts_with('-')
            || word.starts_with("--confirm=")
            || CLIENT_FLAGS.contains(&word.as_str())
        {
            request.push(word);
            continue;
        }
        return Err(anyhow::anyhow!(
            "option {word} is not allowed in an agent's ssh request"
        ));
    }
    Ok(request)
}

#[cfg(test)]
#[path = "ssh_agent_tests.rs"]
mod tests;
