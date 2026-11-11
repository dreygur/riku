//! `riku` binary entry point. All logic lives in the `riku` library crate; the
//! bin only parses CLI args and dispatches, so the modules compile once (in the
//! lib) rather than being recompiled here.

use anyhow::Result;
use clap::{ArgMatches, FromArgMatches};

use riku::cli::address::ServerName;
use riku::cli::parse::{command_names, parse_with_help};
use riku::cli::remote::flow::{run_or_forward, Outcome};
use riku::cli::remote::route;
use riku::cli::remote::target::Selection;
use riku::cli::{Cli, Commands};
use riku::config::RikuPaths;
use riku::{cli, dashboard};

const PROGRAM: &str = "riku";

/// Commands that run as services, whose INFO logs are their journal.
const DAEMONS: [&str; 3] = ["supervisor", "router-helper", "dashboard"];

/// Structured logs go to stderr, so stdout stays for command output (and
/// agent JSON). `RUST_LOG` overrides the level: INFO for the daemons, WARN
/// for everything else, whose user facing output is the rail. Escapes only
/// on a terminal and never with `NO_COLOR`.
fn init_tracing(command: &str) {
    use tracing_subscriber::{fmt, EnvFilter};

    let default = if DAEMONS.contains(&command) {
        "info"
    } else {
        "warn"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(riku::util::display::Stream::Stderr.colour())
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .init();
}

/// Parses `argv`. A help request is rendered again with a heading that names
/// the server a bare forwarded command would reach; that render exits.
fn parse(argv: &[String], via_ssh: bool) -> ArgMatches {
    let build =
        |server: Option<&ServerName>| cli::help::command(&cli::help::forward_heading(server));
    let implied = || route::implied_server(via_ssh).map(|s| s.name);
    match parse_with_help(argv, build, implied) {
        Ok(matches) => matches,
        Err(e) => e.exit(),
    }
}

/// Prints a failure on the output rail (`■  reason` on stderr), unless the
/// command already showed it in full (a failed deploy), and exits 1.
fn main() {
    if let Err(e) = run() {
        let reason = format!("{e:#}");
        if !riku::util::display::failure_already_shown(&reason) {
            riku::util::display::error(&reason);
        }
        std::process::exit(1);
    }
}

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
fn run() -> Result<()> {
    let argv = match cli::ssh_command::process_argv() {
        Ok(argv) => argv,
        Err(e) => return Err(e),
    };
    let via_ssh = std::env::var_os("SSH_ORIGINAL_COMMAND").is_some();
    let matches = parse(&argv, via_ssh);
    let args = match Cli::from_arg_matches(&matches) {
        Ok(args) => args,
        Err(e) => e.exit(),
    };
    let selection = Selection {
        flag: args.server.as_deref(),
        local: args.local,
        via_ssh,
    };
    let (command_name, subcommand_name) = command_names(&matches);
    init_tracing(command_name);
    let target = match route::target_for(command_name, subcommand_name, selection) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let command = match run_or_forward(&argv, &matches, args, &target, PROGRAM) {
        Ok(Outcome::Done) => return Ok(()),
        Ok(Outcome::Local(command)) => command,
        Err(e) => return Err(e),
    };
    // The helper runs as a systemd system service, which sets no HOME.
    if matches!(command, Commands::RouterHelper) {
        return cli::router_helper::cmd_router_helper();
    }
    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };
    match command {
        Commands::Dashboard { bind, token } => dashboard::run(&paths, &bind, token),
        command => cli::dispatch::run_local(&paths, command),
    }
}
