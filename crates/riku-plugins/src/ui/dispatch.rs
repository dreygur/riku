//! UI panel verb dispatch (`PLUGIN_PROTOCOL.md` §7.5).
//!
//! Runs a plugin's `ui_panel` verb: no meaningful stdin (`{}`), response is
//! **structured JSON only** — `{"sections": [{"title", "fields": [{"label",
//! "value"}]}]}` — never HTML/JS, closing off injection risk into the
//! dashboard. Must degrade safely: any failure (non-zero exit, timeout,
//! spawn failure, malformed JSON) returns an empty panel and logs a
//! warning — a broken UI plugin can never break the dashboard.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::config::RikuPaths;
use crate::executor::{
    capture_stdout, plugin_timeout, spawn_retrying_etxtbsy, stream_stderr, wait_with_timeout,
};
use crate::manifest::PluginManifest;
use crate::RIKU_PLUGIN_API;

/// One labeled value in a panel section. Plain text only, by construction —
/// this struct has no field that could carry markup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelField {
    pub label: String,
    pub value: String,
}

/// One titled group of fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PanelSection {
    pub title: String,
    #[serde(default)]
    pub fields: Vec<PanelField>,
}

/// The whole panel a plugin's `ui_panel` verb returns. `Default` (empty
/// sections) is exactly what a failed/broken dispatch degrades to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UiPanelResponse {
    #[serde(default)]
    pub sections: Vec<PanelSection>,
}

/// Dispatch `ui_panel` for `manifest`/`bundle`. Always returns a panel — an
/// empty one, with the failure logged, if anything goes wrong.
pub fn run_ui_panel(
    paths: &RikuPaths,
    bundle: &Path,
    manifest: &PluginManifest,
) -> UiPanelResponse {
    let mut cmd = Command::new(manifest.entry_path(bundle));
    cmd.arg("ui_panel")
        .current_dir(bundle)
        .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
        .env("RIKU_ROOT", &paths.riku_root)
        .env("RIKU_PLUGIN_NAME", &manifest.name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group so a timeout can kill the whole tree.
        .process_group(0);
    if let Some(dir) = crate::plugin_data::plugin_data_path(paths, &manifest.name) {
        cmd.env("RIKU_PLUGIN_DATA_PATH", dir);
    }
    crate::sandbox::harden(
        &mut cmd,
        &manifest.capabilities,
        &crate::sandbox::SandboxPaths::default(),
    );

    let mut child = match spawn_retrying_etxtbsy(&mut cmd) {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(target: "riku::ui", plugin = %manifest.name, "failed to spawn ui_panel: {e}");
            return UiPanelResponse::default();
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "{{}}");
    }

    // Capture stdout on a thread (the panel body); stream stderr to the log
    // — same convention as the addon/filter seams' verb dispatch.
    let (stdout_handle, stdout_buf) = capture_stdout(&mut child);
    let plugin_name = manifest.name.clone();
    let stderr_handle = stream_stderr(&mut child, move |line| {
        tracing::info!(target: "riku::ui", plugin = %plugin_name, "{line}");
    });

    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
    if let Some(h) = stdout_handle {
        let _ = h.join();
    }
    if let Some(h) = stderr_handle {
        let _ = h.join();
    }

    if timed_out {
        tracing::warn!(
            target: "riku::ui",
            plugin = %manifest.name,
            "ui_panel timed out; returning empty panel"
        );
        return UiPanelResponse::default();
    }

    let status = match child.wait() {
        Ok(status) => status,
        Err(e) => {
            tracing::warn!(target: "riku::ui", plugin = %manifest.name, "wait failed: {e}");
            return UiPanelResponse::default();
        }
    };
    if !status.success() {
        tracing::warn!(
            target: "riku::ui",
            plugin = %manifest.name,
            "ui_panel exited with {}; returning empty panel",
            status.code().unwrap_or(-1)
        );
        return UiPanelResponse::default();
    }

    let captured = stdout_buf.lock().unwrap().clone();
    let trimmed = captured.trim();
    if trimmed.is_empty() {
        return UiPanelResponse::default();
    }

    match serde_json::from_str::<UiPanelResponse>(trimmed) {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(
                target: "riku::ui",
                plugin = %manifest.name,
                "ui_panel returned invalid JSON ({e}); returning empty panel"
            );
            UiPanelResponse::default()
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

    fn write_ui_bundle(bundle: &Path, name: &str, script: &str) {
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        write_exec(&bundle.join("bin/ui-panel"), script);
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/ui-panel\"\n[ui]\nnav_label=\"Demo Panel\"\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
    }

    #[test]
    fn dispatches_and_parses_a_real_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());
        let bundle = paths.plugin_root.join("demo-ui");
        write_ui_bundle(
            &bundle,
            "demo-ui",
            "#!/bin/sh\nprintf '{\"sections\":[{\"title\":\"Status\",\"fields\":[{\"label\":\"Queue depth\",\"value\":\"12\"}]}]}'\n",
        );
        let manifest = PluginManifest::from_dir(&bundle).unwrap();

        let panel = run_ui_panel(&paths, &bundle, &manifest);
        assert_eq!(panel.sections.len(), 1);
        assert_eq!(panel.sections[0].title, "Status");
        assert_eq!(panel.sections[0].fields[0].label, "Queue depth");
        assert_eq!(panel.sections[0].fields[0].value, "12");
    }

    #[test]
    fn broken_plugin_degrades_to_empty_panel_not_a_crash() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());
        let bundle = paths.plugin_root.join("broken-ui");
        write_ui_bundle(&bundle, "broken-ui", "#!/bin/sh\nexit 1\n");
        let manifest = PluginManifest::from_dir(&bundle).unwrap();

        let panel = run_ui_panel(&paths, &bundle, &manifest);
        assert_eq!(panel, UiPanelResponse::default());
        assert!(panel.sections.is_empty());
    }

    #[test]
    fn malformed_output_degrades_to_empty_panel() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());
        let bundle = paths.plugin_root.join("malformed-ui");
        write_ui_bundle(&bundle, "malformed-ui", "#!/bin/sh\necho 'not json'\n");
        let manifest = PluginManifest::from_dir(&bundle).unwrap();

        let panel = run_ui_panel(&paths, &bundle, &manifest);
        assert_eq!(panel, UiPanelResponse::default());
    }
}
