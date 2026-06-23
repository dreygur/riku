//! Event bus dispatch (Plugin Protocol v1 §7).
//!
//! [`EventBus::emit`] logs every event, then delivers it to each plugin whose
//! manifest subscribes to it: the entry executable is invoked with the verb
//! `on_event` and the event JSON on stdin.
//!
//! Slice 2 implements **observe** mode only — delivery is fire-and-forget and a
//! subscriber failure is logged, never fatal. `gate` mode (veto) needs the
//! trust model (§7.2 / `ROADMAP.md` E2.5); a gate subscriber currently runs as
//! observe and logs that the veto is not yet enforced, so it grants no false
//! sense of security.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::RikuPaths;
use crate::plugins::executor::{
    emit_plugin_output, plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout,
};
use crate::plugins::manifest::{PluginManifest, SubscribeMode};
use crate::plugins::RIKU_PLUGIN_API;

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

        for (bundle, manifest) in self.subscribers_for(envelope.event) {
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

    /// Find plugin bundles whose manifest subscribes to `event`. Invalid
    /// manifests are skipped with a warning — one bad bundle never blocks a
    /// deploy.
    fn subscribers_for(&self, event: &str) -> Vec<(PathBuf, PluginManifest)> {
        let mut out = Vec::new();
        let read_dir = match std::fs::read_dir(&self.paths.plugin_root) {
            Ok(rd) => rd,
            Err(_) => return out,
        };
        for entry in read_dir.flatten() {
            let bundle = entry.path();
            if !bundle.is_dir() || !bundle.join("riku-plugin.toml").exists() {
                continue;
            }
            match PluginManifest::from_dir(&bundle) {
                Ok(manifest) if manifest.subscribes_to(event) => out.push((bundle, manifest)),
                Ok(_) => {}
                Err(e) => tracing::warn!(
                    target: "riku::events",
                    "ignoring invalid plugin bundle {}: {e}",
                    bundle.display()
                ),
            }
        }
        out
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
            .env("RIKU_APP", &envelope.app)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Own process group so a timeout can kill the whole tree.
            .process_group(0);

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
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_exec(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn manifest_toml(name: &str, event: &str) -> String {
        format!(
            "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={RIKU_PLUGIN_API}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"{event}\"]\n"
        )
    }

    fn make_bus_paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        (tmp, paths)
    }

    #[test]
    fn observe_subscriber_receives_event_on_stdin() {
        let (tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("recorder");
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        let received = tmp.path().join("received.json");
        write_exec(
            &bundle.join("bin/on-event"),
            &format!("#!/bin/sh\ncat > '{}'\n", received.display()),
        );
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            manifest_toml("recorder", "deploy.finished"),
        )
        .unwrap();

        EventBus::new(&paths).publish(
            EventName::DeployFinished,
            "myapp",
            serde_json::json!({ "k": "v" }),
        );

        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&received).unwrap()).unwrap();
        assert_eq!(parsed["event"], "deploy.finished");
        assert_eq!(parsed["app"], "myapp");
        assert_eq!(parsed["data"]["k"], "v");
        assert_eq!(parsed["api"], RIKU_PLUGIN_API);
    }

    #[test]
    fn subscriber_to_other_event_is_not_invoked() {
        let (tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("other");
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        let marker = tmp.path().join("ran");
        write_exec(
            &bundle.join("bin/on-event"),
            &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        );
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            manifest_toml("other", "build.started"),
        )
        .unwrap();

        EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
        assert!(
            !marker.exists(),
            "a subscriber to a different event must not run"
        );
    }

    #[test]
    fn emit_is_safe_with_no_plugins_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::from_dirs(tmp.path().join(".riku"), tmp.path());
        // plugin_root does not exist — must not panic.
        EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
    }

    #[test]
    fn invalid_bundle_is_skipped_without_panicking() {
        let (_tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("broken");
        std::fs::create_dir_all(&bundle).unwrap();
        // api 999 is unsupported → manifest invalid → bundle skipped.
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            "name=\"broken\"\nversion=\"1\"\ntype=\"notifier\"\napi=999\nentry=\"x\"\n",
        )
        .unwrap();
        EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));
    }
}
