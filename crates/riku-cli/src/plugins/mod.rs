//! `riku plugins` provider layer: install/list/remove manifest-based plugin
//! bundles via [`PluginInstaller`]. Handles user-facing output only.

use anyhow::Result;

use crate::config::RikuPaths;
use crate::util::display;
use riku_plugins::install::HealthStatus;
use riku_plugins::{MarketplaceService, PluginInstaller, PluginManifest};

/// Print the capabilities a plugin's manifest declares, so the operator sees
/// what they are granting (informed consent, Android-permission style).
fn print_capabilities(manifest: &PluginManifest) {
    let caps = &manifest.capabilities;
    let mut requested = Vec::new();
    if caps.network {
        requested.push("network".to_string());
    }
    if !caps.writes.is_empty() {
        requested.push(format!("writes {:?}", caps.writes));
    }
    if caps.privileged {
        requested.push("privileged".to_string());
    }
    if requested.is_empty() {
        display::note("Capabilities: none declared.");
    } else {
        display::warn(&format!("Capabilities granted: {}", requested.join(", ")));
    }
}

/// `riku plugins install <source>`
pub fn cmd_plugins_install(paths: &RikuPaths, source: &str) -> Result<()> {
    display::info(&format!("Installing plugin from {source}..."));
    let manifest = PluginInstaller::new(paths).install(source)?;
    display::success(&format!(
        "Installed {} v{} ({:?})",
        manifest.name, manifest.version, manifest.plugin_type
    ));
    report_trust(&manifest);
    print_capabilities(&manifest);
    Ok(())
}

/// Report how trustworthy the installed bundle is: a verified signature is the
/// strongest, then a pinned checksum, then nothing.
fn report_trust(manifest: &PluginManifest) {
    if manifest.signature.is_some() {
        display::note("Signature verified by a trusted publisher.");
    } else if manifest.checksum.is_some() {
        display::note("Checksum verified.");
    } else {
        display::warn("No signature or checksum in the manifest, installed unverified.");
    }
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
    let (name, market, version) = parse_spec(spec)?;
    display::info(&format!(
        "Resolving '{name}'{}{}...",
        market.map(|m| format!(" from '{m}'")).unwrap_or_default(),
        version.map(|v| format!(" at '{v}'")).unwrap_or_default()
    ));
    let manifest = MarketplaceService::new(paths).install_named(name, market, version)?;
    display::success(&format!(
        "Installed {} v{}",
        manifest.name, manifest.version
    ));
    report_trust(&manifest);
    print_capabilities(&manifest);
    Ok(())
}

/// `riku plugins doctor`: validate installed bundles (api + integrity).
pub fn cmd_plugins_doctor(paths: &RikuPaths) -> Result<()> {
    let results = PluginInstaller::new(paths).audit();
    if results.is_empty() {
        display::note("No plugin bundles installed.");
        return Ok(());
    }
    let mut failures = 0;
    for r in &results {
        match r.status {
            HealthStatus::Ok => display::success(&format!("{}, {}", r.name, r.detail)),
            HealthStatus::Warn => display::warn(&format!("{}, {}", r.name, r.detail)),
            HealthStatus::Fail => {
                failures += 1;
                display::error(&format!("{}, {}", r.name, r.detail));
            }
        }
    }
    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Parse `name[@marketplace[@version]]` into its parts (empty segments → None).
fn parse_spec(spec: &str) -> Result<(&str, Option<&str>, Option<&str>)> {
    let mut parts = spec.split('@');
    let name = parts.next().filter(|s| !s.is_empty());
    let market = parts.next().filter(|s| !s.is_empty());
    let version = parts.next().filter(|s| !s.is_empty());
    if parts.next().is_some() {
        anyhow::bail!("invalid spec '{spec}' (expected name[@marketplace[@version]])");
    }
    match name {
        Some(name) => Ok((name, market, version)),
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

/// `riku plugins trust add <name> <pubkey>`
pub fn cmd_trust_add(paths: &RikuPaths, name: &str, pubkey: &str) -> Result<()> {
    riku_plugins::signing::Keyring::new(paths).add(name, pubkey)?;
    display::success(&format!("Trusted publisher key '{name}'."));
    Ok(())
}

/// `riku plugins trust list`
pub fn cmd_trust_list(paths: &RikuPaths) -> Result<()> {
    let keys = riku_plugins::signing::Keyring::new(paths).list();
    if keys.is_empty() {
        display::note("No trusted keys. Add one: riku plugins trust add <name> <pubkey>");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = keys
        .iter()
        .map(|k| vec![k.name.clone(), k.pubkey.clone()])
        .collect();
    display::print_table(&["NAME", "PUBLIC KEY"], &rows, 2);
    Ok(())
}

/// `riku plugins trust remove <name>`
pub fn cmd_trust_remove(paths: &RikuPaths, name: &str) -> Result<()> {
    if !riku_plugins::signing::Keyring::new(paths).remove(name)? {
        anyhow::bail!("no trusted key named '{name}'");
    }
    display::success(&format!("Removed trusted key '{name}'."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_spec;

    #[test]
    fn parses_name_marketplace_and_version() {
        assert_eq!(parse_spec("postgres").unwrap(), ("postgres", None, None));
        assert_eq!(
            parse_spec("postgres@official").unwrap(),
            ("postgres", Some("official"), None)
        );
        assert_eq!(
            parse_spec("postgres@official@v1.2.0").unwrap(),
            ("postgres", Some("official"), Some("v1.2.0"))
        );
        // Empty marketplace, pinned version: name@@version.
        assert_eq!(
            parse_spec("postgres@@v1.2.0").unwrap(),
            ("postgres", None, Some("v1.2.0"))
        );
        assert!(parse_spec("a@b@c@d").is_err());
        assert!(parse_spec("").is_err());
    }
}
