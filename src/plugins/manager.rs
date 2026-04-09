//! Plugin manager — discovers and runs lifecycle hook plugins.

use anyhow::Result;
use std::fs;
use std::process::{Command, Stdio};

use crate::config::RikuPaths;
use crate::plugins::hooks::{HookContext, PluginHook};

use super::executor::{emit_plugin_output, plugin_timeout, wait_with_timeout};

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;

/// Orchestrates plugin discovery and hook execution.
///
/// For each hook, the manager looks for an executable file named after the
/// hook (e.g. `riku-pre-deploy`) in the configured plugins directory.
/// The file is executed with the hook's environment variables injected.
///
/// # Failure policy
///
/// - `PreDeploy` hook failure **aborts** the deploy (returns `Err`).
/// - `PreBuild` hook failure **aborts** the deploy (returns `Err`).
/// - `PostBuild` and `PostDeploy` hook failures are **logged as warnings**
///   and do not abort the deploy, because the code is already running.
pub struct PluginManager<'a> {
    paths: &'a RikuPaths,
}

impl<'a> PluginManager<'a> {
    /// Create a new `PluginManager` bound to the given paths.
    pub fn new(paths: &'a RikuPaths) -> Self {
        PluginManager { paths }
    }

    /// Run the hook plugin for `ctx.hook` if one exists.
    ///
    /// Returns `Ok(true)` if a plugin was found and ran successfully,
    /// `Ok(false)` if no plugin exists for this hook,
    /// or `Err` if the plugin failed and the hook is abort-on-failure.
    pub fn run_hook(&self, ctx: &HookContext<'_>) -> Result<bool> {
        let plugin_name = ctx.hook.plugin_name();

        // Validate name before path construction
        if crate::plugins::discovery::validate_plugin_name(plugin_name).is_err() {
            return Ok(false);
        }

        let plugin_path = self.paths.plugin_root.join(plugin_name);

        if !plugin_path.exists() {
            tracing::debug!(
                hook = ctx.hook.hook_name(),
                plugin = plugin_name,
                "No plugin found for hook — skipping"
            );
            return Ok(false);
        }

        // Ensure the plugin is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&plugin_path)?;
            if meta.permissions().mode() & 0o111 == 0 {
                fs::set_permissions(
                    &plugin_path,
                    fs::Permissions::from_mode(meta.permissions().mode() | 0o111),
                )?;
            }
        }

        let env = ctx.build_env();
        let timeout = plugin_timeout();

        tracing::info!(
            hook = ctx.hook.hook_name(),
            app = ctx.app,
            plugin = plugin_name,
            "Running plugin hook"
        );

        let mut child = Command::new(&plugin_path)
            .envs(&env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn hook plugin '{}': {}", plugin_name, e))?;

        let timed_out = wait_with_timeout(&mut child, timeout);

        // Stream captured output to tracing regardless of exit code
        emit_plugin_output(&mut child, plugin_name);

        if timed_out {
            let msg = format!(
                "Hook plugin '{}' for app '{}' timed out after {:?}",
                plugin_name, ctx.app, timeout
            );
            return match ctx.hook {
                PluginHook::PreDeploy | PluginHook::PreBuild => Err(anyhow::anyhow!("{}", msg)),
                PluginHook::PostBuild | PluginHook::PostDeploy => {
                    tracing::warn!("{}", msg);
                    Ok(true)
                }
            };
        }

        let status = child.wait()?;

        if status.success() {
            tracing::info!(
                hook = ctx.hook.hook_name(),
                app = ctx.app,
                "Hook plugin completed successfully"
            );
            return Ok(true);
        }

        let code = status.code().unwrap_or(-1);
        let msg = format!(
            "Hook plugin '{}' for app '{}' exited with code {}",
            plugin_name, ctx.app, code
        );

        match ctx.hook {
            PluginHook::PreDeploy | PluginHook::PreBuild | PluginHook::PostBuild => {
                Err(anyhow::anyhow!("{}", msg))
            }
            PluginHook::PostDeploy => {
                tracing::warn!("{}", msg);
                Ok(true)
            }
        }
    }
}

