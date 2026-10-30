//! `rk`, the laptop only client. It parses the same command tree as `riku`,
//! then either runs a local command, forwards to a server over ssh, or
//! refuses. It never runs server code and is never the ssh forced command.

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#![allow(clippy::question_mark)]

mod local;
mod parse;

use anyhow::Result;
use clap::{ArgMatches, FromArgMatches};
use riku_args::app_hint::apply_app_hint;
use riku_args::cli::{Cli, Commands};
use riku_remote::client_plugins::try_execute_client_plugin;
use riku_remote::remote::dashboard::{check_remote_flags, run_remote};
use riku_remote::remote::route::{forward, target_for};
use riku_remote::remote::target::{Selection, Source, Target};
use riku_remote::remote::Server;
use riku_remote::routing::{build_plugin_args, get_plugin_command};

use local::{no_local_mode, run_without_server};
use parse::parse;

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

fn on_server(
    argv: &[String],
    matches: &ArgMatches,
    command: &Commands,
    server: &Server,
    source: Source,
    sent_hint: Option<&str>,
) -> Result<()> {
    if let Commands::Dashboard { .. } = command {
        if let Err(e) = check_remote_flags(matches) {
            return Err(e);
        }
        return run_remote(server);
    }
    match forward(argv, server, source, sent_hint) {
        Ok(code) => std::process::exit(code),
        Err(e) => Err(e),
    }
}

fn run(argv: Vec<String>) -> Result<()> {
    let matches = match parse(&argv) {
        Ok(matches) => matches,
        Err(e) => return Err(e),
    };
    let args = match Cli::from_arg_matches(&matches) {
        Ok(args) => args,
        Err(e) => e.exit(),
    };
    if args.local {
        return Err(no_local_mode());
    }
    let name = matches.subcommand_name().unwrap_or("");
    let sub = matches
        .subcommand_matches(name)
        .and_then(|m| m.subcommand_name());
    if let Err(e) = local::check_supported(name) {
        return Err(e);
    }
    let selection = Selection {
        flag: args.server.as_deref(),
        local: false,
        via_ssh: false,
    };
    let target = match target_for(name, sub, selection) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let hint = match &target {
        Target::Remote { app_hint, .. } => app_hint.as_deref(),
        Target::Local => args.app_hint.as_deref(),
    };
    let mut command = args.command;
    let hint_used = apply_app_hint(&mut command, hint);

    match run_client_plugin(&command, &target) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => return Err(e),
    }
    match &target {
        Target::Remote {
            server,
            source,
            app_hint,
        } => {
            let sent_hint = if hint_used { app_hint.as_deref() } else { None };
            on_server(&argv, &matches, &command, server, *source, sent_hint)
        }
        Target::Local => run_without_server(command, name, sub),
    }
}

fn main() -> Result<()> {
    run(std::env::args().collect())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
