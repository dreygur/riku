mod cli;
mod config;
mod deploy;
mod nginx;
mod plugins;
mod supervisor;
mod util;

use anyhow::Result;
use clap::Parser;

use cli::client_plugins;
use cli::container;
use cli::{AppsCmd, Cli, Commands, ConfigCmd, PluginCmd, StatsCmd};
use config::RikuPaths;

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

    // Check for client plugins first
    // Client plugins can override built-in commands
    if let Some(plugin_name) = get_plugin_command(&args.command) {
        // Build plugin arguments: server, app, command, extra args
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
        Commands::Logs { app, process } => cli::apps::cmd_logs(&paths, &app, &process)?,
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
        Commands::Update => cli::apps::cmd_update()?,
        Commands::Supervisor => cli::apps::cmd_supervisor(&paths)?,
        Commands::Plugin(cmd) => match cmd {
            PluginCmd::List => {
                let plugins = client_plugins::list_client_plugins()?;
                if plugins.is_empty() {
                    println!("No client plugins installed.");
                    println!("\nInstall plugins by placing executable scripts in:");
                    println!("  ~/.riku/client-plugins/");
                } else {
                    println!("Available client plugins:");
                    for plugin in plugins {
                        println!("  {}", plugin);
                    }
                }
            }
            PluginCmd::Exists { name } => {
                if client_plugins::client_plugin_exists(&name)? {
                    println!("Plugin '{}' is installed and executable.", name);
                    std::process::exit(0);
                } else {
                    println!("Plugin '{}' not found or not executable.", name);
                    std::process::exit(1);
                }
            }
            PluginCmd::Hooks => {
                let hooks = plugins::list_plugins(&paths)?;
                if hooks.is_empty() {
                    println!("No server-side hook plugins installed.");
                    println!("\nInstall hook plugins by placing executable scripts in:");
                    println!("  ~/.riku/plugins/");
                } else {
                    println!("Available server-side hook plugins:");
                    for hook in hooks {
                        println!("  {}", hook);
                    }
                }
            }
            PluginCmd::Check { name } => {
                plugins::discovery::validate_plugin_name(&name)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                if plugins::plugin_exists(&name, &paths) {
                    let plugin_path = paths.plugin_root.join(&name);
                    println!("Hook plugin '{}' exists and is executable.", name);
                    println!("  Path: {}", plugin_path.display());
                    std::process::exit(0);
                } else {
                    println!("Hook plugin '{}' not found or not executable.", name);
                    std::process::exit(1);
                }
            }
        },
        Commands::GitHook { app, repo_path } => {
            cli::git::cmd_git_hook(&paths, &app, repo_path.as_deref())?
        }
        Commands::GitReceivePack { app } => cli::git::cmd_git_receive_pack(&paths, &app)?,
        Commands::GitUploadPack { app } => cli::git::cmd_git_upload_pack(&paths, &app)?,
        Commands::Scp { args } => cli::scp::cmd_scp(&paths, &args)?,
    }

    Ok(())
}

/// Extract the plugin command name from a CLI command.
/// Returns None for commands that shouldn't be overridden by plugins.
fn get_plugin_command(command: &Commands) -> Option<String> {
    match command {
        // These commands can be overridden by client plugins
        Commands::Apps { .. } => Some("apps".to_string()),
        Commands::Config(_) => Some("config".to_string()),
        Commands::Deploy { .. } => Some("deploy".to_string()),
        Commands::Destroy { .. } => Some("destroy".to_string()),
        Commands::Logs { .. } => Some("logs".to_string()),
        Commands::Ps { .. } => Some("ps".to_string()),
        Commands::Stats(_) => Some("stats".to_string()),
        Commands::Run { .. } => Some("run".to_string()),
        Commands::Restart { .. } => Some("restart".to_string()),
        Commands::Stop { .. } => Some("stop".to_string()),
        Commands::Container { .. } => Some("container".to_string()),

        // Plugin command - don't recursively check for plugins
        Commands::Plugin(_) => None,

        // Agent command - don't check for plugins
        Commands::Agent { .. } => None,

        // Core commands that shouldn't be overridden
        Commands::Init { .. } => None,
        Commands::Update => None,
        Commands::Supervisor => None,
        Commands::GitHook { .. } => None,
        Commands::GitReceivePack { .. } => None,
        Commands::GitUploadPack { .. } => None,
        Commands::Scp { .. } => None,
    }
}

/// Build arguments for client plugin execution.
/// Plugin interface: $1=server, $2=app, $3=command, $4+=extra args
fn build_plugin_args(command: &Commands) -> Vec<String> {
    // For now, pass empty server (plugin can determine from git remote)
    // and extract app name and command-specific args
    let mut args = Vec::new();

    // $1: server (empty for now, plugins can determine from git remote)
    args.push(String::new());

    // $2: app name and $3+: command-specific args
    match command {
        Commands::Apps { .. } => {
            args.push(String::new()); // No app for apps list
            args.push("apps".to_string());
        }
        Commands::Config(cmd) => match cmd {
            ConfigCmd::Show { app } => {
                args.push(app.clone());
                args.push("config:show".to_string());
            }
            ConfigCmd::Get { app, key } => {
                args.push(app.clone());
                args.push(format!("config:get:{}", key));
            }
            ConfigCmd::Set { app, settings } => {
                args.push(app.clone());
                args.push("config:set".to_string());
                args.extend(settings.clone());
            }
            ConfigCmd::Unset { app, keys } => {
                args.push(app.clone());
                args.push("config:unset".to_string());
                args.extend(keys.clone());
            }
            ConfigCmd::Live { app } => {
                args.push(app.clone());
                args.push("config:live".to_string());
            }
        },
        Commands::Deploy { app, from } => {
            args.push(app.clone());
            args.push("deploy".to_string());
            if let Some(from_path) = from {
                args.push("--from".to_string());
                args.push(from_path.clone());
            }
        }
        Commands::Destroy { app } => {
            args.push(app.clone());
            args.push("destroy".to_string());
        }
        Commands::Logs { app, process } => {
            args.push(app.clone());
            args.push("logs".to_string());
            if process != "*" {
                args.push(process.clone());
            }
        }
        Commands::Ps { app, scale, .. } => {
            if !scale.is_empty() {
                args.push(app.clone().unwrap_or_default());
                args.push("ps:scale".to_string());
                args.extend(scale.clone());
            } else {
                args.push(app.clone().unwrap_or_default());
                args.push("ps:show".to_string());
            }
        }
        Commands::Run { app, cmd } => {
            args.push(app.clone());
            args.push("run".to_string());
            args.extend(cmd.clone());
        }
        Commands::Restart { app, .. } => {
            args.push(app.clone());
            args.push("restart".to_string());
        }
        Commands::Stop { app } => {
            args.push(app.clone());
            args.push("stop".to_string());
        }
        Commands::Container { .. } => {
            args.push(String::new());
            args.push("container".to_string());
        }
        _ => {}
    }

    args
}
