//! `rk`, the laptop only client. It parses the same command tree as `riku`,
//! then either runs a local command, forwards to a server over ssh, or
//! refuses. It never runs server code and is never the ssh forced command.

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#![allow(clippy::question_mark)]

mod local;
mod parse;

use anyhow::Result;
use clap::FromArgMatches;
use riku_args::cli::Cli;
use riku_args::parse::command_names;
use riku_remote::plugin_author::Program;
use riku_remote::remote::flow::{run_or_forward, Outcome};
use riku_remote::remote::route::target_for;
use riku_remote::remote::target::Selection;

use local::{check_supported, no_local_mode, run_without_server};
use parse::parse;

const PROGRAM: &str = Program::Rk.name();

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
    let (name, sub) = command_names(&matches);
    if let Err(e) = check_supported(name) {
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
    match run_or_forward(&argv, &matches, args, &target, PROGRAM) {
        Ok(Outcome::Done) => Ok(()),
        Ok(Outcome::Local(command)) => run_without_server(command),
        Err(e) => Err(e),
    }
}

fn main() -> Result<()> {
    run(std::env::args().collect())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
