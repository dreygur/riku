//! Filter dispatch (`PLUGIN_PROTOCOL.md` §7.3).
//!
//! [`FilterBus::apply`] runs a value through every plugin subscribed to a
//! filter name, in priority order, each receiving the previous one's output.
//! Verb `on_filter`, request `{"filter": name, "data": value}` on stdin,
//! response `{"data": value}` on stdout.
//!
//! **Must degrade safely, never break a caller**: a non-zero exit, timeout,
//! spawn failure, or malformed response is logged as a warning and the
//! *input* value passes through unchanged to the next filter in the chain —
//! a broken filter plugin can only turn a filter into a no-op, never a hard
//! failure. This is why filters have no `gate`-equivalent mode: a filter
//! can decline to transform, but never veto.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::config::RikuPaths;
use crate::executor::{plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout};
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
        // to the log — same convention as the addon seam's verb dispatch.
        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stdout_handle = child.stdout.take().map(|out| {
            let buf = Arc::clone(&stdout_buf);
            std::thread::spawn(move || {
                let mut s = String::new();
                if BufReader::new(out).read_to_string(&mut s).is_ok() {
                    *buf.lock().unwrap() = s;
                }
            })
        });
        let plugin_name = manifest.name.clone();
        let stderr_handle = child.stderr.take().map(|err| {
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    tracing::info!(target: "riku::filters", filter = %plugin_name, "{line}");
                }
            })
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
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_exec(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn make_bus_paths() -> (tempfile::TempDir, RikuPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = RikuPaths::for_tests(tmp.path());
        (tmp, paths)
    }

    fn write_filter_bundle(
        bundle: &Path,
        name: &str,
        filter_name: &str,
        priority: i32,
        script: &str,
    ) {
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        write_exec(&bundle.join("bin/on-filter"), script);
        std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"{name}\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-filter\"\n[filters]\nsubscribe=[\"{filter_name}\"]\npriority={priority}\n",
                crate::RIKU_PLUGIN_API
            ),
        )
        .unwrap();
    }

    #[test]
    fn no_subscribers_returns_input_unchanged() {
        let (_tmp, paths) = make_bus_paths();
        let result = FilterBus::new(&paths).apply("nginx.include_content", serde_json::json!(""));
        assert_eq!(result, serde_json::json!(""));
    }

    #[test]
    fn single_filter_transforms_the_value() {
        let (_tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("uppercaser");
        write_filter_bundle(
            &bundle,
            "uppercaser",
            "greeting",
            0,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nupper=$(printf '%s' \"$data\" | tr a-z A-Z)\nprintf '{\"data\":\"%s\"}' \"$upper\"\n",
        );

        let result = FilterBus::new(&paths).apply("greeting", serde_json::json!("hello"));
        assert_eq!(result, serde_json::json!("HELLO"));
    }

    #[test]
    fn chain_runs_in_priority_order_each_seeing_previous_output() {
        let (_tmp, paths) = make_bus_paths();

        // "second" installed first / alphabetically first, but priority 5
        // means it must run AFTER "first" (priority 1) — proves ordering
        // isn't filesystem or name order.
        write_filter_bundle(
            &paths.plugin_root.join("second"),
            "second",
            "chain",
            5,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%sB\"}' \"$data\"\n",
        );
        write_filter_bundle(
            &paths.plugin_root.join("first"),
            "first",
            "chain",
            1,
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%sA\"}' \"$data\"\n",
        );

        let result = FilterBus::new(&paths).apply("chain", serde_json::json!("x"));
        assert_eq!(result, serde_json::json!("xAB"));
    }

    #[test]
    fn broken_filter_degrades_to_passthrough_not_failure() {
        let (_tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("broken");
        write_filter_bundle(&bundle, "broken", "chain", 0, "#!/bin/sh\nexit 1\n");

        let result = FilterBus::new(&paths).apply("chain", serde_json::json!("unchanged"));
        assert_eq!(result, serde_json::json!("unchanged"));
    }

    #[test]
    fn malformed_output_degrades_to_passthrough() {
        let (_tmp, paths) = make_bus_paths();
        let bundle = paths.plugin_root.join("malformed");
        write_filter_bundle(
            &bundle,
            "malformed",
            "chain",
            0,
            "#!/bin/sh\necho 'not json'\n",
        );

        let result = FilterBus::new(&paths).apply("chain", serde_json::json!("unchanged"));
        assert_eq!(result, serde_json::json!("unchanged"));
    }
}
