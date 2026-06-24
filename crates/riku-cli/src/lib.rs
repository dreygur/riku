//! CLI provider layer — module declarations and public re-exports.

// Dependency crates aliased as their former module names.
pub(crate) use riku_config as config;
pub(crate) use riku_deploy as deploy;
pub(crate) use riku_nginx as nginx;
pub(crate) use riku_supervisor as supervisor;
pub(crate) use riku_util as util;

pub mod addon;
pub mod agent;
pub mod apps;
pub mod backup;
#[allow(clippy::module_inception)]
pub mod cli;
pub mod client_plugins;
pub mod cmds;
pub mod container;
pub mod control_actions;
pub mod doctor;
pub mod git;
pub mod hooks;
pub mod plugins;
pub mod quickstart;
pub mod routing;
pub mod scp;
pub mod setup;

pub use cli::{Cli, Commands};
pub use cmds::{
    AddonCmd, AppsCmd, ConfigCmd, HookCmd, MarketplaceCmd, PluginCmd, PluginsCmd, StatsCmd,
    TrustCmd,
};
