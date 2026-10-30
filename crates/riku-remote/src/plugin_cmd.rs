//! Dispatch for the `plugin` command, which inspects client plugins.

use anyhow::Result;

use crate::client_plugins;
use crate::cmds::PluginCmd;

/// Runs a `plugin` subcommand against the local client plugin directory.
pub fn run_plugin(cmd: PluginCmd) -> Result<()> {
    match cmd {
        PluginCmd::List => client_plugins::cmd_plugin_list(),
        PluginCmd::Exists { name } => client_plugins::cmd_plugin_exists(&name),
    }
}
