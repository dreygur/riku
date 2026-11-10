//! `riku-plugin.toml` manifest parsing and validation (Plugin Protocol v1 §3).
//!
//! A plugin bundle is a directory containing a `riku-plugin.toml` manifest and
//! one or more executables. This module is the **repository** layer for that
//! manifest: it reads and validates the declaration, with no knowledge of how
//! the kernel dispatches to the plugin.
//!
//! Security: the manifest is attacker-influenced data. [`PluginManifest::validate`]
//! rejects an unsupported API version, an empty/unsafe `name`, and, critically,
//! any `entry` that is absolute or escapes the bundle via `..`, so a manifest
//! can never point the kernel at an executable outside its own directory.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::router_section::RouterSection;
use crate::RIKU_PLUGIN_API;

/// Plugin category (`PLUGIN_PROTOCOL.md` §3). `runtime`/`addon`/`router` bind to
/// a behavior seam; `notifier`/`hook` are event-subscriber categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Runtime,
    Addon,
    Router,
    Notifier,
    Hook,
}

/// Declared capabilities: shown on install, enforced where the platform allows.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub writes: Vec<String>,
    #[serde(default)]
    pub privileged: bool,
}

/// Event-subscription block; present iff the plugin subscribes to events.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventSubscription {
    #[serde(default)]
    pub subscribe: Vec<String>,
    /// Only `"observe"` is accepted, and it is the default when omitted:
    /// delivery is fire-and-forget and a subscriber failure is logged, never
    /// fatal. Kept as a field so that a manifest written for the removed
    /// `"gate"` mode is refused by [`PluginManifest::validate`] with a hint
    /// instead of installing as something else.
    #[serde(default)]
    pub mode: Option<String>,
    /// Delivery order among subscribers of the same event, lower runs
    /// first. Default `0`. Ties keep filesystem discovery order (unspecified).
    #[serde(default)]
    pub priority: i32,
    /// Opt-in: this plugin may emit its own `plugin.custom.*` events via
    /// `riku plugin-emit`. Default `false`: a plugin that only subscribes
    /// need not (and should not) declare this.
    #[serde(default)]
    pub emit: bool,
}

/// Install/uninstall lifecycle opt-in; present iff the plugin implements one
/// of these verbs. Absent (the default) means neither is invoked, every
/// existing plugin (no `[lifecycle]` block) is unaffected.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Lifecycle {
    /// Plugin implements `on_install`, invoked once after its files are
    /// copied into the plugin root (best-effort: a failure is logged, never
    /// blocks the install, since the files are already in place by then).
    #[serde(default)]
    pub install: bool,
    /// Plugin implements `on_uninstall`, invoked before its files are
    /// removed (best-effort: a failure is logged, never blocks removal).
    #[serde(default)]
    pub uninstall: bool,
}

/// A parsed, validated `riku-plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    pub api: u32,
    pub entry: String,
    #[serde(default)]
    pub checksum: Option<String>,
    /// Optional hex Ed25519 signature over the entry bytes (`PLUGIN_PROTOCOL.md`
    /// §10). Verified against the operator's trust keyring on install.
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub events: EventSubscription,
    /// Present on router plugins. `None` means the plugin predates router protocol 2.
    #[serde(default)]
    pub router: Option<RouterSection>,
}

impl PluginManifest {
    /// Parse and validate the `riku-plugin.toml` in `dir`.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join("riku-plugin.toml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        Self::from_toml_str(&text).with_context(|| format!("invalid manifest {}", path.display()))
    }

    /// Parse and validate manifest text.
    pub fn from_toml_str(text: &str) -> Result<Self> {
        let manifest: PluginManifest = toml::from_str(text)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Reject unsupported API versions, unsafe names, traversal in `entry`,
    /// and any event mode other than `observe`.
    pub fn validate(&self) -> Result<()> {
        if self.api != RIKU_PLUGIN_API {
            bail!(
                "plugin '{}' targets API {} but this kernel implements {}",
                self.name,
                self.api,
                RIKU_PLUGIN_API
            );
        }

        if self.name.is_empty()
            || !self
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!(
                "plugin name '{}' is empty or has unsafe characters",
                self.name
            );
        }

        let entry = Path::new(&self.entry);
        if entry.as_os_str().is_empty()
            || entry.is_absolute()
            || entry
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
        {
            bail!(
                "entry '{}' must be a relative path inside the bundle",
                self.entry
            );
        }

        match self.events.mode.as_deref() {
            None | Some("observe") => {}
            Some("gate") => bail!(
                "plugin '{}' sets events.mode = \"gate\", which is no longer supported; \
                 event subscribers only observe. To block a deploy, install a \
                 riku-pre-deploy hook that exits non-zero",
                self.name
            ),
            Some(other) => bail!(
                "plugin '{}' sets events.mode = \"{other}\"; the only supported mode is \"observe\"",
                self.name
            ),
        }

        Ok(())
    }

    /// Resolve the entry executable inside its bundle directory.
    pub fn entry_path(&self, bundle_dir: &Path) -> PathBuf {
        bundle_dir.join(&self.entry)
    }

    /// Whether this plugin subscribes to the given dotted event name.
    pub fn subscribes_to(&self, event: &str) -> bool {
        self.events.subscribe.iter().any(|e| e == event)
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
