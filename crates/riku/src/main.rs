//! `riku` binary entry point. All logic lives in the `riku` library crate; the
//! bin only parses CLI args and dispatches, so the modules compile once (in the
//! lib) rather than being recompiled here.

use anyhow::Result;
use clap::error::ErrorKind;
use clap::{ArgMatches, FromArgMatches};

use riku::cli::app_hint::apply_app_hint;
use riku::cli::client_plugins;
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
    match cli::help::command(cli::help::FORWARD_HEADING).try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) =>
        {
            let server = route::implied_server(via_ssh);
            let heading = cli::help::forward_heading(server.as_ref().map(|s| &s.name));
            cli::help::command(&heading).get_matches_from(argv)
        }
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
    let target = match route::target_for(matches.subcommand_name().unwrap_or(""), selection) {
        Ok(target) => target,
        Err(e) => return Err(e),
    };
    let paths = match RikuPaths::from_env() {
        Ok(paths) => paths,
        Err(e) => return Err(e),
    };

    let (server, hint) = match &target {
        Target::Remote {
            server, app_hint, ..
        } => (Some(server), app_hint.as_deref()),
        Target::Local => (None, args.app_hint.as_deref()),
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

    if let Some(server) = server {
        if let Commands::Dashboard { .. } = command {
            return cli::remote::dashboard::run_remote(server);
        }
        let sent_hint = if hint_used { hint } else { None };
        match route::forward(&argv, server, sent_hint) {
            Ok(code) => std::process::exit(code),
            Err(e) => return Err(e),
        }
    }

    match command {
        Commands::Dashboard { bind, token } => dashboard::run(&paths, &bind, token),
        command => cli::dispatch::run_local(&paths, command),
    }
}
