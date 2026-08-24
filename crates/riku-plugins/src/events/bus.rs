//! Event bus dispatch (Plugin Protocol v1 §7).
//!
//! [`EventBus::emit`] logs every event, then delivers it to each plugin whose
//! manifest subscribes to it: the entry executable is invoked with the verb
//! `on_event` and the event JSON on stdin.
//!
//! Slice 2 implements **observe** mode only: delivery is fire-and-forget and a
//! subscriber failure is logged, never fatal. `gate` mode (veto) needs the
//! trust model (§7.2 / `ROADMAP.md` E2.5); a gate subscriber currently runs as
//! observe and logs that the veto is not yet enforced, so it grants no false
//! sense of security.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::RikuPaths;
use crate::executor::{
    emit_plugin_output, plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout,
};
use crate::manifest::{PluginManifest, SubscribeMode};
use crate::RIKU_PLUGIN_API;

use super::{EventEnvelope, EventName};

/// Delivers lifecycle events to subscribed plugins.
pub struct EventBus<'a> {
    paths: &'a RikuPaths,
}

impl<'a> EventBus<'a> {
    /// Bind a bus to the plugin tree under `paths`.
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    /// Convenience: build an envelope and emit it.
    pub fn publish(&self, event: EventName, app: &str, data: serde_json::Value) {
        self.emit(&EventEnvelope::new(event, app, data));
    }

    /// Publish a `plugin.custom.*` event on behalf of `source_plugin`
    /// (`riku plugin-emit`: `PLUGIN_PROTOCOL.md` §7.4). Callers are
    /// responsible for having already validated `name`'s namespace and that
    /// `source_plugin` declared `events.emit = true`; this method just
    /// builds the envelope and delivers it like any other event.
    pub fn publish_custom(
        &self,
        name: &str,
        source_plugin: &str,
        app: &str,
        data: serde_json::Value,
    ) {
        self.emit(&EventEnvelope::new_custom(name, source_plugin, app, data));
    }

    /// Log the event and deliver it to every subscriber.
    pub fn emit(&self, envelope: &EventEnvelope) {
        let line = match envelope.to_json_line() {
            Ok(line) => line,
            // An envelope that cannot serialize is a bug, not a deploy failure.
            Err(e) => {
                tracing::warn!(target: "riku::events", "failed to serialize event: {e}");
                return;
            }
        };
        tracing::debug!(target: "riku::events", "{line}");

        for (bundle, manifest) in self.subscribers_for(&envelope.event) {
            if manifest.events.mode == SubscribeMode::Gate {
                tracing::warn!(
                    target: "riku::events",
                    plugin = %manifest.name,
                    "gate-mode subscription is not yet enforced; running as observe"
                );
            }
            self.run_subscriber(&bundle, &manifest, envelope, &line);
        }
    }

    /// Plugin bundles whose manifest subscribes to `event`, ordered by
    /// `events.priority` (lower first). Ties keep filesystem discovery order.
    fn subscribers_for(&self, event: &str) -> Vec<(PathBuf, PluginManifest)> {
        let mut subscribers: Vec<(PathBuf, PluginManifest)> =
            crate::bundles::find_bundles(&self.paths.plugin_root)
                .into_iter()
                .filter(|(_, manifest)| manifest.subscribes_to(event))
                .collect();
        subscribers.sort_by_key(|(_, manifest)| manifest.events.priority);
        subscribers
    }

    /// Invoke one subscriber with `on_event` and the event JSON on stdin.
    fn run_subscriber(
        &self,
        bundle: &Path,
        manifest: &PluginManifest,
        envelope: &EventEnvelope,
        json_line: &str,
    ) {
        let mut cmd = Command::new(manifest.entry_path(bundle));
        cmd.arg("on_event")
            .current_dir(bundle)
            .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
            .env("RIKU_ROOT", &self.paths.riku_root)
            .env("RIKU_PLUGIN_NAME", &manifest.name)
            .env("RIKU_APP", &envelope.app)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Own process group so a timeout can kill the whole tree.
            .process_group(0);
        if let Some(dir) = crate::plugin_data::plugin_data_path(self.paths, &manifest.name) {
            cmd.env("RIKU_PLUGIN_DATA_PATH", dir);
        }

        // Confine the subscriber to its declared capabilities before launch.
        let sandbox_paths = crate::sandbox::SandboxPaths {
            app_path: Some(self.paths.app_root.join(&envelope.app)),
            env_path: Some(self.paths.env_root.join(&envelope.app)),
            ..Default::default()
        };
        crate::sandbox::harden(&mut cmd, &manifest.capabilities, &sandbox_paths);

        let mut child = match spawn_retrying_etxtbsy(&mut cmd) {
            Ok(child) => child,
            Err(e) => {
                tracing::warn!(
                    target: "riku::events",
                    plugin = %manifest.name,
                    "failed to spawn subscriber: {e}"
                );
                return;
            }
        };

        // Deliver the event, then close stdin so the subscriber sees EOF.
        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "{json_line}");
        }

        let timed_out = wait_with_timeout(&mut child, plugin_timeout());
        emit_plugin_output(&mut child, &manifest.name);

        if timed_out {
            tracing::warn!(
                target: "riku::events",
                plugin = %manifest.name,
                event = %envelope.event,
                "subscriber timed out"
            );
            return;
        }

        match child.wait() {
            Ok(status) if status.success() => {}
            Ok(status) => tracing::warn!(
                target: "riku::events",
                plugin = %manifest.name,
                event = %envelope.event,
                "subscriber exited with {}",
                status.code().unwrap_or(-1)
            ),
            Err(e) => tracing::warn!(
                target: "riku::events",
                plugin = %manifest.name,
                "wait failed: {e}"
            ),
        }
    }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;
