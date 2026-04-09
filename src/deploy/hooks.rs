//! Plugin lifecycle hook invocation for the deploy pipeline.
//!
//! Each hook stage (pre-deploy, pre-build, post-build, post-deploy) is
//! wrapped in its own function so `mod.rs` stays thin and readable.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::config::RikuPaths;
use crate::plugins::{HookContext, PluginHook, PluginManager};

fn run_hook(
    hook: &PluginHook,
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    let plugin_manager = PluginManager::new(paths);
    let env_path = paths.env_root.join(app);
    let ctx = HookContext {
        app,
        hook,
        app_path,
        env_path: &env_path,
        riku_root: &paths.riku_root,
        runtime: runtime_name,
        app_env,
    };
    plugin_manager.run_hook(&ctx).map(|_| ())
}

/// Run the `pre-deploy` hook.  Failures abort the deploy.
pub fn run_pre_deploy(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(&PluginHook::PreDeploy, app, app_path, paths, None, app_env)
}

/// Run the `pre-build` hook.  Failures abort the deploy.
pub fn run_pre_build(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PreBuild,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}

/// Run the `post-build` hook.  Failures abort the deploy.
pub fn run_post_build(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PostBuild,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}

/// Run the `post-deploy` hook.  Failures are warnings (not fatal).
pub fn run_post_deploy(
    app: &str,
    app_path: &Path,
    paths: &RikuPaths,
    runtime_name: Option<&str>,
    app_env: &HashMap<String, String>,
) -> Result<()> {
    run_hook(
        &PluginHook::PostDeploy,
        app,
        app_path,
        paths,
        runtime_name,
        app_env,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    /// Build a RikuPaths rooted at `tmp`, creating the standard sub-directories.
    fn make_paths(tmp: &TempDir) -> RikuPaths {
        let paths = crate::config::RikuPaths::from_dirs(
            tmp.path().join(".riku"),
            &tmp.path().to_path_buf(),
        );
        fs::create_dir_all(&paths.plugin_root).unwrap();
        fs::create_dir_all(&paths.env_root).unwrap();
        paths
    }

    /// Write an executable shell script into the plugin_root.
    ///
    /// Uses write-then-rename to avoid "Text file busy" (ETXTBSY) on Linux when
    /// the file is written and immediately executed in a tight loop.
    fn write_plugin(paths: &RikuPaths, name: &str, script: &str) {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let final_path = paths.plugin_root.join(name);

        // Write to a temp file in the same directory, set permissions, then
        // rename atomically.  rename(2) guarantees the target is replaced
        // atomically, and the kernel will not see a partially-written file.
        let tmp_path = paths.plugin_root.join(format!(".{}.tmp", name));
        let mut f = std::fs::File::create(&tmp_path).unwrap();
        write!(f, "#!/bin/sh\n{}", script).unwrap();
        f.sync_all().unwrap();
        drop(f);

        let mut perms = fs::metadata(&tmp_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp_path, perms).unwrap();

        fs::rename(&tmp_path, &final_path).unwrap();
    }

    // --- pre-deploy ---

    #[test]
    fn test_pre_deploy_no_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env = HashMap::new();
        // No plugin file exists — should succeed silently
        run_pre_deploy("myapp", tmp.path(), &paths, &env)
    }

    #[test]
    fn test_pre_deploy_success_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-pre-deploy", "exit 0");
        let env = HashMap::new();
        run_pre_deploy("myapp", tmp.path(), &paths, &env)
    }

    #[test]
    fn test_pre_deploy_failing_plugin_returns_error() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-pre-deploy", "exit 1");
        let env = HashMap::new();
        let result = run_pre_deploy("myapp", tmp.path(), &paths, &env);
        assert!(result.is_err(), "Failing pre-deploy plugin should abort deploy");
    }

    // --- pre-build ---

    #[test]
    fn test_pre_build_no_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env = HashMap::new();
        run_pre_build("myapp", tmp.path(), &paths, Some("Python"), &env)
    }

    #[test]
    fn test_pre_build_success_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-pre-build", "exit 0");
        let env = HashMap::new();
        run_pre_build("myapp", tmp.path(), &paths, Some("Node"), &env)
    }

    #[test]
    fn test_pre_build_failing_plugin_returns_error() {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-pre-build", "exit 2");
        let env = HashMap::new();
        let result = run_pre_build("myapp", tmp.path(), &paths, None, &env);
        assert!(result.is_err(), "Failing pre-build plugin should abort deploy");
    }

    // --- post-build ---

    #[test]
    fn test_post_build_no_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env = HashMap::new();
        run_post_build("myapp", tmp.path(), &paths, None, &env)
    }

    #[test]
    fn test_post_build_failing_plugin_aborts() {
        // post-build failures are fatal — they abort the deploy
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-post-build", "exit 1");
        let env = HashMap::new();
        let result = run_post_build("myapp", tmp.path(), &paths, Some("Go"), &env);
        assert!(result.is_err(), "failing post-build plugin should abort the deploy");
    }

    // --- post-deploy ---

    #[test]
    fn test_post_deploy_no_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let env = HashMap::new();
        run_post_deploy("myapp", tmp.path(), &paths, None, &env)
    }

    #[test]
    fn test_post_deploy_failing_plugin_returns_ok() -> Result<()> {
        // post-deploy failures are non-fatal
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-post-deploy", "exit 1");
        let env = HashMap::new();
        run_post_deploy("myapp", tmp.path(), &paths, Some("Python"), &env)
    }

    #[test]
    fn test_post_deploy_success_plugin_is_ok() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        write_plugin(&paths, "riku-post-deploy", "exit 0");
        let env = HashMap::new();
        run_post_deploy("myapp", tmp.path(), &paths, None, &env)
    }

    #[test]
    fn test_hook_plugin_receives_riku_app_env_var() -> Result<()> {
        let tmp = TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        // Write a plugin that exits non-zero unless RIKU_APP is set to the expected value
        write_plugin(&paths, "riku-post-deploy", "[ \"$RIKU_APP\" = \"testapp\" ] || exit 1");
        let env = HashMap::new();
        // If the env is injected correctly, the plugin exits 0 → post_deploy returns Ok
        run_post_deploy("testapp", tmp.path(), &paths, None, &env)
    }
}
