//! Registered marketplaces — repository layer.
//!
//! `~/.riku/marketplaces.toml` records which marketplaces are registered; each
//! is cloned to `~/.riku/marketplaces/<name>/`. Registering a marketplace lets
//! it publish code that runs on the server, so it is an explicit, opt-in trust
//! decision (the service warns on `add`).

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

/// A registered marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceRecord {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryDoc {
    #[serde(default, rename = "marketplace")]
    marketplaces: Vec<MarketplaceRecord>,
}

/// Repository for registered marketplaces.
pub struct MarketplaceRegistry<'a> {
    paths: &'a RikuPaths,
}

impl<'a> MarketplaceRegistry<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn file(&self) -> PathBuf {
        self.paths.riku_root.join("marketplaces.toml")
    }

    /// Clone directory for a marketplace.
    pub fn dir(&self, name: &str) -> PathBuf {
        self.paths.riku_root.join("marketplaces").join(name)
    }

    fn load(&self) -> RegistryDoc {
        std::fs::read_to_string(self.file())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, doc: &RegistryDoc) -> Result<()> {
        std::fs::create_dir_all(self.paths.riku_root.as_path())?;
        crate::util::write_atomic(&self.file(), toml::to_string_pretty(doc)?.as_bytes())
    }

    /// All registered marketplaces, sorted by name.
    pub fn list(&self) -> Vec<MarketplaceRecord> {
        let mut m = self.load().marketplaces;
        m.sort_by(|a, b| a.name.cmp(&b.name));
        m
    }

    pub fn get(&self, name: &str) -> Option<MarketplaceRecord> {
        self.load()
            .marketplaces
            .into_iter()
            .find(|m| m.name == name)
    }

    pub fn upsert(&self, record: MarketplaceRecord) -> Result<()> {
        let mut doc = self.load();
        doc.marketplaces.retain(|m| m.name != record.name);
        doc.marketplaces.push(record);
        self.save(&doc)
    }

    /// Remove a record; returns whether one existed.
    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut doc = self.load();
        let before = doc.marketplaces.len();
        doc.marketplaces.retain(|m| m.name != name);
        let removed = doc.marketplaces.len() != before;
        self.save(&doc)?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    #[test]
    fn upsert_get_remove() {
        let (_tmp, paths) = paths();
        let reg = MarketplaceRegistry::new(&paths);
        reg.upsert(MarketplaceRecord {
            name: "official".into(),
            url: "https://example.com/m.git".into(),
        })
        .unwrap();
        assert_eq!(
            reg.get("official").unwrap().url,
            "https://example.com/m.git"
        );
        assert_eq!(reg.list().len(), 1);
        assert!(reg.remove("official").unwrap());
        assert!(reg.get("official").is_none());
    }
}
