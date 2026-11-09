//! Runtime plugins installed as bundle directories, the layout
//! `riku plugins scaffold` writes.
//!
//! Security: the manifest loader already refuses an `entry` that is absolute
//! or contains `..` (`riku_bundle::manifest`), so the resolved entry always
//! stays inside the bundle directory.

use std::path::Path;

use super::bundles::find_bundles;
use super::manifest::PluginType;
use super::runtime::RuntimePlugin;

fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

fn runtime_bundles(plugin_root: &Path) -> Vec<RuntimePlugin> {
    find_bundles(plugin_root)
        .into_iter()
        .filter(|(_, manifest)| manifest.plugin_type == PluginType::Runtime)
        .filter_map(|(dir, manifest)| {
            let path = manifest.entry_path(&dir);
            if !is_executable_file(&path) {
                tracing::warn!(
                    "ignoring runtime bundle {}: entry is not executable",
                    dir.display()
                );
                return None;
            }
            Some(RuntimePlugin {
                name: manifest.name,
                path,
            })
        })
        .collect()
}

/// Add the runtime bundles under `plugin_root` to `plugins`. On a name clash
/// the plain file plugin wins, so an existing install behaves as before.
pub(crate) fn add_runtime_bundles(plugins: &mut Vec<RuntimePlugin>, plugin_root: &Path) {
    for bundle in runtime_bundles(plugin_root) {
        if let Some(file) = plugins.iter().find(|p| p.name == bundle.name) {
            tracing::warn!(
                "runtime plugin '{}' exists as {} and {}; using the file",
                bundle.name,
                file.path.display(),
                bundle.path.display()
            );
            continue;
        }
        plugins.push(bundle);
    }
}
