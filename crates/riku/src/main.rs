//! `riku` binary entry point. All logic lives in the `riku` library crate; the
//! bin only parses CLI args and dispatches, so the modules compile once (in the
//! lib) rather than being recompiled here.

use anyhow::Result;
use clap::{ArgMatches, FromArgMatches};

use riku::cli::address::ServerName;
use riku::cli::app_hint::apply_app_hint;
use riku::cli::client_plugins;
use riku::cli::parse::parse_with_help;
use riku::cli::remote::route;
use riku::cli::remote::target::{Selection, Target};
use riku::cli::routing::{build_plugin_args, get_plugin_command};
use riku::cli::{Cli, Commands};
use riku::config::RikuPaths;
use riku::{cli, dashboard};

/// Initialize tracing subscriber for structured logging
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    // Default to INFO level, can be overridden with RUST_LOG env var
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
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

// Explicit match over `?` per project convention (.claude/CLAUDE.md rule 16).
#[allow(clippy::question_mark)]
fn main() -> Result<()> {
    // Initialize structured logging
    init_tracing();

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
    let command_name = matches.subcommand_name().unwrap_or("");
    let subcommand_name = matches
        .subcommand_matches(command_name)
        .and_then(|sub| sub.subcommand_name());
    let target = match route::target_for(command_name, subcommand_name, selection) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };

    let (server, source, hint) = match &target {
        Target::Remote {
            server,
            source,
            app_hint,
        } => (Some(server), Some(*source), app_hint.as_deref()),
        Target::Local => (None, None, args.app_hint.as_deref()),
    };
    let mut command = args.command;
    let hint_used = apply_app_hint(&mut command, hint);

    // Check for client plugins first: they can override built-in commands.
    if let Some(plugin_name) = get_plugin_command(&command) {
        let plugin_args = build_plugin_args(&command, server.map(|s| &s.address));
        match client_plugins::try_execute_client_plugin(&plugin_name, &plugin_args) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => return Err(e),
        }
    }

    if let (Some(server), Some(source)) = (server, source) {
        if let Commands::Dashboard { .. } = command {
            if let Err(e) = cli::remote::dashboard::check_remote_flags(&matches) {
                return Err(e);
            }
            return cli::remote::dashboard::run_remote(server);
        }
        let sent_hint = if hint_used { hint } else { None };
        match route::forward(&argv, server, source, sent_hint) {
            Ok(code) => std::process::exit(code),
            Err(e) => return Err(e),
        }
    }

    match command {
        Commands::Dashboard { bind, token } => dashboard::run(&paths, &bind, token),
        command => cli::dispatch::run_local(&paths, command),
    }
}
