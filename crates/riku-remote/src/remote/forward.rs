//! Runs a forwarded command on a server through the system `ssh`.
//!
//! Security model: the ssh destination is a validated `SshAddress`, which
//! cannot start with `-`, and it follows a `--` so ssh never reads it or
//! anything after it as an option. The remote command is one argument built
//! with `shlex::try_join`, so the server's `SSH_ORIGINAL_COMMAND` split gives
//! back exactly the words typed here. No local shell is involved.

use std::os::unix::process::ExitStatusExt;

use super::Server;
use anyhow::{anyhow, Result};
use clap::Command;

const SERVER_ARG_ID: &str = "server";

/// Parses `argv` and says whether `-s/--server` was given. `None` when the
/// line does not parse.
fn server_flag_given(cli: &Command, argv: &[String]) -> Option<bool> {
    match cli.clone().try_get_matches_from(argv) {
        Ok(matches) => Some(matches.contains_id(SERVER_ARG_ID)),
        Err(_) => None,
    }
}

/// Token sets that could be the server option: `-s NAME`, `--server NAME`,
/// `--server=NAME`, `-sNAME` and `-s=NAME`.
fn server_arg_candidates(argv: &[String]) -> Vec<Vec<usize>> {
    argv.iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, word)| {
            if (word == "-s" || word == "--server") && i + 1 < argv.len() {
                Some(vec![i, i + 1])
            } else if word.starts_with("--server=") || (word.starts_with("-s") && word.len() > 2) {
                Some(vec![i])
            } else {
                None
            }
        })
        .collect()
}

/// Positions in `argv` of the `-s/--server` tokens clap consumed.
///
/// clap's own indices count attached values and short clusters as extra
/// slots, so they are not argv positions. Instead each candidate is removed
/// in turn and the line parsed again: the one whose removal leaves no server
/// is the one clap read. A `-s` inside a `run` command line is never picked,
/// because removing it leaves the real one in place. `None` when no
/// candidate fits.
pub fn server_arg_positions(cli: &Command, argv: &[String]) -> Option<Vec<usize>> {
    if server_flag_given(cli, argv) != Some(true) {
        return Some(Vec::new());
    }
    server_arg_candidates(argv).into_iter().find(|consumed| {
        let mut rest = argv[..1].to_vec();
        rest.extend(strip_target_args(argv, consumed));
        server_flag_given(cli, &rest) == Some(false)
    })
}

/// Drops the program name and the `consumed` positions from `argv`.
pub fn strip_target_args(argv: &[String], consumed: &[usize]) -> Vec<String> {
    argv.iter()
        .enumerate()
        .skip(1)
        .filter(|(i, _)| !consumed.contains(i))
        .map(|(_, word)| word.clone())
        .collect()
}

fn is_follow_flag(word: &str) -> bool {
    word == "--follow" || (word.starts_with('-') && !word.starts_with("--") && word.contains('f'))
}

/// Whether the command needs a remote tty: `run`, and `logs` with follow.
pub fn needs_tty(args: &[String]) -> bool {
    match args.split_first() {
        Some((command, _)) if command == "run" => true,
        Some((command, rest)) if command == "logs" => rest.iter().any(|w| is_follow_flag(w)),
        _ => false,
    }
}

/// Arguments for `ssh` that run `args` on `server`, without a leading `riku`
/// since the server's forced command supplies it.
pub fn ssh_args(
    server: &Server,
    args: &[String],
    tty: bool,
    app_hint: Option<&str>,
) -> Vec<String> {
    let hint = app_hint.map(|app| ["--app-hint", app]);
    let words = hint
        .iter()
        .flatten()
        .copied()
        .chain(args.iter().map(String::as_str));
    // Process arguments cannot hold NUL, and git_remote drops app names with
    // control characters, so quoting cannot fail.
    let remote = shlex::try_join(words).expect("forwarded words never contain NUL");

    let mut ssh = Vec::with_capacity(4);
    if tty {
        ssh.push("-t".to_string());
    }
    ssh.push("--".to_string());
    ssh.push(server.address.as_str().to_string());
    ssh.push(remote);
    ssh
}

/// Runs `args` on `server` with inherited stdio and returns the remote exit
/// code. ssh dying from a signal is reported and counts as exit code 1.
pub fn run(server: &Server, args: Vec<String>, app_hint: Option<&str>) -> Result<i32> {
    let ssh = ssh_args(server, &args, needs_tty(&args), app_hint);
    let status = match std::process::Command::new("ssh").args(&ssh).status() {
        Ok(status) => status,
        Err(e) => return Err(anyhow!("failed to start ssh: {e}")),
    };
    match (status.code(), status.signal()) {
        (Some(code), _) => Ok(code),
        (None, signal) => {
            eprintln!("ssh was killed by signal {}", signal.unwrap_or_default());
            Ok(1)
        }
    }
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod tests;
