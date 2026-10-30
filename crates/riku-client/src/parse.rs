//! Turns `argv` into clap matches for `rk`, with the client's own help.

use anyhow::Result;
use clap::ArgMatches;
use riku_args::help::client_command;
use riku_args::parse::parse_with_help;
use riku_remote::remote::route::implied_server;

use crate::local::check_supported;

const HELP: &str = "help";

fn command_name(argv: &[String]) -> Option<String> {
    let lenient = client_command(None)
        .ignore_errors(true)
        .disable_help_flag(true)
        .disable_help_subcommand(true);
    let matches = match lenient.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(_) => return None,
    };
    matches.subcommand_name().map(str::to_string)
}

/// `rk help init` names its command after `help`, which the lenient parse
/// does not see as a subcommand once help is disabled.
fn without_help_word(argv: &[String]) -> Vec<String> {
    let mut rest = argv.to_vec();
    if let Some(i) = argv.iter().skip(1).position(|word| word == HELP) {
        rest.remove(i + 1);
    }
    rest
}

/// A refused command is reported as refused even when help is asked for or
/// its own arguments do not parse, since `rk container export` should not
/// complain about flags it will never use.
fn refusal_behind(argv: &[String]) -> Result<()> {
    let name = match command_name(argv) {
        Some(name) => name,
        None => match command_name(&without_help_word(argv)) {
            Some(name) => name,
            None => return Ok(()),
        },
    };
    check_supported(&name)
}

/// Parses `argv`. A help request is rendered again with a heading that names
/// the server a bare forwarded command would reach; that render exits.
pub(crate) fn parse(argv: &[String]) -> Result<ArgMatches> {
    if let Err(refusal) = refusal_behind(argv) {
        return Err(refusal);
    }
    let implied = || implied_server(false).map(|s| s.name);
    match parse_with_help(argv, client_command, implied) {
        Ok(matches) => Ok(matches),
        Err(e) => e.exit(),
    }
}
