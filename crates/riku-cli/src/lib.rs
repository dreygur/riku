//! CLI provider layer: module declarations and public re-exports.

// Dependency crates aliased as their former module names.
pub(crate) use riku_caddy as caddy;
pub(crate) use riku_config as config;
pub(crate) use riku_deploy as deploy;
pub(crate) use riku_nginx as nginx;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

pub use riku_args::{address, app_hint, cli, cmds, help, parse, placement};
pub use riku_remote::{client_plugins, quickstart, remote, routing};

pub mod addon;
pub mod agent;
pub mod apps;
pub mod backup;
pub mod container;
pub mod control_actions;
pub mod dispatch;
pub mod doctor;
pub mod git;
pub mod hooks;
pub mod plugins;
pub mod scp;
pub mod setup;
pub mod ssh_command;

pub use riku_args::cli::{require_app, Cli, Commands};
pub use riku_args::cmds::{
    AddonCmd, AppsCmd, ConfigCmd, HookCmd, MarketplaceCmd, PluginCmd, PluginsCmd, ServerCmd,
    StatsCmd, TrustCmd,
};
