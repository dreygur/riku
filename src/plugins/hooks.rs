//! Plugin lifecycle hook definitions.
//!
//! Hooks fire at defined points in the deploy pipeline. Each hook maps to a
//! conventional plugin name (e.g. `PluginHook::PreDeploy` → `riku-pre-deploy`).
//!
//! ## Hook Execution Order
//!
//! ```text
//! git push received
//!   → code checked out
//!   → env vars loaded
//!   → [PRE_DEPLOY hook]     ← customise env, validate, abort deploy
//!   → runtime detected
//!   → [PRE_BUILD hook]      ← install extra deps, patch sources
//!   → runtime build step
//!   → [POST_BUILD hook]     ← run tests, asset compilation
//!   → worker configs written
//!   → [POST_DEPLOY hook]    ← notify Slack, warm caches, run migrations
//! ```
//!
//! ## Writing a Hook Plugin
//!
//! Drop an executable into `~/.riku/plugins/` named after the hook:
//!
//! ```sh
//! #!/bin/bash
//! # ~/.riku/plugins/riku-post-deploy
//! echo "Deployed $RIKU_APP at $(date)"
//! curl -s "$SLACK_WEBHOOK" -d "{\"text\": \"$RIKU_APP deployed\"}"
//! ```
//!
//! ## Environment Variables Available to Hook Plugins
//!
//! | Variable        | Description                          |
//! |-----------------|--------------------------------------|
//! | `RIKU_APP`      | Application name                     |
//! | `RIKU_HOOK`     | Hook name (e.g. `pre-deploy`)        |
//! | `RIKU_APP_PATH` | Path to the checked-out source code  |
//! | `RIKU_ENV_PATH` | Path to the app's env directory      |
//! | `RIKU_ROOT`     | Riku root directory (`~/.riku`)      |
//! | `RIKU_RUNTIME`  | Detected runtime (e.g. `Python`)     |
//! | All app env vars from the ENV file                          |

use std::collections::HashMap;
use std::path::Path;

/// Lifecycle hooks fired at defined points in the deploy pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginHook {
    /// Fired after env vars are loaded, before runtime detection and build.
    /// Plugins can abort the deploy by exiting non-zero.
    PreDeploy,
    /// Fired after runtime detection, before the build step (e.g. `pip install`).
    PreBuild,
    /// Fired after the build step, before worker configs are written.
    PostBuild,
    /// Fired after worker configs are written and the supervisor has been signalled.
    PostDeploy,
}

impl PluginHook {
    /// The conventional plugin name for this hook (e.g. `riku-pre-deploy`).
    pub fn plugin_name(&self) -> &'static str {
        match self {
            PluginHook::PreDeploy => "riku-pre-deploy",
            PluginHook::PreBuild => "riku-pre-build",
            PluginHook::PostBuild => "riku-post-build",
            PluginHook::PostDeploy => "riku-post-deploy",
        }
    }

    /// Short name used in the `RIKU_HOOK` environment variable.
    pub fn hook_name(&self) -> &'static str {
        match self {
            PluginHook::PreDeploy => "pre-deploy",
            PluginHook::PreBuild => "pre-build",
            PluginHook::PostBuild => "post-build",
            PluginHook::PostDeploy => "post-deploy",
        }
    }
}

impl std::fmt::Display for PluginHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hook_name())
    }
}

/// Context passed to a hook plugin via environment variables.
pub struct HookContext<'a> {
    pub app: &'a str,
    pub hook: &'a PluginHook,
    pub app_path: &'a Path,
    pub env_path: &'a Path,
    pub riku_root: &'a Path,
    /// Detected runtime name, if known at hook invocation time.
    pub runtime: Option<&'a str>,
    /// App env vars loaded from the ENV file.
    pub app_env: &'a HashMap<String, String>,
}

impl<'a> HookContext<'a> {
    /// Build the full environment map to pass to the hook process.
    ///
    /// Starts with the app's env vars, then overlays Riku-specific variables
    /// so that hook plugins can introspect the deploy context.
    pub fn build_env(&self) -> HashMap<String, String> {
        let mut env = self.app_env.clone();
        env.insert("RIKU_APP".to_string(), self.app.to_string());
        env.insert("RIKU_HOOK".to_string(), self.hook.hook_name().to_string());
        env.insert(
            "RIKU_APP_PATH".to_string(),
            self.app_path.to_string_lossy().to_string(),
        );
        env.insert(
            "RIKU_ENV_PATH".to_string(),
            self.env_path.to_string_lossy().to_string(),
        );
        env.insert(
            "RIKU_ROOT".to_string(),
            self.riku_root.to_string_lossy().to_string(),
        );
        if let Some(rt) = self.runtime {
            env.insert("RIKU_RUNTIME".to_string(), rt.to_string());
        }
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_hook_plugin_names() {
        assert_eq!(PluginHook::PreDeploy.plugin_name(), "riku-pre-deploy");
        assert_eq!(PluginHook::PreBuild.plugin_name(), "riku-pre-build");
        assert_eq!(PluginHook::PostBuild.plugin_name(), "riku-post-build");
        assert_eq!(PluginHook::PostDeploy.plugin_name(), "riku-post-deploy");
    }

    #[test]
    fn test_hook_names() {
        assert_eq!(PluginHook::PreDeploy.hook_name(), "pre-deploy");
        assert_eq!(PluginHook::PreBuild.hook_name(), "pre-build");
        assert_eq!(PluginHook::PostBuild.hook_name(), "post-build");
        assert_eq!(PluginHook::PostDeploy.hook_name(), "post-deploy");
    }

    #[test]
    fn test_hook_display() {
        assert_eq!(format!("{}", PluginHook::PreDeploy), "pre-deploy");
        assert_eq!(format!("{}", PluginHook::PostDeploy), "post-deploy");
    }

    #[test]
    fn test_hook_context_builds_env() {
        let app_path = PathBuf::from("/home/deploy/.riku/apps/myapp");
        let env_path = PathBuf::from("/home/deploy/.riku/envs/myapp");
        let riku_root = PathBuf::from("/home/deploy/.riku");

        let mut app_env = HashMap::new();
        app_env.insert("DATABASE_URL".to_string(), "postgres://localhost/mydb".to_string());
        app_env.insert("SECRET_KEY".to_string(), "abc123".to_string());

        let ctx = HookContext {
            app: "myapp",
            hook: &PluginHook::PostDeploy,
            app_path: &app_path,
            env_path: &env_path,
            riku_root: &riku_root,
            runtime: Some("Python"),
            app_env: &app_env,
        };

        let env = ctx.build_env();

        assert_eq!(env["RIKU_APP"], "myapp");
        assert_eq!(env["RIKU_HOOK"], "post-deploy");
        assert_eq!(env["RIKU_APP_PATH"], "/home/deploy/.riku/apps/myapp");
        assert_eq!(env["RIKU_ENV_PATH"], "/home/deploy/.riku/envs/myapp");
        assert_eq!(env["RIKU_ROOT"], "/home/deploy/.riku");
        assert_eq!(env["RIKU_RUNTIME"], "Python");
        // App env vars are included
        assert_eq!(env["DATABASE_URL"], "postgres://localhost/mydb");
        assert_eq!(env["SECRET_KEY"], "abc123");
    }

    #[test]
    fn test_hook_context_without_runtime() {
        let app_path = PathBuf::from("/tmp/apps/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = PathBuf::from("/tmp/.riku");
        let app_env = HashMap::new();

        let ctx = HookContext {
            app: "myapp",
            hook: &PluginHook::PreDeploy,
            app_path: &app_path,
            env_path: &env_path,
            riku_root: &riku_root,
            runtime: None,
            app_env: &app_env,
        };

        let env = ctx.build_env();
        assert!(!env.contains_key("RIKU_RUNTIME"), "No RIKU_RUNTIME when unknown");
    }

    #[test]
    fn test_riku_vars_override_app_env() {
        // If app ENV file somehow has RIKU_APP set, the context value wins
        let app_path = PathBuf::from("/tmp/apps/myapp");
        let env_path = PathBuf::from("/tmp/envs/myapp");
        let riku_root = PathBuf::from("/tmp/.riku");
        let mut app_env = HashMap::new();
        app_env.insert("RIKU_APP".to_string(), "wrong-app-name".to_string());

        let ctx = HookContext {
            app: "correct-app",
            hook: &PluginHook::PreDeploy,
            app_path: &app_path,
            env_path: &env_path,
            riku_root: &riku_root,
            runtime: None,
            app_env: &app_env,
        };

        let env = ctx.build_env();
        assert_eq!(env["RIKU_APP"], "correct-app");
    }
}
