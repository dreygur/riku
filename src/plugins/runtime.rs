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
    tracing::info!(plugin = plugin.name.as_str(), "running build");

    // The build step (npm install, pip install, cargo build, ...) is the one
    // part of the deploy pipeline that ran with zero resource limits: worker
    // processes get cgroup/rlimit constraints in spawn_process, but nothing
    // bounded the build itself, so a malicious or buggy postinstall script
    // (or a crafted Cargo.toml/package.json) could exhaust host memory/CPU
    // before any worker limit ever applied. Apply the same RLIMIT_* ceiling
    // used for workers here too.
    let limits = crate::supervisor::resource_limits::ResourceLimits::from_env();

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
    let timed_out = wait_with_timeout(&mut child, plugin_timeout());
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

/// Run `plugin env` and parse stdout as `KEY=VALUE` lines.
/// Empty lines and lines beginning with `#` are ignored.
/// A non-zero exit is logged as a warning but does not abort.
pub fn get_env(
    plugin: &RuntimePlugin,
    ctx: &RuntimeContext<'_>,
) -> Result<HashMap<String, String>> {
    let output = Command::new(&plugin.path)
        .arg("env")
        .envs(ctx.build_env())
        .output()
        .map_err(|e| anyhow!("Failed to run '{} env': {}", plugin.name, e))?;

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
    let output = Command::new(&plugin.path)
        .arg("start")
        .envs(ctx.build_env())
        .output()
        .map_err(|e| anyhow!("Failed to run '{} start': {}", plugin.name, e))?;

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
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn make_plugin(dir: &Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn discover_skips_lifecycle_hooks() {
        let tmp = TempDir::new().unwrap();
        make_plugin(tmp.path(), "riku-pre-deploy", "#!/bin/sh\n");
        make_plugin(tmp.path(), "node", "#!/bin/sh\n");
        let plugins = discover(tmp.path());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "node");
    }

    #[test]
    fn discover_sorts_alphabetically() {
        let tmp = TempDir::new().unwrap();
        make_plugin(tmp.path(), "ruby", "#!/bin/sh\n");
        make_plugin(tmp.path(), "node", "#!/bin/sh\n");
        make_plugin(tmp.path(), "python", "#!/bin/sh\n");
        let plugins = discover(tmp.path());
        let names: Vec<_> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["node", "python", "ruby"]);
    }

    #[test]
    fn discover_skips_non_executable() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("notexec");
        fs::write(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let plugins = discover(tmp.path());
        assert!(plugins.is_empty());
    }

    #[test]
    fn detect_runtime_override_missing_plugin_errors() {
        let tmp = TempDir::new().unwrap();
        let plugins = discover(tmp.path());
        let mut env = HashMap::new();
        env.insert("RUNTIME".into(), "ghost".into());
        assert!(detect(&plugins, tmp.path(), &env).is_err());
    }

    #[test]
    fn detect_runtime_override_selects_named_plugin() {
        let tmp = TempDir::new().unwrap();
        make_plugin(tmp.path(), "python", "#!/bin/sh\nexit 0\n");
        make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 0\n");
        let plugins = discover(tmp.path());
        let mut env = HashMap::new();
        env.insert("RUNTIME".into(), "python".into());
        let result = detect(&plugins, tmp.path(), &env).unwrap();
        assert_eq!(result.unwrap().name, "python");
    }

    #[test]
    fn detect_first_match_alphabetically() {
        let tmp = TempDir::new().unwrap();
        // Both accept — 'node' < 'python' alphabetically
        make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 0\n");
        make_plugin(tmp.path(), "python", "#!/bin/sh\nexit 0\n");
        let plugins = discover(tmp.path());
        let result = detect(&plugins, tmp.path(), &HashMap::new())
            .unwrap()
            .unwrap();
        assert_eq!(result.name, "node");
    }

    #[test]
    fn detect_returns_none_when_no_match() {
        let tmp = TempDir::new().unwrap();
        make_plugin(tmp.path(), "node", "#!/bin/sh\nexit 1\n");
        let plugins = discover(tmp.path());
        let result = detect(&plugins, tmp.path(), &HashMap::new()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_env_parses_key_value_lines() {
        let tmp = TempDir::new().unwrap();
        make_plugin(
            tmp.path(),
            "testplugin",
            "#!/bin/sh\necho 'FOO=bar'\necho '# comment'\necho ''\necho 'BAZ=qux'\n",
        );
        let plugins = discover(tmp.path());
        let ctx = RuntimeContext {
            app: "myapp",
            app_path: tmp.path(),
            env_path: tmp.path(),
            riku_root: tmp.path(),
            app_env: &HashMap::new(),
        };
        let env = get_env(&plugins[0], &ctx).unwrap();
        assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(env.get("BAZ").map(String::as_str), Some("qux"));
        assert_eq!(env.len(), 2);
    }

    #[test]
    fn get_start_cmd_returns_first_nonempty_line() {
        let tmp = TempDir::new().unwrap();
        make_plugin(
            tmp.path(),
            "testplugin",
            "#!/bin/sh\necho ''\necho 'node server.js'\n",
        );
        let plugins = discover(tmp.path());
        let ctx = RuntimeContext {
            app: "myapp",
            app_path: tmp.path(),
            env_path: tmp.path(),
            riku_root: tmp.path(),
            app_env: &HashMap::new(),
        };
        let cmd = get_start_cmd(&plugins[0], &ctx).unwrap();
        assert_eq!(cmd.as_deref(), Some("node server.js"));
    }

    #[test]
    fn parse_env_lines_handles_values_with_equals() {
        let raw = b"URL=http://example.com?foo=bar\nKEY=val\n";
        let env = parse_env_lines(raw).unwrap();
        assert_eq!(
            env.get("URL").map(String::as_str),
            Some("http://example.com?foo=bar")
        );
        assert_eq!(env.get("KEY").map(String::as_str), Some("val"));
    }
}
