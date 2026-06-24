//! Router verb dispatch (`PLUGIN_PROTOCOL.md` §4, §6.2).
//!
//! Runs one router verb (`configure`/`reload`) as a fresh child process: the
//! verb is `argv[1]`, an optional request is a single JSON line on stdin, and
//! success is the exit code. stderr is streamed to the deploy log live; stdout
//! has no contract for this seam, so it is discarded (plugins log to stderr per
//! §4). The shared plugin timeout bounds the call so a wedged router can never
//! hang a deploy against an undrained pipe.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::config::RikuPaths;
use crate::plugins::executor::{plugin_timeout, spawn_retrying_etxtbsy, wait_with_timeout};
use crate::plugins::manifest::PluginManifest;
use crate::plugins::RIKU_PLUGIN_API;

/// Invoke a router verb. `input` is written as one JSON line on stdin when
/// present (`configure`); `reload` passes `None`. App context env is set when
/// `app` is `Some`. Returns `Ok(())` on a zero exit.
pub fn run_verb(
    paths: &RikuPaths,
    bundle: &Path,
    manifest: &PluginManifest,
    verb: &str,
    app: Option<&str>,
    input: Option<&serde_json::Value>,
) -> Result<()> {
    let mut cmd = Command::new(manifest.entry_path(bundle));
    cmd.arg(verb)
        .current_dir(bundle)
        .env("RIKU_PLUGIN_API", RIKU_PLUGIN_API.to_string())
        .env("RIKU_ROOT", &paths.riku_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0);

    if let Some(app) = app {
        cmd.env("RIKU_APP", app)
            .env("RIKU_APP_PATH", paths.app_root.join(app))
            .env("RIKU_ENV_PATH", paths.env_root.join(app));
    }

    let mut child = spawn_retrying_etxtbsy(&mut cmd)
        .with_context(|| format!("spawning router '{}'", manifest.name))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Some(value) = input {
            let line = serde_json::to_string(value)?;
            let _ = writeln!(stdin, "{line}");
        }
    }

    let plugin_name = manifest.name.clone();
    let stderr_handle = child.stderr.take().map(|err| {
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                tracing::info!(router = %plugin_name, "{line}");
            }
        })
    });

    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
    if let Some(h) = stderr_handle {
        let _ = h.join();
    }

    if timed_out {
        bail!("router '{}' verb '{}' timed out", manifest.name, verb);
    }

    let status = child.wait()?;
    if !status.success() {
        bail!(
            "router '{}' verb '{}' failed (exit {})",
            manifest.name,
            verb,
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}
