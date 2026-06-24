//! Plugin routing helpers for main.rs.
//!
//! Determines which CLI commands can be intercepted by client plugins
//! and builds the argument list passed to those plugins.

use super::{Commands, ConfigCmd};

/// Extract the plugin command name from a CLI command.
/// Returns None for commands that shouldn't be overridden by plugins.
pub fn get_plugin_command(command: &Commands) -> Option<String> {
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
        Commands::Rollback { .. } => None,
        Commands::Container { .. } => Some("container".to_string()),

        // Plugin/Hook commands — don't recursively check for plugins
        Commands::Plugin(_) => None,
        Commands::Plugins(_) => None,
        Commands::Hook(_) => None,

        // Agent command - don't check for plugins
        Commands::Agent { .. } => None,

        // Core commands that shouldn't be overridden
        Commands::Init { .. } => None,
        Commands::Quickstart { .. } => None,
        Commands::Doctor => None,
        Commands::Dashboard { .. } => None,
        Commands::Addon(_) => None,
        Commands::Update => None,
        Commands::InstallPlugins { .. } => None,
        Commands::Supervisor => None,
        Commands::GitHook { .. } => None,
        Commands::GitReceivePack { .. } => None,
        Commands::GitUploadPack { .. } => None,
        Commands::Scp { .. } => None,
        Commands::NsShim => None,
        Commands::DumpState => None,
    }
}

/// Build arguments for client plugin execution.
/// Plugin interface: $1=server, $2=app, $3=command, $4+=extra args
pub fn build_plugin_args(command: &Commands) -> Vec<String> {
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
        Commands::Logs {
            app,
            process,
            deploy,
            follow,
        } => {
            args.push(app.clone());
            args.push("logs".to_string());
            if *deploy {
                args.push("--deploy".to_string());
            }
            if *follow {
                args.push("--follow".to_string());
            }
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
