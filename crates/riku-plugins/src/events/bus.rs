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

    fn manifest_toml_with_priority(name: &str, event: &str, priority: i32) -> String {
        format!(
            "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={RIKU_PLUGIN_API}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"{event}\"]\npriority={priority}\n"
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
    fn subscribers_run_in_priority_order() {
        let (tmp, paths) = make_bus_paths();
        let marker = tmp.path().join("order.log");

        let make_bundle = |name: &str, priority: i32| {
            let bundle = paths.plugin_root.join(name);
            std::fs::create_dir_all(bundle.join("bin")).unwrap();
            write_exec(
                &bundle.join("bin/on-event"),
                &format!("#!/bin/sh\necho {name} >> '{}'\n", marker.display()),
            );
            std::fs::write(
                bundle.join("riku-plugin.toml"),
                manifest_toml_with_priority(name, "deploy.finished", priority),
            )
            .unwrap();
        };

        // Installed alphabetically reversed and out of priority order, so a
        // filesystem-order or name-order pass would get this wrong too.
        make_bundle("second", 5);
        make_bundle("first", 1);

        EventBus::new(&paths).publish(EventName::DeployFinished, "app", serde_json::json!({}));

        let order = std::fs::read_to_string(&marker).unwrap();
        assert_eq!(order, "first\nsecond\n");
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

    /// End-to-end: the real `plugins/riku-notify` bundle shipped in the repo,
    /// installed into a temp plugin root exactly as `riku install-plugins`
    /// would lay it out, actually reaches a webhook when `app.restarted`
    /// fires — through the *real* sandbox (`capabilities.network = true`
    /// must actually let curl's DNS/TCP through under landlock), not just a
    /// direct invocation of the script.
    #[test]
    fn bundled_riku_notify_plugin_delivers_webhook_on_app_restarted() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let (_tmp, paths) = make_bus_paths();

        // Copy the repo's real plugins/riku-notify/ bundle into the temp
        // plugin root — the actual shipped manifest + script, not a fixture.
        let repo_bundle = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/riku-notify");
        let dest = paths.plugin_root.join("riku-notify");
        std::fs::create_dir_all(dest.join("bin")).unwrap();
        std::fs::copy(
            repo_bundle.join("riku-plugin.toml"),
            dest.join("riku-plugin.toml"),
        )
        .unwrap();
        std::fs::copy(repo_bundle.join("bin/on-event"), dest.join("bin/on-event")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dest.join("bin/on-event"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }

        // Minimal one-shot HTTP server: accept a single connection, read the
        // request, hand back 200. No new dev-dependency for this.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
            String::from_utf8_lossy(&buf[..n]).to_string()
        });

        std::env::set_var(
            "RIKU_NOTIFY_WEBHOOK_URL",
            format!("http://127.0.0.1:{port}/incident"),
        );

        EventBus::new(&paths).publish(
            EventName::AppRestarted,
            "demo-app",
            serde_json::json!({ "instance": "demo-app.web.0", "exit_code": 137, "restart_count": 2 }),
        );

        std::env::remove_var("RIKU_NOTIFY_WEBHOOK_URL");

        let request = received.join().unwrap();
        assert!(request.contains("POST /incident"), "got: {request}");
        assert!(
            request.contains("\"instance\":\"demo-app.web.0\""),
            "got: {request}"
        );
        assert!(request.contains("\"exit_code\":137"), "got: {request}");
        assert!(request.contains("\"restart_count\":2"), "got: {request}");
    }
}
