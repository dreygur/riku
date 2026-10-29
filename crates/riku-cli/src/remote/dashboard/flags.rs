//! Flags `riku dashboard` only honors when it serves on this machine.

use anyhow::{anyhow, Result};
use clap::parser::ValueSource;
use clap::ArgMatches;

/// Clap argument id and the flag as typed.
const LOCAL_ONLY: [(&str, &str); 2] = [("bind", "--bind"), ("token", "--token")];

/// Refuses `--bind` and `--token` for a remote dashboard instead of
/// silently ignoring them: the remote side always binds `127.0.0.1:0` and
/// makes its own token.
pub fn check_remote_flags(matches: &ArgMatches) -> Result<()> {
    let dashboard = match matches.subcommand_matches("dashboard") {
        Some(dashboard) => dashboard,
        None => return Ok(()),
    };
    let given = LOCAL_ONLY
        .iter()
        .find(|(id, _)| dashboard.value_source(id) == Some(ValueSource::CommandLine));
    match given {
        Some((_, flag)) => Err(anyhow!(
            "{flag} only applies to a dashboard served on this machine; \
             a remote session binds 127.0.0.1 on the server and makes its own token"
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "flags_tests.rs"]
mod tests;
