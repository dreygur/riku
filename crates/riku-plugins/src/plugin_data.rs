//! Generic plugin-owned scratch directory (`PLUGIN_PROTOCOL.md` §3).
//!
//! Every manifest-based plugin invocation gets its own directory under
//! `data_root/plugin-data/<plugin-name>/`, auto-created lazily on first use.
//! This generalizes the addon seam's existing per-*instance*
//! `RIKU_ADDON_DATA_PATH` (which stays as-is, unchanged) to a per-*plugin*
//! directory available to every seam — it's net-new storage the plugin owns,
//! not access to riku's own control-plane state (`RikuPaths`, ENV, worker
//! TOML remain kernel-only, per `PLUGIN_PROTOCOL.md` §9).

use std::path::PathBuf;

use crate::config::RikuPaths;

/// The scratch directory for `plugin_name`, created if it doesn't exist yet.
/// Returns `None` (rather than failing the caller's dispatch) if creation
/// fails — this is an additive convenience, never a hard requirement for a
/// plugin invocation to proceed.
pub fn plugin_data_path(paths: &RikuPaths, plugin_name: &str) -> Option<PathBuf> {
    let dir = paths.data_root.join("plugin-data").join(plugin_name);
    match std::fs::create_dir_all(&dir) {
        Ok(()) => Some(dir),
        Err(e) => {
            tracing::warn!(
                plugin = plugin_name,
                "could not create plugin data directory {}: {e}",
                dir.display()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_returns_a_per_plugin_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());

        let dir = plugin_data_path(&paths, "my-plugin").unwrap();
        assert!(dir.exists());
        assert_eq!(dir, paths.data_root.join("plugin-data").join("my-plugin"));
    }

    #[test]
    fn different_plugins_get_different_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());

        let a = plugin_data_path(&paths, "plugin-a").unwrap();
        let b = plugin_data_path(&paths, "plugin-b").unwrap();
        assert_ne!(a, b);
    }
}
