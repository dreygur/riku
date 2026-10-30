//! Local dispatch for the client plugin, plugin bundle and hook commands.

use anyhow::Result;

use crate::cmds::{HookCmd, MarketplaceCmd, PluginsCmd, TrustCmd};
use crate::config::RikuPaths;
use crate::{hooks, plugins};

fn run_trust(paths: &RikuPaths, cmd: TrustCmd) -> Result<()> {
    match cmd {
        TrustCmd::Add { name, pubkey } => plugins::cmd_trust_add(paths, &name, &pubkey),
        TrustCmd::List => plugins::cmd_trust_list(paths),
        TrustCmd::Remove { name } => plugins::cmd_trust_remove(paths, &name),
    }
}

fn run_marketplace(paths: &RikuPaths, cmd: MarketplaceCmd) -> Result<()> {
    match cmd {
        MarketplaceCmd::Add { url, name } => {
            plugins::cmd_marketplace_add(paths, &url, name.as_deref())
        }
        MarketplaceCmd::List => plugins::cmd_marketplace_list(paths),
        MarketplaceCmd::Remove { name } => plugins::cmd_marketplace_remove(paths, &name),
    }
}

pub(super) fn run_plugins(paths: &RikuPaths, cmd: PluginsCmd) -> Result<()> {
    match cmd {
        PluginsCmd::Install { source } => plugins::cmd_plugins_install(paths, &source),
        PluginsCmd::Scaffold {
            name,
            plugin_type,
            dir,
        } => plugins::cmd_plugins_scaffold(&name, &plugin_type, dir.as_deref()),
        PluginsCmd::List => plugins::cmd_plugins_list(paths),
        PluginsCmd::Remove { name } => plugins::cmd_plugins_remove(paths, &name),
        PluginsCmd::Search { query } => plugins::cmd_plugins_search(paths, &query),
        PluginsCmd::Add { spec } => plugins::cmd_plugins_add(paths, &spec),
        PluginsCmd::Doctor => plugins::cmd_plugins_doctor(paths),
        PluginsCmd::Keygen { out } => plugins::cmd_plugins_keygen(&out),
        PluginsCmd::Sign { bundle, key } => plugins::cmd_plugins_sign(&bundle, &key),
        PluginsCmd::Trust(cmd) => run_trust(paths, cmd),
        PluginsCmd::Marketplace(cmd) => run_marketplace(paths, cmd),
    }
}

pub(super) fn run_hook(paths: &RikuPaths, cmd: HookCmd) -> Result<()> {
    match cmd {
        HookCmd::List => hooks::cmd_hook_list(paths),
        HookCmd::Check { name } => hooks::cmd_hook_check(paths, &name),
    }
}
