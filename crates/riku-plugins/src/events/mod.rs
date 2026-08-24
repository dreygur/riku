//! Lifecycle event schema and bus: Plugin Protocol v1 (`PLUGIN_PROTOCOL.md` §7).
//!
//! This module defines the **schema** ([`EventName`], [`EventEnvelope`]) and
//! re-exports the [`EventBus`], which delivers events to subscribed plugins.

mod bus;

pub use bus::EventBus;

use serde::Serialize;

use super::hooks::PluginHook;
use super::RIKU_PLUGIN_API;

/// Lifecycle event names. Names are dotted (`deploy.requested`).
///
/// `PLUGIN_PROTOCOL.md` §7.1 defines the full v1 target catalog (build/deploy
/// failures, `release.activated`, the `app.*` events). This enum grows one
/// variant at a time as each event's emit site is wired, so it never carries a
/// name the kernel does not yet emit. The current set covers the four lifecycle
/// points that map to the legacy hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventName {
    DeployRequested,
    BuildStarted,
    BuildFinished,
    DeployFinished,
    AppRestarted,
    AppFailed,
}

impl EventName {
    /// The dotted wire name, e.g. `"deploy.requested"`.
    pub fn as_str(self) -> &'static str {
        match self {
            EventName::DeployRequested => "deploy.requested",
            EventName::BuildStarted => "build.started",
            EventName::BuildFinished => "build.finished",
            EventName::DeployFinished => "deploy.finished",
            EventName::AppRestarted => "app.restarted",
            EventName::AppFailed => "app.failed",
        }
    }

    /// The lifecycle event corresponding to a legacy hook stage, so the four
    /// existing hooks emit events at the exact same points (`PLUGIN_PROTOCOL.md`
    /// §7.1, "Legacy hook" column).
    pub fn from_hook(hook: &PluginHook) -> Self {
        match hook {
            PluginHook::PreDeploy => EventName::DeployRequested,
            PluginHook::PreBuild => EventName::BuildStarted,
            PluginHook::PostBuild => EventName::BuildFinished,
            PluginHook::PostDeploy => EventName::DeployFinished,
        }
    }
}

/// The serialized event passed to subscribers: the envelope from
/// `PLUGIN_PROTOCOL.md` §7.
#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope {
    /// Protocol version that produced this event.
    pub api: u32,
    /// Dotted event name. A `String`, not `&'static str`: kernel events use a
    /// fixed name from [`EventName`], but `plugin.custom.*` events (§7.4) are
    /// dynamic strings supplied at emit time.
    pub event: String,
    /// RFC 3339 / ISO 8601 UTC timestamp.
    pub ts: String,
    /// App the event concerns.
    pub app: String,
    /// Event-specific payload.
    pub data: serde_json::Value,
    /// `None` for kernel-emitted events. `Some(plugin_name)` for a
    /// `plugin.custom.*` event: always kernel-set from the emitting
    /// plugin's own manifest, never a value the plugin could supply itself,
    /// so a subscriber can trust this field to tell kernel-truth from
    /// plugin-claimed (`PLUGIN_PROTOCOL.md` §7.4).
    pub source_plugin: Option<String>,
}

impl EventEnvelope {
    /// Build a kernel-emitted envelope stamped with the current API version
    /// and UTC time. `source_plugin` is always `None`.
    pub fn new(event: EventName, app: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            api: RIKU_PLUGIN_API,
            event: event.as_str().to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            app: app.into(),
            data,
            source_plugin: None,
        }
    }

    /// Build a plugin-emitted envelope. `name` must already be validated as
    /// `plugin.custom.*` by the caller (`riku plugin-emit`: §7.4); this
    /// constructor doesn't re-check it, since it's the one place
    /// `source_plugin` is ever set to `Some`, and that's what subscribers
    /// rely on to distinguish it from a kernel event.
    pub fn new_custom(
        name: impl Into<String>,
        source_plugin: impl Into<String>,
        app: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            api: RIKU_PLUGIN_API,
            event: name.into(),
            ts: chrono::Utc::now().to_rfc3339(),
            app: app.into(),
            data,
            source_plugin: Some(source_plugin.into()),
        }
    }

    /// Serialize to the single-line JSON wire form delivered on a subscriber's
    /// stdin.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hook_maps_to_an_event() {
        assert_eq!(
            EventName::from_hook(&PluginHook::PreDeploy),
            EventName::DeployRequested
        );
        assert_eq!(
            EventName::from_hook(&PluginHook::PreBuild),
            EventName::BuildStarted
        );
        assert_eq!(
            EventName::from_hook(&PluginHook::PostBuild),
            EventName::BuildFinished
        );
        assert_eq!(
            EventName::from_hook(&PluginHook::PostDeploy),
            EventName::DeployFinished
        );
    }

    #[test]
    fn envelope_serializes_to_versioned_json_line() {
        let env = EventEnvelope::new(
            EventName::DeployFinished,
            "myapp",
            serde_json::json!({ "runtime": "node" }),
        );
        let line = env.to_json_line().unwrap();
        // One line, no embedded newlines, it is delivered as a single line.
        assert!(!line.contains('\n'));

        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["api"], RIKU_PLUGIN_API);
        assert_eq!(parsed["event"], "deploy.finished");
        assert_eq!(parsed["app"], "myapp");
        assert_eq!(parsed["data"]["runtime"], "node");
        assert!(parsed["ts"].as_str().unwrap().contains('T'));
        assert!(parsed["source_plugin"].is_null());
    }

    #[test]
    fn custom_envelope_carries_source_plugin() {
        let env = EventEnvelope::new_custom(
            "plugin.custom.something",
            "my-plugin",
            "myapp",
            serde_json::json!({ "k": "v" }),
        );
        let line = env.to_json_line().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["event"], "plugin.custom.something");
        assert_eq!(parsed["source_plugin"], "my-plugin");
        assert_eq!(parsed["app"], "myapp");
    }

    #[test]
    fn event_names_are_dotted() {
        assert_eq!(EventName::DeployRequested.as_str(), "deploy.requested");
        assert_eq!(EventName::BuildStarted.as_str(), "build.started");
        assert_eq!(EventName::AppRestarted.as_str(), "app.restarted");
        assert_eq!(EventName::AppFailed.as_str(), "app.failed");
    }
}
