//! Plugin lockfile — repository layer (`PLUGIN_PROTOCOL.md` / ROADMAP E2).
//!
//! `~/.riku/riku-plugins.lock` records every installed bundle: its resolved
//! name, the source it came from, the version, and the verified checksum. This
//! pins exactly what executable code is on the host — no silent auto-update.

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

/// One locked plugin install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    pub name: String,
    pub source: String,
    pub version: String,
    /// The computed `sha256:` digest of the installed entry — pins the exact
    /// bytes on disk for later tamper detection, whether or not the author
    /// attested it.
    #[serde(default)]
    pub checksum: Option<String>,
    /// Whether the manifest itself pinned a checksum that matched on install
    /// (author-attested), versus a digest we merely recorded.
    #[serde(default)]
    pub author_pinned: bool,
    /// The trusted key name whose Ed25519 signature verified this bundle on
    /// install, if it was signed by a trusted publisher.
    #[serde(default)]
    pub signer: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LockDoc {
    #[serde(default, rename = "plugin")]
    plugins: Vec<LockEntry>,
}

/// Repository for the lockfile.
pub struct Lockfile<'a> {
    paths: &'a RikuPaths,
}

impl<'a> Lockfile<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn path(&self) -> PathBuf {
        self.paths.riku_root.join("riku-plugins.lock")
    }

    fn load(&self) -> LockDoc {
        std::fs::read_to_string(self.path())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, doc: &LockDoc) -> Result<()> {
        std::fs::create_dir_all(self.paths.riku_root.as_path())?;
        crate::util::write_atomic(&self.path(), toml::to_string_pretty(doc)?.as_bytes())
    }

    /// All locked entries, sorted by name.
    pub fn entries(&self) -> Vec<LockEntry> {
        let mut entries = self.load().plugins;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    /// Insert or replace the entry for `entry.name`.
    pub fn upsert(&self, entry: LockEntry) -> Result<()> {
        let mut doc = self.load();
        doc.plugins.retain(|e| e.name != entry.name);
        doc.plugins.push(entry);
        self.save(&doc)
    }

    /// Remove the entry named `name`; returns whether one existed.
    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut doc = self.load();
        let before = doc.plugins.len();
        doc.plugins.retain(|e| e.name != name);
        let removed = doc.plugins.len() != before;
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

    fn entry(name: &str) -> LockEntry {
        LockEntry {
            name: name.to_string(),
            source: "./somewhere".to_string(),
            version: "1.0.0".to_string(),
            checksum: Some("sha256:abc".to_string()),
            author_pinned: false,
            signer: None,
        }
    }

    #[test]
    fn upsert_replaces_by_name_and_sorts() {
        let (_tmp, paths) = paths();
        let lock = Lockfile::new(&paths);
        lock.upsert(entry("zeta")).unwrap();
        lock.upsert(entry("alpha")).unwrap();
        // Re-upsert alpha with a new version replaces, not duplicates.
        let mut a2 = entry("alpha");
        a2.version = "2.0.0".to_string();
        lock.upsert(a2).unwrap();

        let names: Vec<_> = lock.entries().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
        assert_eq!(lock.entries()[0].version, "2.0.0");
    }

    #[test]
    fn remove_reports_presence() {
        let (_tmp, paths) = paths();
        let lock = Lockfile::new(&paths);
        lock.upsert(entry("p")).unwrap();
        assert!(lock.remove("p").unwrap());
        assert!(!lock.remove("p").unwrap());
        assert!(lock.entries().is_empty());
    }
}
