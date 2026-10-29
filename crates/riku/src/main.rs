//! `riku` binary entry point. All logic lives in the `riku` library crate; the
//! bin only parses CLI args and dispatches, so the modules compile once (in the
//! lib) rather than being recompiled here.

use anyhow::Result;
use clap::error::ErrorKind;
use clap::{ArgMatches, FromArgMatches};

use riku::cli::client_plugins;
use riku::cli::container;
use riku::cli::remote::route;
use riku::cli::remote::target::{Selection, Target};
use riku::cli::routing::{build_plugin_args, get_plugin_command};
use riku::cli::{
    require_app, AddonCmd, AppsCmd, Cli, Commands, ConfigCmd, HookCmd, MarketplaceCmd, PluginCmd,
    PluginsCmd, StatsCmd, TrustCmd,
};
use riku::config::RikuPaths;
use riku::{cli, dashboard, supervisor};

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

    // Check for client plugins first: they can override built-in commands.
    if let Some(plugin_name) = get_plugin_command(&args.command) {
        let server = match &target {
            Target::Remote { server, .. } => Some(&server.address),
            Target::Local => None,
        };
        let plugin_args = build_plugin_args(&args.command, server);
        if client_plugins::try_execute_client_plugin(&plugin_name, &plugin_args)? {
            return Ok(());
        }
    }

    if let Target::Remote {
        server, app_hint, ..
    } = &target
    {
        match route::forward(&argv, server, app_hint.as_deref()) {
            Ok(code) => std::process::exit(code),
            Err(e) => return Err(e),
        }
    }

    let hint = args.app_hint.as_deref();
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
            ConfigCmd::Show { app } => match require_app(app, hint) {
                Ok(app) => cli::apps::cmd_config_show(&paths, &app)?,
                Err(e) => return Err(e),
            },
            ConfigCmd::Get { app, key } => cli::apps::cmd_config_get(&paths, &app, &key)?,
            ConfigCmd::Set { app, settings } => cli::apps::cmd_config_set(&paths, &app, &settings)?,
            ConfigCmd::Unset { app, keys } => cli::apps::cmd_config_unset(&paths, &app, &keys)?,
            ConfigCmd::Live { app } => match require_app(app, hint) {
                Ok(app) => cli::apps::cmd_config_live(&paths, &app)?,
                Err(e) => return Err(e),
            },
        },
        Commands::Container { cmd } => {
            cli::container::cmd_container(container::ContainerCmd { command: cmd }, &paths)?
        }
        Commands::Deploy { app, from } => match require_app(app, hint) {
            Ok(app) => cli::apps::cmd_deploy(&paths, &app, from.as_deref())?,
            Err(e) => return Err(e),
        },
        Commands::Destroy { app } => match require_app(app, hint) {
            Ok(app) => cli::apps::cmd_destroy(&paths, &app)?,
            Err(e) => return Err(e),
        },
        Commands::Logs {
            app,
            process,
            deploy,
            follow,
        } => {
            let app = match require_app(app, hint) {
                Ok(app) => app,
                Err(e) => return Err(e),
            };
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
            StatsCmd::App { app } => match require_app(app, hint) {
                Ok(app) => cli::apps::cmd_stats_app(&paths, &app)?,
                Err(e) => return Err(e),
            },
        },
        Commands::Run { app, cmd } => cli::apps::cmd_run(&paths, &app, &cmd)?,
        Commands::Restart { app, hot } => {
            let app = match require_app(app, hint) {
                Ok(app) => app,
                Err(e) => return Err(e),
            };
            if hot {
                cli::apps::cmd_hot_reload(&paths, &app)?;
            } else {
                cli::apps::cmd_restart(&paths, &app)?;
            }
        }
        Commands::Stop { app } => match require_app(app, hint) {
            Ok(app) => cli::apps::cmd_stop(&paths, &app)?,
            Err(e) => return Err(e),
        },
        Commands::Rollback { app, to, list } => {
            let app = match require_app(app, hint) {
                Ok(app) => app,
                Err(e) => return Err(e),
            };
            cli::apps::cmd_rollback(&paths, &app, to.as_deref(), list)?
        }
        Commands::Init {
            no_systemd,
            ssh_key,
            add_key,
            user_steps,
        } => cli::setup::cmd_init(cli::setup::InitOptions {
            no_systemd,
            user_steps,
            ssh_key,
            add_keys: add_key,
        })?,
        Commands::Quickstart {
            name,
            runtime,
            remote,
        } => cli::quickstart::cmd_quickstart(&name, &runtime, remote.as_deref())?,
        Commands::Backup { app, out } => match require_app(app, hint) {
            Ok(app) => cli::backup::cmd_backup(&paths, &app, out.as_deref())?,
            Err(e) => return Err(e),
        },
        Commands::Restore { app, file } => cli::backup::cmd_restore(&paths, &app, &file)?,
        Commands::Doctor { fix } => cli::doctor::cmd_doctor(&paths, fix)?,
        Commands::Dashboard { bind, token } => dashboard::run(&paths, &bind, token)?,
        Commands::Addon(cmd) => match cmd {
            AddonCmd::List => cli::addon::cmd_addon_list(&paths)?,
            AddonCmd::Create { plugin, name } => {
                cli::addon::cmd_addon_create(&paths, &plugin, &name)?
            }
            AddonCmd::Bind { instance, app } => {
                let app = match require_app(app, hint) {
                    Ok(app) => app,
                    Err(e) => return Err(e),
                };
                cli::addon::cmd_addon_bind(&paths, &instance, &app)?
            }
            AddonCmd::Unbind { instance, app } => {
                let app = match require_app(app, hint) {
                    Ok(app) => app,
                    Err(e) => return Err(e),
                };
                cli::addon::cmd_addon_unbind(&paths, &instance, &app)?
            }
            AddonCmd::Destroy { instance } => cli::addon::cmd_addon_destroy(&paths, &instance)?,
            AddonCmd::Backup { instance } => cli::addon::cmd_addon_backup(&paths, &instance)?,
        },
        Commands::Update => cli::apps::cmd_update()?,
        Commands::InstallPlugins { plugins } => {
            let only = if plugins.is_empty() {
                None
            } else {
                Some(plugins)
            };
            cli::apps::cmd_install_plugins(&paths, only)?
        }
        Commands::PluginEmit {
            event_name,
            data,
            app,
        } => cli::apps::cmd_plugin_emit(&paths, &event_name, &data, app.as_deref())?,
        Commands::Supervisor => cli::apps::cmd_supervisor(&paths)?,
        Commands::Plugin(cmd) => match cmd {
            PluginCmd::List => cli::client_plugins::cmd_plugin_list()?,
            PluginCmd::Exists { name } => cli::client_plugins::cmd_plugin_exists(&name)?,
        },
        Commands::Plugins(cmd) => match cmd {
            PluginsCmd::Install { source } => cli::plugins::cmd_plugins_install(&paths, &source)?,
            PluginsCmd::Scaffold {
                name,
                plugin_type,
                dir,
            } => cli::plugins::cmd_plugins_scaffold(&name, &plugin_type, dir.as_deref())?,
            PluginsCmd::List => cli::plugins::cmd_plugins_list(&paths)?,
            PluginsCmd::Remove { name } => cli::plugins::cmd_plugins_remove(&paths, &name)?,
            PluginsCmd::Search { query } => cli::plugins::cmd_plugins_search(&paths, &query)?,
            PluginsCmd::Add { spec } => cli::plugins::cmd_plugins_add(&paths, &spec)?,
            PluginsCmd::Doctor => cli::plugins::cmd_plugins_doctor(&paths)?,
            PluginsCmd::Keygen { out } => cli::plugins::cmd_plugins_keygen(&out)?,
            PluginsCmd::Sign { bundle, key } => cli::plugins::cmd_plugins_sign(&bundle, &key)?,
            PluginsCmd::Trust(cmd) => match cmd {
                TrustCmd::Add { name, pubkey } => {
                    cli::plugins::cmd_trust_add(&paths, &name, &pubkey)?
                }
                TrustCmd::List => cli::plugins::cmd_trust_list(&paths)?,
                TrustCmd::Remove { name } => cli::plugins::cmd_trust_remove(&paths, &name)?,
            },
            PluginsCmd::Marketplace(cmd) => match cmd {
                MarketplaceCmd::Add { url, name } => {
                    cli::plugins::cmd_marketplace_add(&paths, &url, name.as_deref())?
                }
                MarketplaceCmd::List => cli::plugins::cmd_marketplace_list(&paths)?,
                MarketplaceCmd::Remove { name } => {
                    cli::plugins::cmd_marketplace_remove(&paths, &name)?
                }
            },
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
