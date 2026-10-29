//! Builds the process argv, recovering it from `SSH_ORIGINAL_COMMAND` when
//! riku runs as an sshd forced command.
//!
//! Security model: `SSH_ORIGINAL_COMMAND` is attacker-controlled text from an
//! authenticated key holder. It is only tokenized with POSIX shell quoting
//! rules. Nothing is executed through a shell, and no globs, variables or
//! command substitutions are expanded, so each token reaches clap verbatim.

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
/// When riku is started with no arguments and `SSH_ORIGINAL_COMMAND` is set
/// and nonempty, the argv comes from that variable. Otherwise it is the real
/// process argv.
// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
pub fn process_argv() -> anyhow::Result<Vec<String>> {
    let original = std::env::var(ORIGINAL_COMMAND_VAR).unwrap_or_default();
    if std::env::args().count() != 1 || original.is_empty() {
        return Ok(std::env::args().collect());
    }
    let words = match split_original(&original) {
        Ok(words) => words,
        Err(e) => return Err(e),
    };
    let mut argv = vec![PROGRAM_NAME.to_string()];
    argv.extend(words);
    Ok(argv)
}

#[cfg(test)]
#[path = "ssh_command_tests.rs"]
mod tests;
