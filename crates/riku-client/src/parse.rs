//! Turns `argv` into clap matches for `rk`, with the client's own help.

use anyhow::Result;
use clap::ArgMatches;
use riku_args::help::client_command;
use riku_args::parse::parse_with_help;
use riku_remote::remote::route::implied_server;

use crate::local::check_supported;

/// A refused command is reported as refused even when its own arguments do
/// not parse, since `rk container export` should not complain about flags it
/// will never use.
fn refusal_behind(argv: &[String]) -> Result<()> {
    let lenient = client_command(None).ignore_errors(true);
    match lenient.try_get_matches_from(argv) {
        Ok(matches) => check_supported(matches.subcommand_name().unwrap_or("")),
        Err(_) => Ok(()),
    }
}

/// Parses `argv`. A help request is rendered again with a heading that names
/// the server a bare forwarded command would reach; that render exits.
pub fn parse(argv: &[String]) -> Result<ArgMatches> {
    let implied = || implied_server(false).map(|s| s.name);
    match parse_with_help(argv, client_command, implied) {
        Ok(matches) => Ok(matches),
        Err(e) => match refusal_behind(argv) {
            Ok(()) => e.exit(),
            Err(refusal) => Err(refusal),
        },
    }
}
