//! Filter dispatch (`PLUGIN_PROTOCOL.md` §7.3).
//!
//! [`FilterBus::apply`] runs a value through every plugin subscribed to a
//! filter name, in priority order, each receiving the previous one's output.
//! Verb `on_filter`, request `{"filter": name, "data": value}` on stdin,
//! response `{"data": value}` on stdout.
//!
//! **Must degrade safely, never break a caller**: a non-zero exit, timeout,
//! spawn failure, or malformed response is logged as a warning and the
//! *input* value passes through unchanged to the next filter in the chain,
//! a broken filter plugin can only turn a filter into a no-op, never a hard
//! failure. This is why filters have no `gate`-equivalent mode: a filter
//! can decline to transform, but never veto.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::RikuPaths;
use crate::executor::{
    capture_stdout, plugin_timeout, spawn_retrying_etxtbsy, stream_stderr, wait_with_timeout,
};
use crate::manifest::PluginManifest;
use crate::RIKU_PLUGIN_API;

/// Delivers a value through the filter chain registered for a name.
pub struct FilterBus<'a> {
    paths: &'a RikuPaths,
}

impl<'a> FilterBus<'a> {
    /// Bind a bus to the plugin tree under `paths`.
    pub fn new(paths: &'a RikuPaths) -> Self {
        Self { paths }
    }

    /// Run `data` through every plugin subscribed to `filter_name`, in
    /// priority order, returning the final (possibly unchanged) value.
    pub fn apply(&self, filter_name: &str, data: serde_json::Value) -> serde_json::Value {
        let mut value = data;
        for (bundle, manifest) in self.subscribers_for(filter_name) {
            value = self.run_filter(&bundle, &manifest, filter_name, value);
        }
        value
    }

    /// Plugin bundles registered for `filter_name`, ordered by
    /// `filters.priority` (lower first). Ties keep filesystem discovery order.
    fn subscribers_for(&self, filter_name: &str) -> Vec<(PathBuf, PluginManifest)> {
        let mut subscribers: Vec<(PathBuf, PluginManifest)> =
            crate::bundles::find_bundles(&self.paths.plugin_root)
                .into_iter()
                .filter(|(_, manifest)| manifest.filters.subscribe.iter().any(|f| f == filter_name))
                .collect();
        subscribers.sort_by_key(|(_, manifest)| manifest.filters.priority);
        subscribers
    }

    /// Invoke one filter. Any failure mode returns `data` unchanged.
    fn run_filter(
        &self,
        bundle: &Path,
        manifest: &PluginManifest,
        filter_name: &str,
        data: serde_json::Value,
    ) -> serde_json::Value {
        let request = serde_json::json!({ "filter": filter_name, "data": data });
        let line = match serde_json::to_string(&request) {
            Ok(line) => line,
            Err(e) => {
                tracing::warn!(
                    target: "riku::filters",
                    plugin = %manifest.name,
                    "failed to serialize filter request: {e}"
                );
                return data;
            }
        };

        let mut cmd = Command::new(manifest.entry_path(bundle));
        cmd.arg("on_filter")
            .current_dir(bundle)
            .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
            .env("RIKU_ROOT", &self.paths.riku_root)
            .env("RIKU_PLUGIN_NAME", &manifest.name)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Own process group so a timeout can kill the whole tree.
            .process_group(0);
        if let Some(dir) = crate::plugin_data::plugin_data_path(self.paths, &manifest.name) {
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
                tracing::warn!(
                    target: "riku::filters",
                    plugin = %manifest.name,
                    filter = filter_name,
                    "failed to spawn filter: {e}"
                );
                return data;
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "{line}");
        }

        // Capture stdout on a thread (the transformed value); stream stderr
        // to the log: same convention as the addon seam's verb dispatch.
        let (stdout_handle, stdout_buf) = capture_stdout(&mut child);
        let plugin_name = manifest.name.clone();
        let stderr_handle = stream_stderr(&mut child, move |line| {
            tracing::info!(target: "riku::filters", filter = %plugin_name, "{line}");
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
                target: "riku::filters",
                plugin = %manifest.name,
                filter = filter_name,
                "filter timed out; passing input through unchanged"
            );
            return data;
        }

        let status = match child.wait() {
            Ok(status) => status,
            Err(e) => {
                tracing::warn!(target: "riku::filters", plugin = %manifest.name, "wait failed: {e}");
                return data;
            }
        };
        if !status.success() {
            tracing::warn!(
                target: "riku::filters",
                plugin = %manifest.name,
                filter = filter_name,
                "filter exited with {}; passing input through unchanged",
                status.code().unwrap_or(-1)
            );
            return data;
        }

        let captured = stdout_buf.lock().unwrap().clone();
        let trimmed = captured.trim();
        if trimmed.is_empty() {
            tracing::warn!(
                target: "riku::filters",
                plugin = %manifest.name,
                filter = filter_name,
                "filter returned no output; passing input through unchanged"
            );
            return data;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(response) => response.get("data").cloned().unwrap_or(data),
            Err(e) => {
                tracing::warn!(
                    target: "riku::filters",
                    plugin = %manifest.name,
                    filter = filter_name,
                    "filter returned invalid JSON ({e}); passing input through unchanged"
                );
                data
            }
        }
    }
}

#[cfg(test)]
#[path = "bus_tests.rs"]
mod tests;
