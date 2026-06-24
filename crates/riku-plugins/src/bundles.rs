//! Discovery of manifest-based plugin bundles in the plugins directory.
//!
//! A bundle is a directory containing a `riku-plugin.toml`. This module finds
//! and parses them; invalid manifests are skipped with a warning so one bad
//! bundle never blocks discovery.

use std::path::{Path, PathBuf};

use super::manifest::{PluginManifest, PluginType};

/// Every valid plugin bundle under `plugin_root`, paired with its directory.
pub fn find_bundles(plugin_root: &Path) -> Vec<(PathBuf, PluginManifest)> {
    let mut out = Vec::new();
    let read_dir = match std::fs::read_dir(plugin_root) {
        Ok(rd) => rd,
        Err(_) => return out,
    };
    for entry in read_dir.flatten() {
        let bundle = entry.path();
        if !bundle.is_dir() || !bundle.join("riku-plugin.toml").exists() {
            continue;
        }
        match PluginManifest::from_dir(&bundle) {
            Ok(manifest) => out.push((bundle, manifest)),
            Err(e) => tracing::warn!("ignoring invalid plugin bundle {}: {e}", bundle.display()),
        }
    }
    out
}

/// The addon bundle providing `name`, if installed.
pub fn find_addon(plugin_root: &Path, name: &str) -> Option<(PathBuf, PluginManifest)> {
    find_bundles(plugin_root)
        .into_iter()
        .find(|(_, m)| m.plugin_type == PluginType::Addon && m.name == name)
}

/// The router bundle providing `name`, if installed.
pub fn find_router(plugin_root: &Path, name: &str) -> Option<(PathBuf, PluginManifest)> {
    find_bundles(plugin_root)
        .into_iter()
        .find(|(_, m)| m.plugin_type == PluginType::Router && m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_bundle(root: &Path, name: &str, ptype: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1\"\ntype=\"{ptype}\"\napi={}\nentry=\"bin/x\"\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
    }

    #[test]
    fn finds_addon_by_name_and_type() {
        let tmp = tempfile::tempdir().unwrap();
        write_bundle(tmp.path(), "postgres", "addon");
        write_bundle(tmp.path(), "slack", "notifier");

        let found = find_addon(tmp.path(), "postgres");
        assert!(found.is_some());
        // A notifier with the same lookup name is not an addon match.
        assert!(find_addon(tmp.path(), "slack").is_none());
        assert!(find_addon(tmp.path(), "missing").is_none());
    }

    #[test]
    fn finds_router_by_name_and_type() {
        let tmp = tempfile::tempdir().unwrap();
        write_bundle(tmp.path(), "caddy", "router");
        write_bundle(tmp.path(), "postgres", "addon");

        assert!(find_router(tmp.path(), "caddy").is_some());
        // An addon is not a router match, and vice versa.
        assert!(find_router(tmp.path(), "postgres").is_none());
        assert!(find_addon(tmp.path(), "caddy").is_none());
        assert!(find_router(tmp.path(), "missing").is_none());
    }

    #[test]
    fn find_bundles_on_missing_dir_is_empty() {
        assert!(find_bundles(Path::new("/no/such/dir")).is_empty());
    }
}
