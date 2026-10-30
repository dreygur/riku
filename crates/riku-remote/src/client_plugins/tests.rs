use super::discovery::{get_client_plugin_path, list_client_plugins};
use super::execute::is_executable;
use std::env;
use std::fs;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

// Mutex to ensure tests don't run in parallel when modifying HOME
static HOME_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_home_mutex() -> &'static Mutex<()> {
    HOME_MUTEX.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_get_client_plugin_path() {
    let _guard = get_home_mutex().lock().unwrap();
    let original_home = env::var("HOME").ok();

    // Set HOME for testing
    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path());

    let path = get_client_plugin_path("test-plugin").unwrap();
    assert!(path.ends_with(".riku/client-plugins/test-plugin"));

    // Restore original HOME
    match original_home {
        Some(home) => env::set_var("HOME", home),
        None => env::remove_var("HOME"),
    }
}

#[test]
fn test_is_executable() {
    let temp_dir = TempDir::new().unwrap();
    let script_path = temp_dir.path().join("test-script");

    // Create a script
    let mut file = fs::File::create(&script_path).unwrap();
    writeln!(file, "#!/bin/sh").unwrap();
    writeln!(file, "echo test").unwrap();

    // Should not be executable yet
    assert!(!is_executable(&script_path));

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    // Should be executable now
    assert!(is_executable(&script_path));
}

#[test]
fn test_list_client_plugins_empty() {
    let _guard = get_home_mutex().lock().unwrap();
    let original_home = env::var("HOME").ok();

    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path());

    let plugins = list_client_plugins().unwrap();
    assert!(plugins.is_empty());

    // Restore original HOME
    match original_home {
        Some(home) => env::set_var("HOME", home),
        None => env::remove_var("HOME"),
    }
}

#[test]
fn test_list_client_plugins() {
    let _guard = get_home_mutex().lock().unwrap();
    let original_home = env::var("HOME").ok();

    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path());

    // Create plugins directory
    let plugins_dir = temp_dir.path().join(".riku").join("client-plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    // Create a plugin
    let plugin_path = plugins_dir.join("test-plugin");
    let mut file = fs::File::create(&plugin_path).unwrap();
    writeln!(file, "#!/bin/sh").unwrap();

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&plugin_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&plugin_path, perms).unwrap();
    }

    let plugins = list_client_plugins().unwrap();
    assert_eq!(plugins, vec!["test-plugin"]);

    // Restore original HOME
    match original_home {
        Some(home) => env::set_var("HOME", home),
        None => env::remove_var("HOME"),
    }
}
