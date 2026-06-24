//! `riku plugins` provider layer — install/list/remove manifest-based plugin
//! bundles via [`PluginInstaller`]. Handles user-facing output only.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::plugins::{MarketplaceService, PluginInstaller};
use crate::util::display;

/// `riku plugins install <source>`
pub fn cmd_plugins_install(paths: &RikuPaths, source: &str) -> Result<()> {
    display::info(&format!("Installing plugin from {source}..."));
    let manifest = PluginInstaller::new(paths).install(source)?;
    display::success(&format!(
        "Installed {} v{} ({:?})",
        manifest.name, manifest.version, manifest.plugin_type
    ));
    if manifest.checksum.is_some() {
        display::note("Checksum verified.");
    } else {
        display::warn("No checksum pinned in the manifest — installed unverified.");
    }
    Ok(())
}

/// `riku plugins list`
pub fn cmd_plugins_list(paths: &RikuPaths) -> Result<()> {
    let installed = PluginInstaller::new(paths).list();
    if installed.is_empty() {
        display::note("No plugin bundles installed. Add one: riku plugins install <source>");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = installed
        .iter()
        .map(|(manifest, lock)| {
            let verified = if lock.as_ref().is_some_and(|l| l.author_pinned) {
                "yes"
            } else {
                "no"
            };
            vec![
                manifest.name.clone(),
                manifest.version.clone(),
                format!("{:?}", manifest.plugin_type).to_lowercase(),
                verified.to_string(),
            ]
        })
        .collect();
    display::print_table(&["NAME", "VERSION", "TYPE", "VERIFIED"], &rows, 2);
    Ok(())
}

/// `riku plugins remove <name>`
pub fn cmd_plugins_remove(paths: &RikuPaths, name: &str) -> Result<()> {
    PluginInstaller::new(paths).remove(name)?;
    display::success(&format!("Removed plugin '{name}'."));
    Ok(())
}

/// `riku plugins search <query>`
pub fn cmd_plugins_search(paths: &RikuPaths, query: &str) -> Result<()> {
    let results = MarketplaceService::new(paths).search(query);
    if results.is_empty() {
        display::note("No matches. Register a marketplace: riku plugins marketplace add <git-url>");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = results
        .iter()
        .map(|(market, e)| {
            vec![
                e.name.clone(),
                e.plugin_type.clone().unwrap_or_default(),
                market.clone(),
                e.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    display::print_table(&["NAME", "TYPE", "MARKETPLACE", "DESCRIPTION"], &rows, 2);
    Ok(())
}

/// `riku plugins add <name|name@marketplace>`
pub fn cmd_plugins_add(paths: &RikuPaths, spec: &str) -> Result<()> {
    let (name, market) = parse_spec(spec)?;
    display::info(&format!(
        "Resolving '{name}'{}...",
        market.map(|m| format!(" from '{m}'")).unwrap_or_default()
    ));
    let manifest = MarketplaceService::new(paths).install_named(name, market)?;
    display::success(&format!(
        "Installed {} v{}",
        manifest.name, manifest.version
    ));
    if manifest.checksum.is_none() {
        display::warn("No checksum pinned in the manifest — installed unverified.");
    }
    Ok(())
}

/// Parse `name`, `name@marketplace`. Rejects an (unsupported) version segment.
fn parse_spec(spec: &str) -> Result<(&str, Option<&str>)> {
    let mut parts = spec.split('@');
    let name = parts.next().filter(|s| !s.is_empty());
    let market = parts.next();
    if parts.next().is_some() {
        anyhow::bail!("version pinning ('name@market@version') is not yet supported");
    }
    match name {
        Some(name) => Ok((name, market)),
        None => anyhow::bail!("empty plugin name in '{spec}'"),
    }
}

/// `riku plugins marketplace add <url> [--name]`
pub fn cmd_marketplace_add(paths: &RikuPaths, url: &str, name: Option<&str>) -> Result<()> {
    display::warn(
        "A marketplace can publish code that runs on this server. Only add ones you trust.",
    );
    let registered = MarketplaceService::new(paths).add(url, name)?;
    display::success(&format!("Registered marketplace '{registered}'."));
    Ok(())
}

/// `riku plugins marketplace list`
pub fn cmd_marketplace_list(paths: &RikuPaths) -> Result<()> {
    let markets = MarketplaceService::new(paths).list();
    if markets.is_empty() {
        display::note(
            "No marketplaces registered. Add one: riku plugins marketplace add <git-url>",
        );
        return Ok(());
    }
    let rows: Vec<Vec<String>> = markets
        .iter()
        .map(|m| vec![m.name.clone(), m.url.clone()])
        .collect();
    display::print_table(&["NAME", "URL"], &rows, 2);
    Ok(())
}

/// `riku plugins marketplace remove <name>`
pub fn cmd_marketplace_remove(paths: &RikuPaths, name: &str) -> Result<()> {
    MarketplaceService::new(paths).remove(name)?;
    display::success(&format!("Removed marketplace '{name}'."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_spec;

    #[test]
    fn parses_name_and_marketplace() {
        assert_eq!(parse_spec("postgres").unwrap(), ("postgres", None));
        assert_eq!(
            parse_spec("postgres@official").unwrap(),
            ("postgres", Some("official"))
        );
        assert!(parse_spec("a@b@c").is_err());
        assert!(parse_spec("").is_err());
    }
}
