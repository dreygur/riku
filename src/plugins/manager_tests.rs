//! Tests for [`super::manager::PluginManager`].

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use crate::config::RikuPaths;
use crate::plugins::hooks::{HookContext, PluginHook};
use crate::plugins::manager::PluginManager;

fn setup_paths(temp: &TempDir) -> RikuPaths {
    let riku_root = temp.path().join(".riku");
    fs::create_dir_all(riku_root.join("plugins")).unwrap();
    fs::create_dir_all(riku_root.join("apps")).unwrap();
    fs::create_dir_all(riku_root.join("envs")).unwrap();
    RikuPaths::from_dirs(riku_root, temp.path())
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

    let plugin_path = paths.plugin_root.join("riku-pre-deploy");
    fs::write(
        &plugin_path,
        "#!/bin/sh\necho 'validation failed' >&2\nexit 1\n",
    )
    .unwrap();
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
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("exited with code 1"));
}

#[test]
fn test_post_deploy_hook_failure_is_warning_not_error() {
    let temp = TempDir::new().unwrap();
    let paths = setup_paths(&temp);

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
    let result = manager.run_hook(&ctx);
    assert!(
        result.is_ok(),
        "post-deploy failure should not abort: {:?}",
        result
    );
}

#[test]
fn test_hook_receives_riku_env_vars() {
    let temp = TempDir::new().unwrap();
    let paths = setup_paths(&temp);

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
    assert!(
        output.contains("app=testapp"),
        "RIKU_APP not passed to plugin"
    );
    assert!(
        output.contains("hook=post-build"),
        "RIKU_HOOK not passed to plugin"
    );
}

#[test]
fn test_plugin_timeout_kills_hung_plugin() {
    let temp = TempDir::new().unwrap();
    let paths = setup_paths(&temp);

    let plugin_path = paths.plugin_root.join("riku-pre-deploy");
    fs::write(&plugin_path, "#!/bin/sh\nsleep 60\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&plugin_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    std::env::set_var("RIKU_PLUGIN_TIMEOUT", "1");
    let manager = PluginManager::new(&paths);
    let app_path = PathBuf::from("/tmp/myapp");
    let env_path = PathBuf::from("/tmp/envs/myapp");
    let riku_root = paths.riku_root.clone();
    let app_env = HashMap::new();
    let hook = PluginHook::PreDeploy;

    let ctx = make_ctx("myapp", &hook, &app_path, &env_path, &riku_root, &app_env);
    let result = manager.run_hook(&ctx);
    std::env::remove_var("RIKU_PLUGIN_TIMEOUT");

    assert!(result.is_err(), "Timed-out pre-deploy plugin should abort");
    assert!(result.unwrap_err().to_string().contains("timed out"));
}

#[test]
fn test_plugin_stdout_stderr_captured() {
    let temp = TempDir::new().unwrap();
    let paths = setup_paths(&temp);

    let plugin_path = paths.plugin_root.join("riku-post-deploy");
    fs::write(
        &plugin_path,
        "#!/bin/sh\necho 'stdout line'\necho 'stderr line' >&2\nexit 0\n",
    )
    .unwrap();
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
    let result = manager.run_hook(&ctx);
    assert!(
        result.is_ok(),
        "Plugin with stdout/stderr should not fail: {:?}",
        result
    );
    assert!(result.unwrap(), "Should return true on success");
}
