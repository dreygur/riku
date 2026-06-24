//! Addon verb dispatch (`PLUGIN_PROTOCOL.md` §4, §6.1).
//!
//! Runs one addon verb (`provision`/`bind`/…) as a child process: the verb is
//! `argv[1]`, the request is a JSON line on stdin, and the response is parsed
//! from stdout (stderr is streamed to the deploy log). Output is captured on a
//! reader thread so a large or slow plugin cannot deadlock against an undrained
//! pipe, and the shared timeout still bounds the whole call.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};

use crate::config::RikuPaths;
use crate::plugins::executor::{plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout};
use crate::plugins::manifest::PluginManifest;
use crate::plugins::RIKU_PLUGIN_API;

/// Everything an addon verb invocation needs.
pub struct VerbCall<'a> {
    pub paths: &'a RikuPaths,
    pub bundle: &'a Path,
    pub manifest: &'a PluginManifest,
    pub verb: &'a str,
    pub instance: &'a str,
    pub data_path: &'a Path,
    /// Bound app context, for `bind`/`unbind`.
    pub app: Option<&'a str>,
    pub input: serde_json::Value,
}

/// Invoke an addon verb and return its parsed JSON response (an empty object
/// when the plugin prints nothing).
pub fn run_verb(call: VerbCall<'_>) -> Result<serde_json::Value> {
    let mut cmd = Command::new(call.manifest.entry_path(call.bundle));
    cmd.arg(call.verb)
        .current_dir(call.bundle)
        .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
        .env("RIKU_ROOT", &call.paths.riku_root)
        .env("RIKU_ADDON_INSTANCE", call.instance)
        .env("RIKU_ADDON_DATA_PATH", call.data_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    if let Some(app) = call.app {
        cmd.env("RIKU_APP", app)
            .env("RIKU_APP_PATH", call.paths.app_root.join(app))
            .env("RIKU_ENV_PATH", call.paths.env_root.join(app));
    }

    let mut child = spawn_retrying_etxtbsy(&mut cmd)
        .with_context(|| format!("spawning addon '{}'", call.manifest.name))?;

    if let Some(mut stdin) = child.stdin.take() {
        let line = serde_json::to_string(&call.input)?;
        let _ = writeln!(stdin, "{line}");
    }

    // Capture stdout on a thread (response body); stream stderr to the log.
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
    let plugin_name = call.manifest.name.clone();
    let stderr_handle = child.stderr.take().map(|err| {
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                tracing::info!(addon = %plugin_name, "{line}");
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
        bail!(
            "addon '{}' verb '{}' timed out",
            call.manifest.name,
            call.verb
        );
    }

    let status = child.wait()?;
    if !status.success() {
        bail!(
            "addon '{}' verb '{}' failed (exit {})",
            call.manifest.name,
            call.verb,
            status.code().unwrap_or(-1)
        );
    }

    let captured = stdout_buf.lock().unwrap().clone();
    let trimmed = captured.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(trimmed).with_context(|| {
        format!(
            "addon '{}' verb '{}' returned invalid JSON",
            call.manifest.name, call.verb
        )
    })
}
