//! Addon instance registry — repository layer (`PLUGIN_PROTOCOL.md` §6.1).
//!
//! Each provisioned addon instance has one record under
//! `~/.riku/addons/instances/<instance>.toml` tracking which addon owns it and
//! which apps are bound (and the env keys injected into each, so `unbind` can
//! remove exactly what `bind` added). Pure persistence — no plugin execution.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;

/// Persistent record of one provisioned addon instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceRecord {
    /// Addon plugin that owns this instance (manifest `name`).
    pub plugin: String,
    /// Operator-chosen instance name (unique across addons).
    pub instance: String,
    /// Bound apps → the env keys this instance injected into each app's ENV.
    #[serde(default)]
    pub bindings: BTreeMap<String, Vec<String>>,
}

impl InstanceRecord {
    pub fn new(plugin: impl Into<String>, instance: impl Into<String>) -> Self {
        Self {
            plugin: plugin.into(),
            instance: instance.into(),
            bindings: BTreeMap::new(),
        }
    }
}

/// Repository for [`InstanceRecord`]s under `~/.riku/addons/instances/`.
pub struct InstanceStore<'a> {
    paths: &'a RikuPaths,
}

impl<'a> InstanceStore<'a> {
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    fn dir(&self) -> PathBuf {
        self.paths.riku_root.join("addons").join("instances")
    }

    fn record_path(&self, instance: &str) -> PathBuf {
        self.dir().join(format!("{instance}.toml"))
    }

    /// Whether an instance with this name is registered.
    pub fn exists(&self, instance: &str) -> bool {
        self.record_path(instance).exists()
    }

    /// Load one instance record.
    pub fn load(&self, instance: &str) -> Result<InstanceRecord> {
        let path = self.record_path(instance);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("no such addon instance '{instance}'"))?;
        toml::from_str(&text).with_context(|| format!("corrupt instance record {}", path.display()))
    }

    /// Persist a record atomically.
    pub fn save(&self, record: &InstanceRecord) -> Result<()> {
        std::fs::create_dir_all(self.dir())?;
        let text = toml::to_string_pretty(record)?;
        crate::util::write_atomic(&self.record_path(&record.instance), text.as_bytes())
    }

    /// Remove a record (no-op if already gone).
    pub fn delete(&self, instance: &str) -> Result<()> {
        let path = self.record_path(instance);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// All registered instances, sorted by name. Corrupt records are skipped.
    pub fn list(&self) -> Vec<InstanceRecord> {
        let mut out = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(self.dir()) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(record) = toml::from_str::<InstanceRecord>(&text) {
                        out.push(record);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.instance.cmp(&b.instance));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    #[test]
    fn save_load_roundtrip() {
        let (_tmp, paths) = store_paths();
        let store = InstanceStore::new(&paths);
        let mut rec = InstanceRecord::new("postgres", "db1");
        rec.bindings
            .insert("myapp".into(), vec!["DATABASE_URL".into()]);
        store.save(&rec).unwrap();

        assert!(store.exists("db1"));
        assert_eq!(store.load("db1").unwrap(), rec);
    }

    #[test]
    fn list_is_sorted_and_delete_works() {
        let (_tmp, paths) = store_paths();
        let store = InstanceStore::new(&paths);
        store
            .save(&InstanceRecord::new("postgres", "zeta"))
            .unwrap();
        store.save(&InstanceRecord::new("redis", "alpha")).unwrap();

        let names: Vec<_> = store.list().into_iter().map(|r| r.instance).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);

        store.delete("alpha").unwrap();
        assert!(!store.exists("alpha"));
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn load_missing_instance_errors() {
        let (_tmp, paths) = store_paths();
        assert!(InstanceStore::new(&paths).load("ghost").is_err());
    }
}
