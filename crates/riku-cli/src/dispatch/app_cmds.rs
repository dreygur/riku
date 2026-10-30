//! Local dispatch for the app, config, process, stats and addon commands.

use anyhow::{anyhow, Result};

use crate::cli::require_app;
use crate::cmds::{AddonCmd, AppsCmd, ConfigCmd, StatsCmd};
use crate::config::RikuPaths;
use crate::{addon, apps};

pub(super) fn run_apps(paths: &RikuPaths, cmd: Option<AppsCmd>) -> Result<()> {
    match cmd {
        Some(AppsCmd::Create { name }) => apps::cmd_apps_create(paths, &name),
        Some(AppsCmd::Info { name }) => apps::cmd_apps_info(paths, &name),
        Some(AppsCmd::Destroy { name }) => apps::cmd_destroy(paths, &name),
        None => apps::cmd_apps(paths),
    }
}

pub(super) fn run_config(paths: &RikuPaths, cmd: ConfigCmd) -> Result<()> {
    match cmd {
        ConfigCmd::Show { app } => {
            require_app(app).and_then(|app| apps::cmd_config_show(paths, &app))
        }
        ConfigCmd::Get { app, key } => apps::cmd_config_get(paths, &app, &key),
        ConfigCmd::Set { app, settings } => apps::cmd_config_set(paths, &app, &settings),
        ConfigCmd::Unset { app, keys } => apps::cmd_config_unset(paths, &app, &keys),
        ConfigCmd::Live { app } => {
            require_app(app).and_then(|app| apps::cmd_config_live(paths, &app))
        }
    }
}

pub(super) fn run_logs(
    paths: &RikuPaths,
    app: Option<String>,
    process: &str,
    deploy: bool,
    follow: bool,
) -> Result<()> {
    match require_app(app) {
        Ok(app) if deploy => apps::cmd_deploy_logs(paths, &app, follow),
        Ok(app) => apps::cmd_logs(paths, &app, process),
        Err(e) => Err(e),
    }
}

/// An omitted app means every app, except when scaling.
pub(super) fn run_ps(
    paths: &RikuPaths,
    app: Option<String>,
    verbose: bool,
    scale: &[String],
) -> Result<()> {
    match app {
        Some(app) if !scale.is_empty() => apps::cmd_ps_scale(paths, &app, scale),
        None if !scale.is_empty() => Err(anyhow!("App name required for scaling")),
        Some(app) => apps::cmd_ps_show(paths, &app, verbose),
        None => apps::cmd_ps_all(paths, true),
    }
}

pub(super) fn run_stats(paths: &RikuPaths, cmd: StatsCmd) -> Result<()> {
    match cmd {
        StatsCmd::All => apps::cmd_stats_all(paths),
        StatsCmd::App { app } => require_app(app).and_then(|app| apps::cmd_stats_app(paths, &app)),
    }
}

pub(super) fn run_restart(paths: &RikuPaths, app: Option<String>, hot: bool) -> Result<()> {
    match require_app(app) {
        Ok(app) if hot => apps::cmd_hot_reload(paths, &app),
        Ok(app) => apps::cmd_restart(paths, &app),
        Err(e) => Err(e),
    }
}

pub(super) fn run_addon(paths: &RikuPaths, cmd: AddonCmd) -> Result<()> {
    match cmd {
        AddonCmd::List => addon::cmd_addon_list(paths),
        AddonCmd::Create { plugin, name } => addon::cmd_addon_create(paths, &plugin, &name),
        AddonCmd::Bind { instance, app } => {
            require_app(app).and_then(|app| addon::cmd_addon_bind(paths, &instance, &app))
        }
        AddonCmd::Unbind { instance, app } => {
            require_app(app).and_then(|app| addon::cmd_addon_unbind(paths, &instance, &app))
        }
        AddonCmd::Destroy { instance } => addon::cmd_addon_destroy(paths, &instance),
        AddonCmd::Backup { instance } => addon::cmd_addon_backup(paths, &instance),
    }
}
