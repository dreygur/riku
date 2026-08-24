//! Runtime plugin discovery and dispatch.
//!
//! A runtime plugin is any executable in `~/.riku/plugins/` whose name does NOT
//! start with `riku-`. It implements four subcommands:
//!
//! | Subcommand | Purpose |
//! |------------|---------|
//! | `detect`   | Exit 0 if this plugin handles the app, exit 1 to skip. |
//! | `build`    | Install dependencies (npm install, pip install, etc.). |
//! | `env`      | Print `KEY=VALUE` lines to stdout; merged into worker env. |
//! | `start`    | Print the default start command (used when Procfile has no `web` entry). |
//!
//! All subcommands receive context via environment variables:
//! `RIKU_APP`, `RIKU_APP_PATH`, `RIKU_ENV_PATH`, `RIKU_ROOT`.
//!
//! ## Detection resolution
//!
//! 1. If `RUNTIME=<name>` is set in the app ENV, that plugin is used directly
//!    (error if not found).
//! 2. Otherwise plugins sorted alphabetically are tried in order; first
//!    `detect` exit-0 wins. If multiple match, first alphabetically wins.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::executor::{plugin_timeout, wait_with_timeout};

/// A runtime plugin discovered in the plugins directory.
#[derive(Debug, Clone)]
pub struct RuntimePlugin {
    /// Plugin name (basename of the executable, e.g. `"node"`, `"python"`).
    pub name: String,
    /// Absolute path to the plugin executable.
    pub path: PathBuf,
}

/// Context passed to every runtime plugin subcommand via environment variables.
pub struct RuntimeContext<'a> {
    pub app: &'a str,
    pub app_path: &'a Path,
    pub env_path: &'a Path,
    pub riku_root: &'a Path,
    pub app_env: &'a HashMap<String, String>,
}

impl<'a> RuntimeContext<'a> {
    fn build_env(&self) -> HashMap<String, String> {
        let mut env = self.app_env.clone();
        env.insert("RIKU_APP".into(), self.app.into());
        env.insert("RIKU_APP_PATH".into(), self.app_path.display().to_string());
        env.insert("RIKU_ENV_PATH".into(), self.env_path.display().to_string());
        env.insert("RIKU_ROOT".into(), self.riku_root.display().to_string());
        env
    }
}

/// Scan `plugin_root` for runtime plugins: executable files whose names do NOT
/// start with `riku-`. Returns them sorted alphabetically for deterministic detection.
pub fn discover(plugin_root: &Path) -> Vec<RuntimePlugin> {
    if !plugin_root.exists() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(plugin_root) else {
        return Vec::new();
    };

    let mut plugins: Vec<RuntimePlugin> = entries
        .flatten()
        .filter_map(|entry| {
            let ft = entry.file_type().ok()?;
            if !ft.is_file() {
                return None;
            }

            let name = entry.file_name();
            let name = name.to_str()?;

            // Lifecycle hooks keep the riku- prefix — skip them
            if name.starts_with("riku-") {
                return None;
            }

            // Only consider executables on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let meta = entry.metadata().ok()?;
                if meta.permissions().mode() & 0o111 == 0 {
                    return None;
                }
            }

            Some(RuntimePlugin {
                name: name.to_string(),
                path: entry.path(),
            })
        })
        .collect();

    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

/// Detect which plugin handles the app.
///
/// If `RUNTIME` is set in `app_env`, that plugin is returned directly (returns an
/// error if no plugin with that name exists). Otherwise each plugin's `detect`
/// subcommand is run in alphabetical order; the first exit-0 result wins.
/// Returns `None` when no plugin matches and `RUNTIME` is not set.
pub fn detect(
    plugins: &[RuntimePlugin],
    app_path: &Path,
    app_env: &HashMap<String, String>,
) -> Result<Option<RuntimePlugin>> {
    if let Some(runtime_name) = app_env.get("RUNTIME") {
        let plugin = plugins
            .iter()
            .find(|p| p.name == *runtime_name)
            .ok_or_else(|| {
                anyhow!(
                    "RUNTIME='{}' is set but no plugin named '{}' was found in plugins directory",
                    runtime_name,
                    runtime_name
                )
            })?;
        return Ok(Some(plugin.clone()));
    }

    for plugin in plugins {
        if plugin_accepts(plugin, app_path, app_env)? {
            return Ok(Some(plugin.clone()));
        }
    }

    Ok(None)
}

/// Run `plugin detect`; returns `true` if the plugin accepts the app (exit 0).
fn plugin_accepts(
    plugin: &RuntimePlugin,
    app_path: &Path,
    app_env: &HashMap<String, String>,
) -> Result<bool> {
    let mut child = super::executor::spawn_retrying_etxtbsy(
        Command::new(&plugin.path)
            .arg("detect")
            .env("RIKU_APP_PATH", app_path)
            .envs(app_env)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Own process group so a timeout can killpg() the whole tree.
            .process_group(0),
    )
    .map_err(|e| anyhow!("Failed to run '{} detect': {}", plugin.name, e))?;

    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
    let status = child.wait()?;

    if timed_out {
        tracing::warn!(
            plugin = plugin.name.as_str(),
            "'detect' timed out — skipping"
        );
        return Ok(false);
    }

    Ok(status.success())
}

/// Run `plugin build`, streaming stdout and stderr to the terminal in real time.
/// Aborts the deploy if the build exits non-zero or times out.
pub fn build(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>) -> Result<()> {
    // The build step (npm install, pip install, cargo build, ...) is bounded by
    // the same RLIMIT_* ceilings as workers (CPU time, open files, file size,
    // core dumps) plus the build timeout, so a malicious or buggy postinstall
    // script can't run unbounded. The memory ceiling (RLIMIT_AS) is opt-in
    // (`RIKU_MAX_MEMORY_MB`): a default virtual-address cap aborts node/v8 and
    // JVM builds, which reserve multiple GB of virtual memory at startup. See
    // `ResourceLimits::from_env`.
    build_with_limits(
        plugin,
        ctx,
        crate::util::resource_limits::ResourceLimits::from_env(),
        plugin_timeout(),
    )
}

/// [`build`] with the resource ceilings and timeout supplied by the caller
/// instead of read from the environment. Tests exercise the limit paths
/// through this: `RIKU_MAX_MEMORY_MB` and `RIKU_PLUGIN_TIMEOUT` are
/// process-global, so setting them races every other test that spawns a
/// plugin in parallel.
pub fn build_with_limits(
    plugin: &RuntimePlugin,
    ctx: &RuntimeContext<'_>,
    limits: crate::util::resource_limits::ResourceLimits,
    timeout: Duration,
) -> Result<()> {
    tracing::info!(plugin = plugin.name.as_str(), "running build");

    let mut cmd = Command::new(&plugin.path);
    cmd.arg("build")
        .envs(ctx.build_env())
        // Piped (not inherited) so `tee_output` can retain a stderr tail
        // for resource-exhaustion classification below, while still
        // mirroring both streams live to the terminal in real time.
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group so a timeout can killpg() the whole tree.
        .process_group(0);
    unsafe {
        cmd.pre_exec(move || limits.apply());
    }
    let mut child = super::executor::spawn_retrying_etxtbsy(&mut cmd)
        .map_err(|e| anyhow!("Failed to spawn '{} build': {}", plugin.name, e))?;

    let (tee_handles, stderr_tail) = super::executor::tee_output(&mut child);
    let timed_out = wait_with_timeout(&mut child, timeout);
    let status = child.wait()?;
    for h in tee_handles {
        let _ = h.join();
    }

    if timed_out {
        anyhow::bail!("Build timed out for plugin '{}'", plugin.name);
    }
    if !status.success() {
        let tail = stderr_tail.lock().unwrap().clone();
        if let Some(cause) = super::executor::classify_resource_exit(&status, &tail) {
            return Err(crate::error::DeployError::resource_exhausted(
                "build",
                &plugin.name,
                &cause,
            )
            .into());
        }
        anyhow::bail!(
            "Build failed: plugin '{}' exited with code {}",
            plugin.name,
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

/// Run `plugin pull-service <service>`, streaming output live. Used by the
/// supervisor's periodic image-watch check to refresh one compose service
/// without a full deploy.
pub fn pull_service(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>, service: &str) -> Result<()> {
    tracing::info!(
        plugin = plugin.name.as_str(),
        service,
        "running pull-service"
    );

    let mut cmd = Command::new(&plugin.path);
    cmd.arg("pull-service")
        .arg(service)
        .envs(ctx.build_env())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group so a timeout can killpg() the whole tree.
        .process_group(0);
    let mut child = super::executor::spawn_retrying_etxtbsy(&mut cmd)
        .map_err(|e| anyhow!("Failed to spawn '{} pull-service': {}", plugin.name, e))?;

    let (tee_handles, _stderr_tail) = super::executor::tee_output(&mut child);
    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
    let status = child.wait()?;
    for h in tee_handles {
        let _ = h.join();
    }

    if timed_out {
        anyhow::bail!("pull-service timed out for plugin '{}'", plugin.name);
    }
    if !status.success() {
        anyhow::bail!(
            "pull-service failed: plugin '{}' exited with code {}",
            plugin.name,
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

/// Run `plugin env` and parse stdout as `KEY=VALUE` lines.
/// Empty lines and lines beginning with `#` are ignored.
/// A non-zero exit is logged as a warning but does not abort.
pub fn get_env(
    plugin: &RuntimePlugin,
    ctx: &RuntimeContext<'_>,
) -> Result<HashMap<String, String>> {
    let mut cmd = Command::new(&plugin.path);
    cmd.arg("env").envs(ctx.build_env());
    let output = match super::executor::output_retrying_etxtbsy(&mut cmd) {
        Ok(o) => o,
        Err(e) => return Err(anyhow!("Failed to run '{} env': {}", plugin.name, e)),
    };

    if !output.status.success() {
        tracing::warn!(
            plugin = plugin.name.as_str(),
            "'env' subcommand returned non-zero — env vars may be incomplete"
        );
    }

    parse_env_lines(&output.stdout)
}

/// Run `plugin start` and return the first non-empty trimmed line, or `None`.
pub fn get_start_cmd(plugin: &RuntimePlugin, ctx: &RuntimeContext<'_>) -> Result<Option<String>> {
    let mut cmd = Command::new(&plugin.path);
    cmd.arg("start").envs(ctx.build_env());
    let output = match super::executor::output_retrying_etxtbsy(&mut cmd) {
        Ok(o) => o,
        Err(e) => return Err(anyhow!("Failed to run '{} start': {}", plugin.name, e)),
    };

    let cmd = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned);

    Ok(cmd)
}

/// Parse `KEY=VALUE` lines from raw bytes. Lines empty or starting with `#` are skipped.
fn parse_env_lines(raw: &[u8]) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for line in BufReader::new(raw).lines().map_while(Result::ok) {
        let line = line.trim().to_string();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            env.insert(k.trim().to_string(), v.to_string());
        }
    }
    Ok(env)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
