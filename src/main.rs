//! `riku` binary entry point. All logic lives in the `riku` library crate; the
//! bin only parses CLI args and dispatches, so the modules compile once (in the
//! lib) rather than being recompiled here.

use anyhow::Result;
use clap::Parser;

use riku::cli::client_plugins;
use riku::cli::container;
use riku::cli::routing::{build_plugin_args, get_plugin_command};
use riku::cli::{AppsCmd, Cli, Commands, ConfigCmd, HookCmd, PluginCmd, StatsCmd};
use riku::config::RikuPaths;
use riku::{cli, supervisor};

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

fn main() -> Result<()> {
    // Initialize structured logging
    init_tracing();

    let args = Cli::parse();
    let paths = RikuPaths::from_env();

    // Check for client plugins first — they can override built-in commands.
    if let Some(plugin_name) = get_plugin_command(&args.command) {
        let plugin_args = build_plugin_args(&args.command);
        if client_plugins::try_execute_client_plugin(&plugin_name, &plugin_args)? {
            return Ok(());
        }
    }

    match args.command {
        Commands::Agent {
            intro,
            schema,
            agent_help,
            command,
            args,
            confirm,
            json: _,
        } => {
            if intro {
                cli::agent::cmd_agent_intro(&paths)?;
            } else if schema {
                cli::agent::cmd_agent_schema()?;
            } else if agent_help {
                cli::agent::cmd_agent_help(command.as_deref())?;
            } else if let Some(cmd) = command {
                cli::agent::cmd_agent_execute(&paths, &cmd, &args, confirm.as_deref())?;
            } else {
                cli::agent::cmd_agent_help(None)?;
            }
        }
        Commands::Apps { cmd } => match cmd {
            Some(AppsCmd::Create { name }) => cli::apps::cmd_apps_create(&paths, &name)?,
            Some(AppsCmd::Info { name }) => cli::apps::cmd_apps_info(&paths, &name)?,
            Some(AppsCmd::Destroy { name }) => cli::apps::cmd_destroy(&paths, &name)?,
            None => cli::apps::cmd_apps(&paths)?,
        },
        Commands::Config(cmd) => match cmd {
            ConfigCmd::Show { app } => cli::apps::cmd_config_show(&paths, &app)?,
            ConfigCmd::Get { app, key } => cli::apps::cmd_config_get(&paths, &app, &key)?,
            ConfigCmd::Set { app, settings } => cli::apps::cmd_config_set(&paths, &app, &settings)?,
            ConfigCmd::Unset { app, keys } => cli::apps::cmd_config_unset(&paths, &app, &keys)?,
            ConfigCmd::Live { app } => cli::apps::cmd_config_live(&paths, &app)?,
        },
        Commands::Container { cmd } => {
            cli::container::cmd_container(container::ContainerCmd { command: cmd }, &paths)?
        }
        Commands::Deploy { app, from } => cli::apps::cmd_deploy(&paths, &app, from.as_deref())?,
        Commands::Destroy { app } => cli::apps::cmd_destroy(&paths, &app)?,
        Commands::Logs {
            app,
            process,
            deploy,
            follow,
        } => {
            if deploy {
                cli::apps::cmd_deploy_logs(&paths, &app, follow)?;
            } else {
                cli::apps::cmd_logs(&paths, &app, &process)?;
            }
        }
        Commands::Ps {
            app,
            verbose,
            scale,
        } => {
            if !scale.is_empty() {
                // Scale command
                let app_name =
                    app.ok_or_else(|| anyhow::anyhow!("App name required for scaling"))?;
                cli::apps::cmd_ps_scale(&paths, &app_name, &scale)?;
            } else if let Some(app_name) = app {
                // Show specific app
                cli::apps::cmd_ps_show(&paths, &app_name, verbose)?;
            } else {
                // Show all processes (always verbose by default)
                cli::apps::cmd_ps_all(&paths, true)?;
            }
        }
        Commands::Stats(cmd) => match cmd {
            StatsCmd::All => cli::apps::cmd_stats_all(&paths)?,
            StatsCmd::App { app } => cli::apps::cmd_stats_app(&paths, &app)?,
        },
        Commands::Run { app, cmd } => cli::apps::cmd_run(&paths, &app, &cmd)?,
        Commands::Restart { app, hot } => {
            if hot {
                cli::apps::cmd_hot_reload(&paths, &app)?;
            } else {
                cli::apps::cmd_restart(&paths, &app)?;
            }
        }
        Commands::Stop { app } => cli::apps::cmd_stop(&paths, &app)?,
        Commands::Init { no_systemd } => cli::setup::cmd_init(no_systemd)?,
        Commands::Doctor => cli::doctor::cmd_doctor(&paths)?,
        Commands::Update => cli::apps::cmd_update()?,
        Commands::InstallPlugins { plugins } => {
            let only = if plugins.is_empty() {
                None
            } else {
                Some(plugins)
            };
            cli::apps::cmd_install_plugins(&paths, only)?
        }
        Commands::Supervisor => cli::apps::cmd_supervisor(&paths)?,
        Commands::Plugin(cmd) => match cmd {
            PluginCmd::List => cli::client_plugins::cmd_plugin_list()?,
            PluginCmd::Exists { name } => cli::client_plugins::cmd_plugin_exists(&name)?,
        },
        Commands::Hook(cmd) => match cmd {
            HookCmd::List => cli::hooks::cmd_hook_list(&paths)?,
            HookCmd::Check { name } => cli::hooks::cmd_hook_check(&paths, &name)?,
        },
        Commands::GitHook { app, repo_path } => {
            cli::git::cmd_git_hook(&paths, &app, repo_path.as_deref())?
        }
        Commands::GitReceivePack { app } => cli::git::cmd_git_receive_pack(&paths, &app)?,
        Commands::GitUploadPack { app } => cli::git::cmd_git_upload_pack(&paths, &app)?,
        Commands::Scp { args } => cli::scp::cmd_scp(&paths, &args)?,
        Commands::NsShim => {
            let root = std::env::var("RIKU_NS_ROOT")
                .map_err(|_| anyhow::anyhow!("__ns-shim: RIKU_NS_ROOT not set"))?;
            let command = std::env::var("RIKU_NS_CMD")
                .map_err(|_| anyhow::anyhow!("__ns-shim: RIKU_NS_CMD not set"))?;
            // Only returns on failure: success either execs the real worker
            // command or becomes the signal-forwarding shim and `_exit`s.
            supervisor::process::isolation::exec_isolated(std::path::Path::new(&root), &command)?;
        }
        Commands::DumpState => cli::apps::cmd_dump_state(&paths)?,
    }

    Ok(())
}
