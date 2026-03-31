//! Plugin manager — discovers and runs lifecycle hook plugins.

use anyhow::Result;
use std::fs;
use std::process::Command;

use crate::config::RikuPaths;
use crate::plugins::hooks::{HookContext, PluginHook};

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

        tracing::info!(
            hook = ctx.hook.hook_name(),
            app = ctx.app,
            plugin = plugin_name,
            "Running plugin hook"
        );

        let status = Command::new(&plugin_path)
            .envs(&env)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to execute hook plugin '{}': {}", plugin_name, e))?;

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

        // Pre-deploy and pre-build failures abort the deploy
        match ctx.hook {
            PluginHook::PreDeploy | PluginHook::PreBuild => {
                Err(anyhow::anyhow!("{}", msg))
            }
            // Post-deploy and post-build failures are warnings, not fatal
            PluginHook::PostBuild | PluginHook::PostDeploy => {
                tracing::warn!("{}", msg);
                Ok(true)
            }
        }
    }

    /// Run all hooks that have corresponding plugin files, collecting results.
    ///
    /// Returns `Ok(n)` with the number of plugins that ran, or `Err` if any
    /// abort-on-failure hook failed.
    #[allow(dead_code)]
    pub fn run_all_hooks(&self, ctx: &HookContext<'_>) -> Result<usize> {
        let ran = self.run_hook(ctx)? as usize;
        Ok(ran)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_paths(temp: &TempDir) -> RikuPaths {
        let riku_root = temp.path().join(".riku");
        fs::create_dir_all(riku_root.join("plugins")).unwrap();
        fs::create_dir_all(riku_root.join("apps")).unwrap();
        fs::create_dir_all(riku_root.join("envs")).unwrap();
        RikuPaths::from_dirs(riku_root, &temp.path().to_path_buf())
    }

    fn make_ctx<'a>(
        app: &'a str,
        hook: &'a PluginHook,
        app_path: &'a PathBuf,
        env_path: &'a PathBuf,
        riku_root: &'a PathBuf,
        app_env: &'a HashMap<String, String>,
    ) -> HookContext<'a> {
        HookContext {
            app,
            hook,
            app_path,
            env_path,
            riku_root,
            runtime: None,
            app_env,
        }
    }

    #[test]
    fn test_run_hook_no_plugin_returns_false() {
        let temp = TempDir::new().unwrap();
        let paths = setup_paths(&temp);
        let manager = PluginManager::new(&paths);

        let app_path = PathBuf::from("/tmp/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = paths.riku_root.clone();
        let app_env = HashMap::new();
        let hook = PluginHook::PostDeploy;

        let ctx = make_ctx("myapp", &hook, &app_path, &env_path, &riku_root, &app_env);
        let result = manager.run_hook(&ctx).unwrap();
        assert!(!result, "Should return false when no plugin exists");
    }

    #[test]
    fn test_run_hook_success() {
        let temp = TempDir::new().unwrap();
        let paths = setup_paths(&temp);

        // Create a plugin that exits 0
        let plugin_path = paths.plugin_root.join("riku-post-deploy");
        fs::write(&plugin_path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let manager = PluginManager::new(&paths);
        let app_path = PathBuf::from("/tmp/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = paths.riku_root.clone();
        let app_env = HashMap::new();
        let hook = PluginHook::PostDeploy;

        let ctx = make_ctx("myapp", &hook, &app_path, &env_path, &riku_root, &app_env);
        let result = manager.run_hook(&ctx).unwrap();
        assert!(result, "Should return true when plugin runs successfully");
    }

    #[test]
    fn test_pre_deploy_hook_failure_aborts() {
        let temp = TempDir::new().unwrap();
        let paths = setup_paths(&temp);

        // Create a plugin that exits 1 (failure)
        let plugin_path = paths.plugin_root.join("riku-pre-deploy");
        fs::write(&plugin_path, "#!/bin/sh\necho 'validation failed' >&2\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let manager = PluginManager::new(&paths);
        let app_path = PathBuf::from("/tmp/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = paths.riku_root.clone();
        let app_env = HashMap::new();
        let hook = PluginHook::PreDeploy;

        let ctx = make_ctx("myapp", &hook, &app_path, &env_path, &riku_root, &app_env);
        let result = manager.run_hook(&ctx);
        assert!(result.is_err(), "pre-deploy failure should abort deploy");
        assert!(result.unwrap_err().to_string().contains("exited with code 1"));
    }

    #[test]
    fn test_post_deploy_hook_failure_is_warning_not_error() {
        let temp = TempDir::new().unwrap();
        let paths = setup_paths(&temp);

        // Create a post-deploy plugin that fails
        let plugin_path = paths.plugin_root.join("riku-post-deploy");
        fs::write(&plugin_path, "#!/bin/sh\nexit 2\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let manager = PluginManager::new(&paths);
        let app_path = PathBuf::from("/tmp/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = paths.riku_root.clone();
        let app_env = HashMap::new();
        let hook = PluginHook::PostDeploy;

        let ctx = make_ctx("myapp", &hook, &app_path, &env_path, &riku_root, &app_env);
        // Should NOT return Err — post-deploy failures are warnings
        let result = manager.run_hook(&ctx);
        assert!(result.is_ok(), "post-deploy failure should not abort: {:?}", result);
    }

    #[test]
    fn test_hook_receives_riku_env_vars() {
        let temp = TempDir::new().unwrap();
        let paths = setup_paths(&temp);

        // Create a plugin that writes RIKU_APP to a temp file for verification
        let output_file = temp.path().join("hook_output.txt");
        let plugin_content = format!(
            "#!/bin/sh\necho \"app=$RIKU_APP hook=$RIKU_HOOK\" > '{}'\n",
            output_file.display()
        );
        let plugin_path = paths.plugin_root.join("riku-post-build");
        fs::write(&plugin_path, &plugin_content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let manager = PluginManager::new(&paths);
        let app_path = PathBuf::from("/tmp/testapp");
        let env_path = PathBuf::from("/tmp/envs/testapp");
        let riku_root = paths.riku_root.clone();
        let app_env = HashMap::new();
        let hook = PluginHook::PostBuild;

        let ctx = make_ctx("testapp", &hook, &app_path, &env_path, &riku_root, &app_env);
        manager.run_hook(&ctx).unwrap();

        let output = fs::read_to_string(&output_file).unwrap();
        assert!(output.contains("app=testapp"), "RIKU_APP not passed to plugin");
        assert!(output.contains("hook=post-build"), "RIKU_HOOK not passed to plugin");
    }
}
