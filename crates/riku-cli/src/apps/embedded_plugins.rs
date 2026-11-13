//! The shell runtime plugins and the `riku-notify` bundle, built into riku
//! from the repo's `plugins/` directory.
//!
//! Installing one writes these bytes out, so it needs no network and runs
//! exactly what this riku was built with: there is nothing to download or
//! verify.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::bundled_record;
use super::plugin_write::write_plugin_file;

const EXECUTABLE: u32 = 0o755;
const READABLE: u32 = 0o644;

/// One file of an embedded plugin: the directory under the plugin root it
/// goes in (empty for the root itself), its name, bytes and mode.
pub(crate) struct EmbeddedFile {
    pub dir: &'static str,
    pub name: &'static str,
    pub bytes: &'static [u8],
    pub mode: u32,
}

impl EmbeddedFile {
    /// Where the file goes, relative to the plugin root: `python`,
    /// `riku-notify/bin/on-event`.
    pub(crate) fn relative_path(&self) -> String {
        if self.dir.is_empty() {
            self.name.to_string()
        } else {
            format!("{}/{}", self.dir, self.name)
        }
    }
}

/// A plugin riku carries, by the name `riku install-plugins` uses.
pub(crate) struct EmbeddedPlugin {
    pub name: &'static str,
    pub files: &'static [EmbeddedFile],
}

macro_rules! runtime {
    ($name:literal) => {
        EmbeddedPlugin {
            name: $name,
            files: &[EmbeddedFile {
                dir: "",
                name: $name,
                bytes: include_bytes!(concat!("../../../../plugins/", $name)),
                mode: EXECUTABLE,
            }],
        }
    };
}

/// Every embedded plugin. The runtimes come first, in the order of
/// `riku_util::runtime_markers::BUNDLED_RUNTIMES`.
pub(crate) const EMBEDDED: &[EmbeddedPlugin] = &[
    runtime!("go"),
    runtime!("node"),
    runtime!("python"),
    runtime!("ruby"),
    runtime!("rust-lang"),
    EmbeddedPlugin {
        name: "riku-notify",
        files: &[
            EmbeddedFile {
                dir: "riku-notify",
                name: "riku-plugin.toml",
                bytes: include_bytes!("../../../../plugins/riku-notify/riku-plugin.toml"),
                mode: READABLE,
            },
            EmbeddedFile {
                dir: "riku-notify/bin",
                name: "on-event",
                bytes: include_bytes!("../../../../plugins/riku-notify/bin/on-event"),
                mode: EXECUTABLE,
            },
        ],
    },
];

/// The embedded runtime plugins, installed when no names are given.
pub(crate) const EMBEDDED_RUNTIMES: &[&str] = &["go", "node", "python", "ruby", "rust-lang"];

/// The embedded plugin called `name`.
pub(crate) fn find(name: &str) -> Option<&'static EmbeddedPlugin> {
    EMBEDDED.iter().find(|plugin| plugin.name == name)
}

/// Write `plugin`'s files under `plugin_root` and note their hashes in the
/// bundled record, so a later `riku init` can tell them from edited copies.
#[allow(clippy::question_mark)]
pub(crate) fn write_out(plugin: &EmbeddedPlugin, plugin_root: &Path) -> Result<()> {
    for file in plugin.files {
        let dir = plugin_root.join(file.dir);
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(anyhow!("cannot create {}: {e}", dir.display()));
        }
        if let Err(e) = write_plugin_file(&dir, file.name, file.bytes, file.mode) {
            return Err(e);
        }
    }
    bundled_record::record_written(plugin, plugin_root)
}

#[cfg(test)]
#[path = "embedded_plugins_tests.rs"]
mod tests;
