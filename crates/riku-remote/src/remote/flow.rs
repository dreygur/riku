//! The part of dispatch `riku` and `rk` share once argv is parsed and the
//! target is known: the app hint, client plugins, the remote dashboard and
//! forwarding. What runs locally is left to each binary.

use crate::cli::{Cli, Commands};
use anyhow::Result;
use clap::ArgMatches;
use riku_args::app_hint::apply_app_hint;

use crate::client_plugins::try_execute_client_plugin;
use crate::routing::{build_plugin_args, get_plugin_command};

use super::dashboard::{check_remote_flags, run_remote};
use super::route::forward;
use super::target::{Source, Target};
use super::Server;

/// What is left to do after the shared flow.
pub enum Outcome {
    /// Handled here, nothing left to run.
    Done,
    /// Runs on this machine; the caller decides how.
    Local(Commands),
}

fn run_client_plugin(command: &Commands, target: &Target) -> Result<bool> {
    let name = match get_plugin_command(command) {
        Some(name) => name,
        None => return Ok(false),
    };
    let address = match target {
        Target::Remote { server, .. } => Some(&server.address),
        Target::Local => None,
    };
    try_execute_client_plugin(&name, &build_plugin_args(command, address))
}

#[allow(clippy::question_mark)]
fn on_server(
    argv: &[String],
    matches: &ArgMatches,
    command: &Commands,
    server: &Server,
    source: Source,
    sent_hint: Option<&str>,
    program: &str,
) -> Result<Outcome> {
    if let Commands::Dashboard { .. } = command {
        if let Err(e) = check_remote_flags(matches) {
            return Err(e);
        }
        return run_remote(server, program).map(|()| Outcome::Done);
    }
    match forward(argv, server, source, sent_hint, program) {
        Ok(code) => std::process::exit(code),
        Err(e) => Err(e),
    }
}

/// Fills in the app hint, then runs a matching client plugin, opens the
/// remote dashboard, or forwards to the target server. A forwarded command
/// exits the process with the remote exit code.
///
/// `argv` is the full process argv, program name included. `program` is the
/// binary name used in messages that show a command to run.
pub fn run_or_forward(
    argv: &[String],
    matches: &ArgMatches,
    args: Cli,
    target: &Target,
    program: &str,
) -> Result<Outcome> {
    let Cli {
        mut command,
        app_hint,
        ..
    } = args;
    let hint = match target {
        Target::Remote {
            app_hint: remote_hint,
            ..
        } => remote_hint.as_deref(),
        Target::Local => app_hint.as_deref(),
    };
    let hint_used = apply_app_hint(&mut command, hint);

    match run_client_plugin(&command, target) {
        Ok(true) => return Ok(Outcome::Done),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    match target {
        Target::Remote { server, source, .. } => {
            let sent_hint = if hint_used { hint } else { None };
            on_server(argv, matches, &command, server, *source, sent_hint, program)
        }
        Target::Local => Ok(Outcome::Local(command)),
    }
}
