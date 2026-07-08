//! `riku plugin-emit` — lets a plugin's own script fire a custom
//! `plugin.custom.*` event (`PLUGIN_PROTOCOL.md` §7.4).
//!
//! Must be run from within a plugin invocation (reads `RIKU_PLUGIN_NAME`,
//! set by every dispatch site alongside `RIKU_PLUGIN_API`/`RIKU_ROOT`), and
//! that plugin must have declared `[events] emit = true` in its own
//! manifest. The `plugin.custom.` namespace is enforced here, not just
//! documented — a plugin can never emit something that looks like a kernel
//! event (`app.restarted`, `deploy.finished`, …), which is what keeps those
//! events trustworthy for `gate`-mode subscribers.

use anyhow::{bail, Context, Result};

use crate::config::RikuPaths;
use crate::util::display;
use riku_plugins::{EventBus, PluginManifest};

const CUSTOM_NAMESPACE: &str = "plugin.custom.";

pub fn cmd_plugin_emit(
    paths: &RikuPaths,
    event_name: &str,
    data: &str,
    app: Option<&str>,
) -> Result<()> {
    let plugin_name = std::env::var("RIKU_PLUGIN_NAME").context(
        "riku plugin-emit must be run from within a plugin invocation \
         (RIKU_PLUGIN_NAME is unset — this isn't meant to be run by hand)",
    )?;

    if !event_name.starts_with(CUSTOM_NAMESPACE) {
        bail!(
            "custom event names must start with '{CUSTOM_NAMESPACE}' (got '{event_name}') — \
             a plugin can never emit something that looks like a kernel event"
        );
    }

    let manifest = PluginManifest::from_dir(&paths.plugin_root.join(&plugin_name))
        .with_context(|| format!("loading manifest for plugin '{plugin_name}'"))?;

    if !manifest.events.emit {
        bail!(
            "plugin '{plugin_name}' has not declared `[events] emit = true` in its manifest — \
             add it before calling `riku plugin-emit`"
        );
    }

    let value: serde_json::Value =
        serde_json::from_str(data).with_context(|| format!("--data is not valid JSON: {data}"))?;

    EventBus::new(paths).publish_custom(event_name, &plugin_name, app.unwrap_or(""), value);
    display::success(&format!("emitted '{event_name}' from '{plugin_name}'"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    // cmd_plugin_emit reads the process-global RIKU_PLUGIN_NAME env var —
    // serialize tests that set it so they don't race each other.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn write_exec(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn rejects_non_namespaced_event_names() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());

        std::env::set_var("RIKU_PLUGIN_NAME", "whatever");
        let err = cmd_plugin_emit(&paths, "app.restarted", "{}", None)
            .unwrap_err()
            .to_string();
        std::env::remove_var("RIKU_PLUGIN_NAME");

        assert!(err.contains("plugin.custom."), "got: {err}");
    }

    #[test]
    fn rejects_plugin_without_emit_declared() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());

        let bundle = paths.plugin_root.join("quiet");
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        write_exec(&bundle.join("bin/on-event"), "#!/bin/sh\nexit 0\n");
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"quiet\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n",
                riku_plugins::RIKU_PLUGIN_API
            ),
        )
        .unwrap();

        std::env::set_var("RIKU_PLUGIN_NAME", "quiet");
        let err = cmd_plugin_emit(&paths, "plugin.custom.thing", "{}", None)
            .unwrap_err()
            .to_string();
        std::env::remove_var("RIKU_PLUGIN_NAME");

        assert!(err.contains("emit = true"), "got: {err}");
    }

    #[test]
    fn emitter_reaches_a_real_subscriber_with_source_plugin_set() {
        let _guard = ENV_GUARD.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());

        // Plugin A: allowed to emit.
        let emitter = paths.plugin_root.join("emitter");
        std::fs::create_dir_all(emitter.join("bin")).unwrap();
        write_exec(&emitter.join("bin/on-event"), "#!/bin/sh\nexit 0\n");
        std::fs::write(
            emitter.join("riku-plugin.toml"),
            format!(
                "name=\"emitter\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n[events]\nemit=true\n",
                riku_plugins::RIKU_PLUGIN_API
            ),
        )
        .unwrap();

        // Plugin B: subscribed to the custom event, records what it received.
        let received = tmp.path().join("received.json");
        let subscriber = paths.plugin_root.join("subscriber");
        std::fs::create_dir_all(subscriber.join("bin")).unwrap();
        write_exec(
            &subscriber.join("bin/on-event"),
            &format!("#!/bin/sh\ncat > '{}'\n", received.display()),
        );
        std::fs::write(
            subscriber.join("riku-plugin.toml"),
            format!(
                "name=\"subscriber\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-event\"\n[events]\nsubscribe=[\"plugin.custom.thing\"]\n",
                riku_plugins::RIKU_PLUGIN_API
            ),
        )
        .unwrap();

        std::env::set_var("RIKU_PLUGIN_NAME", "emitter");
        let result = cmd_plugin_emit(&paths, "plugin.custom.thing", r#"{"k":"v"}"#, Some("myapp"));
        std::env::remove_var("RIKU_PLUGIN_NAME");
        result.unwrap();

        let body = std::fs::read_to_string(&received).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["event"], "plugin.custom.thing");
        assert_eq!(parsed["source_plugin"], "emitter");
        assert_eq!(parsed["app"], "myapp");
        assert_eq!(parsed["data"]["k"], "v");
    }
}
