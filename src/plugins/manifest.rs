//! `riku-plugin.toml` manifest parsing and validation (Plugin Protocol v1 §3).
//!
//! A plugin bundle is a directory containing a `riku-plugin.toml` manifest and
//! one or more executables. This module is the **repository** layer for that
//! manifest: it reads and validates the declaration, with no knowledge of how
//! the kernel dispatches to the plugin.
//!
//! Security: the manifest is attacker-influenced data. [`PluginManifest::validate`]
//! rejects an unsupported API version, an empty/unsafe `name`, and — critically
//! — any `entry` that is absolute or escapes the bundle via `..`, so a manifest
//! can never point the kernel at an executable outside its own directory.

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::RIKU_PLUGIN_API;

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

/// How an event subscriber participates (`PLUGIN_PROTOCOL.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscribeMode {
    /// Fire-and-forget; failures are logged, never fatal. Open to any plugin.
    #[default]
    Observe,
    /// May veto a gateable event. Requires elevated trust (not yet enforced).
    Gate,
}

/// Declared capabilities — shown on install, enforced where the platform allows.
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
    #[serde(default)]
    pub mode: SubscribeMode,
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
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub events: EventSubscription,
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

    /// Reject unsupported API versions, unsafe names, and traversal in `entry`.
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
mod tests {
    use super::*;

    fn valid_toml() -> String {
        format!(
            r#"
            name = "slack-notify"
            version = "0.1.0"
            type = "notifier"
            api = {RIKU_PLUGIN_API}
            entry = "bin/notify"
            [events]
            subscribe = ["deploy.finished", "deploy.failed"]
            mode = "observe"
            "#
        )
    }

    #[test]
    fn parses_a_valid_manifest() {
        let m = PluginManifest::from_toml_str(&valid_toml()).unwrap();
        assert_eq!(m.name, "slack-notify");
        assert_eq!(m.plugin_type, PluginType::Notifier);
        assert_eq!(m.events.mode, SubscribeMode::Observe);
        assert!(m.subscribes_to("deploy.finished"));
        assert!(!m.subscribes_to("build.started"));
    }

    #[test]
    fn defaults_mode_to_observe_when_omitted() {
        let toml = format!(
            "name=\"n\"\nversion=\"1\"\ntype=\"hook\"\napi={RIKU_PLUGIN_API}\nentry=\"x\"\n[events]\nsubscribe=[\"deploy.finished\"]\n"
        );
        let m = PluginManifest::from_toml_str(&toml).unwrap();
        assert_eq!(m.events.mode, SubscribeMode::Observe);
    }

    #[test]
    fn rejects_unsupported_api() {
        let toml = "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi=999\nentry=\"x\"\n";
        assert!(PluginManifest::from_toml_str(toml).is_err());
    }

    #[test]
    fn rejects_entry_path_traversal() {
        let toml = format!(
            "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"../../etc/evil\"\n"
        );
        let err = PluginManifest::from_toml_str(&toml)
            .unwrap_err()
            .to_string();
        assert!(err.contains("relative path"), "got: {err}");
    }

    #[test]
    fn rejects_absolute_entry() {
        let toml = format!(
            "name=\"n\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"/usr/bin/x\"\n"
        );
        assert!(PluginManifest::from_toml_str(&toml).is_err());
    }

    #[test]
    fn rejects_unsafe_name() {
        let toml = format!(
            "name=\"../evil\"\nversion=\"1\"\ntype=\"addon\"\napi={RIKU_PLUGIN_API}\nentry=\"x\"\n"
        );
        assert!(PluginManifest::from_toml_str(&toml).is_err());
    }

    #[test]
    fn entry_path_joins_under_bundle() {
        let m = PluginManifest::from_toml_str(&valid_toml()).unwrap();
        assert_eq!(
            m.entry_path(Path::new("/plugins/slack")),
            Path::new("/plugins/slack/bin/notify")
        );
    }
}
