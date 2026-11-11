use super::discovery::{client_plugin_path_in, list_client_plugins_in};
use super::execute::is_executable;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_get_client_plugin_path() {
    let home = TempDir::new().unwrap();

    let path = client_plugin_path_in(home.path(), "test-plugin");
    assert!(path.ends_with(".riku/client-plugins/test-plugin"));
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
    let home = TempDir::new().unwrap();

    let plugins = list_client_plugins_in(home.path()).unwrap();
    assert!(plugins.is_empty());
}

#[test]
fn test_list_client_plugins() {
    let home = TempDir::new().unwrap();

    // Create plugins directory
    let plugins_dir = home.path().join(".riku").join("client-plugins");
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

    let plugins = list_client_plugins_in(home.path()).unwrap();
    assert_eq!(plugins, vec!["test-plugin"]);
}
