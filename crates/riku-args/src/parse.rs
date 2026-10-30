//! Argument parsing shared by `riku` and `rk`.

use clap::error::ErrorKind;
use clap::{ArgMatches, Command};

use crate::address::ServerName;

/// Parses `argv` with the command `build` makes. A help request is rendered
/// again with the server a bare forwarded command would reach, and that
/// render exits. Any other parse error is returned for the caller to report.
///
/// `build` gets `None` for the first parse. `implied_server` is only called
/// when help was asked for, so ordinary runs never look a server up.
pub fn parse_with_help(
    argv: &[String],
    build: impl Fn(Option<&ServerName>) -> Command,
    implied_server: impl FnOnce() -> Option<ServerName>,
) -> Result<ArgMatches, clap::Error> {
    let error = match build(None).try_get_matches_from(argv) {
        Ok(matches) => return Ok(matches),
        Err(e) => e,
    };
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let server = implied_server();
            Ok(build(server.as_ref()).get_matches_from(argv))
        }
        _ => Err(error),
    }
}
