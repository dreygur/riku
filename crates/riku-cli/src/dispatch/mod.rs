//! Runs a parsed command on this machine.
//!
//! The binary calls this after target resolution decided the command runs
//! here and after `app_hint::apply_app_hint` filled any omitted app from the
//! hint, so an app that is still missing here is the `app name required`
//! error. `dashboard` is not handled here: riku-dashboard depends on this
//! crate, so the binary dispatches it itself.

mod app_cmds;
mod plugins;

use anyhow::{anyhow, Result};

use crate::agent::auth::AgentSession;
use crate::cli::{require_app, Commands};
use crate::config::RikuPaths;
use crate::{agent, apps, backup, container, doctor, git, quickstart, scp, setup};

/// What `riku agent` was asked to do. The flags are checked in this order.
enum AgentMode {
    Intro,
    Schema,
    Help(Option<String>),
    Run {
        command: String,
        args: Vec<String>,
        confirm: Option<String>,
    },
}

fn agent_mode(
    (intro, schema, agent_help): (bool, bool, bool),
    command: Option<String>,
    args: Vec<String>,
    confirm: Option<String>,
) -> AgentMode {
    if intro {
        return AgentMode::Intro;
    }
    if schema {
        return AgentMode::Schema;
    }
    match command {
        Some(command) if !agent_help => AgentMode::Run {
            command,
            args,
            confirm,
        },
        other => AgentMode::Help(other),
    }
}

fn run_agent(paths: &RikuPaths, session: &AgentSession, mode: AgentMode) -> Result<()> {
    match mode {
        AgentMode::Intro => agent::cmd_agent_intro(paths, session),
        AgentMode::Schema => agent::cmd_agent_schema(),
        AgentMode::Help(command) => agent::cmd_agent_help(command.as_deref()),
        AgentMode::Run {
            command,
            args,
            confirm,
        } => agent::cmd_agent_execute(paths, session, &command, &args, confirm.as_deref()),
    }
}

/// Only returns on failure: success either execs the real worker command or
/// becomes the signal forwarding shim and `_exit`s.
fn run_ns_shim() -> Result<()> {
    let root = match std::env::var("RIKU_NS_ROOT") {
        Ok(root) => root,
        Err(_) => return Err(anyhow!("__ns-shim: RIKU_NS_ROOT not set")),
    };
    let command = match std::env::var("RIKU_NS_CMD") {
        Ok(command) => command,
        Err(_) => return Err(anyhow!("__ns-shim: RIKU_NS_CMD not set")),
    };
    crate::supervisor::process::isolation::exec_isolated(std::path::Path::new(&root), &command)
        .map_err(anyhow::Error::from)
}

fn install_plugins(paths: &RikuPaths, plugins: Vec<String>) -> Result<()> {
    let only = if plugins.is_empty() {
        None
    } else {
        Some(plugins)
    };
    apps::cmd_install_plugins(paths, only)
}

/// Runs `command` here. `dashboard` is an error: the binary handles it.
pub fn run_local(paths: &RikuPaths, command: Commands) -> Result<()> {
    match command {
        Commands::Agent {
            intro,
            schema,
            agent_help,
            scope,
            agent_id,
            command,
            args,
            confirm,
            json: _,
        } => {
            let session = AgentSession::from_flags(scope.as_deref(), agent_id.as_deref());
            let mode = agent_mode((intro, schema, agent_help), command, args, confirm);
            run_agent(paths, &session, mode)
        }
        Commands::Apps { cmd } => app_cmds::run_apps(paths, cmd),
        Commands::Config(cmd) => app_cmds::run_config(paths, cmd),
        Commands::Container { cmd } => {
            container::cmd_container(container::ContainerCmd { command: cmd }, paths)
        }
        Commands::Deploy { app, from } => {
            require_app(app).and_then(|app| apps::cmd_deploy(paths, &app, from.as_deref()))
        }
        Commands::Destroy { app } => apps::cmd_destroy(paths, &app),
        Commands::Logs {
            app,
            process,
            deploy,
            follow,
        } => app_cmds::run_logs(paths, app, &process, deploy, follow),
        Commands::Ps {
            app,
            verbose,
            scale,
        } => app_cmds::run_ps(paths, app, verbose, &scale),
        Commands::Stats(cmd) => app_cmds::run_stats(paths, cmd),
        Commands::Server(cmd) => crate::remote::server_cmd::run(cmd),
        Commands::Run { app, cmd } => apps::cmd_run(paths, &app, &cmd),
        Commands::Restart { app, hot } => app_cmds::run_restart(paths, app, hot),
        Commands::Stop { app } => require_app(app).and_then(|app| apps::cmd_stop(paths, &app)),
        Commands::Rollback { app, to, list } => {
            require_app(app).and_then(|app| apps::cmd_rollback(paths, &app, to.as_deref(), list))
        }
        Commands::Init {
            no_systemd,
            ssh_key,
            add_key,
            user_steps,
        } => setup::cmd_init(setup::InitOptions {
            no_systemd,
            user_steps,
            ssh_key,
            add_keys: add_key,
        }),
        Commands::Quickstart {
            name,
            runtime,
            remote,
        } => quickstart::cmd_quickstart(&name, &runtime, remote.as_deref()),
        Commands::Backup { app, out } => {
            require_app(app).and_then(|app| backup::cmd_backup(paths, &app, out.as_deref()))
        }
        Commands::Restore { app, file } => backup::cmd_restore(paths, &app, &file),
        Commands::Doctor { fix } => doctor::cmd_doctor(paths, fix),
        Commands::Dashboard { .. } => Err(anyhow!("dashboard is run by the riku binary")),
        Commands::Addon(cmd) => app_cmds::run_addon(paths, cmd),
        Commands::Update => apps::cmd_update(),
        Commands::InstallPlugins { plugins } => install_plugins(paths, plugins),
        Commands::PluginEmit {
            event_name,
            data,
            app,
        } => apps::cmd_plugin_emit(paths, &event_name, &data, app.as_deref()),
        Commands::Supervisor => apps::cmd_supervisor(paths),
        Commands::Plugin(cmd) => riku_remote::plugin_cmd::run_plugin(cmd),
        Commands::Plugins(cmd) => plugins::run_plugins(paths, cmd),
        Commands::Hook(cmd) => plugins::run_hook(paths, cmd),
        Commands::GitHook { app, repo_path } => {
            git::cmd_git_hook(paths, &app, repo_path.as_deref())
        }
        Commands::GitReceivePack { app } => git::cmd_git_receive_pack(paths, &app),
        Commands::GitUploadPack { app } => git::cmd_git_upload_pack(paths, &app),
        Commands::Scp { args } => scp::cmd_scp(paths, &args),
        Commands::NsShim => run_ns_shim(),
        Commands::DumpState => apps::cmd_dump_state(paths),
    }
}
