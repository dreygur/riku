//! Plugin marketplace (ROADMAP E2): git-native, no central server. A
//! marketplace is a git repo whose `marketplace.toml` indexes plugins and their
//! sources. Registering one is an explicit trust decision; `search` reads only
//! the index, and the bundle payload is pulled (and checksum-verified) on `add`.

mod index;
mod registry;

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::config::RikuPaths;
use crate::plugins::install::{git_url, PluginInstaller};
use crate::plugins::manifest::PluginManifest;

pub use index::MarketplaceEntry;
pub use registry::MarketplaceRecord;

use registry::MarketplaceRegistry;

/// Business logic for marketplaces.
pub struct MarketplaceService<'a> {
    paths: &'a RikuPaths,
}

impl<'a> MarketplaceService<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn registry(&self) -> MarketplaceRegistry<'a> {
        MarketplaceRegistry::new(self.paths)
    }

    /// Register and clone a marketplace. Returns its resolved name.
    pub fn add(&self, url: &str, name: Option<&str>) -> Result<String> {
        let name = match name {
            Some(n) => n.to_string(),
            None => derive_name(url),
        };
        validate_name(&name)?;

        let registry = self.registry();
        if registry.get(&name).is_some() {
            bail!("marketplace '{name}' is already registered");
        }

        let dir = registry.dir(&name);
        let _ = std::fs::remove_dir_all(&dir);
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let clone_url = git_url(url).unwrap_or_else(|| url.to_string());
        let status = Command::new("git")
            .args(["clone", "--depth", "1", "--quiet", &clone_url])
            .arg(&dir)
            .status()
            .context("running git clone")?;
        if !status.success() {
            bail!("git clone of '{clone_url}' failed");
        }

        // Validate it is actually a marketplace before registering.
        if let Err(e) = index::read_index(&dir) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e.context("not a valid marketplace (no parseable marketplace.toml)"));
        }

        registry.upsert(MarketplaceRecord {
            name: name.clone(),
            url: url.to_string(),
        })?;
        Ok(name)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let registry = self.registry();
        if !registry.remove(name)? {
            bail!("marketplace '{name}' is not registered");
        }
        let _ = std::fs::remove_dir_all(registry.dir(name));
        Ok(())
    }

    pub fn list(&self) -> Vec<MarketplaceRecord> {
        self.registry().list()
    }

    /// Search registered marketplaces; returns `(marketplace, entry)` matches.
    pub fn search(&self, query: &str) -> Vec<(String, MarketplaceEntry)> {
        let q = query.to_lowercase();
        let mut out = Vec::new();
        for market in self.registry().list() {
            let Ok(entries) = index::read_index(&self.registry().dir(&market.name)) else {
                continue;
            };
            for entry in entries {
                let hay = format!(
                    "{} {}",
                    entry.name,
                    entry.description.clone().unwrap_or_default()
                )
                .to_lowercase();
                if q.is_empty() || hay.contains(&q) {
                    out.push((market.name.clone(), entry));
                }
            }
        }
        out
    }

    /// Resolve `name` (optionally scoped to `market`) to one index entry.
    fn resolve(&self, name: &str, market: Option<&str>) -> Result<(String, MarketplaceEntry)> {
        let mut matches: Vec<(String, MarketplaceEntry)> = self
            .search(name)
            .into_iter()
            .filter(|(m, e)| e.name == name && market.is_none_or(|want| want == m))
            .collect();

        match matches.len() {
            0 => bail!(
                "no plugin named '{name}' in {}",
                market
                    .map(|m| format!("marketplace '{m}'"))
                    .unwrap_or_else(|| "any registered marketplace".into())
            ),
            1 => Ok(matches.remove(0)),
            _ => {
                let markets: Vec<&str> = matches.iter().map(|(m, _)| m.as_str()).collect();
                bail!(
                    "'{name}' is in multiple marketplaces ({}) — disambiguate with {name}@<marketplace>",
                    markets.join(", ")
                )
            }
        }
    }

    /// Resolve `name[@market]` through the index and install it, optionally
    /// pinning a `version` (a git tag/branch on the plugin's source repo).
    pub fn install_named(
        &self,
        name: &str,
        market: Option<&str>,
        version: Option<&str>,
    ) -> Result<PluginManifest> {
        let (market_name, entry) = self.resolve(name, market)?;
        let source = self.installable_source(&market_name, &entry);
        PluginInstaller::new(self.paths).install_with_ref(&source, version)
    }

    /// A bundle `source` from the index is either a git URL / absolute path
    /// (used as-is) or a path relative to the marketplace clone.
    fn installable_source(&self, market: &str, entry: &MarketplaceEntry) -> String {
        if git_url(&entry.source).is_some() || Path::new(&entry.source).is_absolute() {
            entry.source.clone()
        } else {
            self.registry()
                .dir(market)
                .join(&entry.source)
                .to_string_lossy()
                .into_owned()
        }
    }
}

/// Derive a marketplace name from a URL: last path segment, minus `.git`.
fn derive_name(url: &str) -> String {
    let base = url.trim_end_matches('/').rsplit('/').next().unwrap_or(url);
    let base = base.strip_suffix(".git").unwrap_or(base);
    base.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("invalid marketplace name '{name}'"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_name_from_various_urls() {
        assert_eq!(
            derive_name("https://github.com/owner/riku-plugins.git"),
            "riku-plugins"
        );
        assert_eq!(derive_name("github:owner/store"), "store");
        assert_eq!(derive_name("/local/my_market/"), "my_market");
    }

    #[test]
    fn validate_name_rejects_unsafe() {
        assert!(validate_name("ok-name_1").is_ok());
        assert!(validate_name("../evil").is_err());
        assert!(validate_name("").is_err());
    }
}
