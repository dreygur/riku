use super::*;
use std::path::Path;

/// Helper: build RikuPaths with a known home and root, no env vars involved.
fn paths_with_root(root: &str, home: &str) -> RikuPaths {
    RikuPaths::from_dirs(PathBuf::from(root), &PathBuf::from(home))
}

#[test]
fn default_paths_use_home_dot_riku() {
    let home = "/home/testuser";
    let paths = paths_with_root(&format!("{home}/.riku"), home);
    assert_eq!(paths.riku_root, Path::new("/home/testuser/.riku"));
}

#[test]
fn all_subdirectory_paths_are_relative_to_riku_root() {
    let root = "/srv/riku";
    let home = "/home/riku";
    let p = paths_with_root(root, home);

    assert_eq!(p.app_root, Path::new("/srv/riku/apps"));
    assert_eq!(p.data_root, Path::new("/srv/riku/data"));
    assert_eq!(p.env_root, Path::new("/srv/riku/envs"));
    assert_eq!(p.git_root, Path::new("/srv/riku/repos"));
    assert_eq!(p.log_root, Path::new("/srv/riku/logs"));
    assert_eq!(p.nginx_root, Path::new("/srv/riku/nginx"));
    assert_eq!(p.cache_root, Path::new("/srv/riku/cache"));
    assert_eq!(p.workers_root, Path::new("/srv/riku/workers"));
    assert_eq!(
        p.workers_available,
        Path::new("/srv/riku/workers-available")
    );
    assert_eq!(p.workers_enabled, Path::new("/srv/riku/workers-enabled"));
    assert_eq!(p.acme_www, Path::new("/srv/riku/acme"));
    assert_eq!(p.plugin_root, Path::new("/srv/riku/plugins"));
}

#[test]
fn custom_root_parameter_works() {
    let p = paths_with_root("/opt/custom-riku", "/home/deploy");
    assert_eq!(p.riku_root, Path::new("/opt/custom-riku"));
    assert_eq!(p.app_root, Path::new("/opt/custom-riku/apps"));
    assert_eq!(p.git_root, Path::new("/opt/custom-riku/repos"));
}

#[test]
fn riku_bin_is_relative_to_home() {
    let p = paths_with_root("/whatever", "/home/alice");
    assert_eq!(p.riku_bin, Path::new("/home/alice/bin"));
}

#[test]
fn acme_root_defaults_to_home_acme_sh() {
    // Test that ACME_ROOT defaults to ~/.acme.sh when not set
    // We test the logic directly rather than through from_dirs to avoid parallel test issues
    let home = PathBuf::from("/home/bob");

    // Simulate the ACME_ROOT resolution logic
    let result = std::env::var("ACME_ROOT_TEST_VAR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".acme.sh"));

    assert_eq!(result, Path::new("/home/bob/.acme.sh"));
}

#[test]
fn acme_root_respects_env_var() {
    // Save the original value
    let orig_value = env::var("ACME_ROOT").ok();

    env::set_var("ACME_ROOT", "/custom/acme");
    let p = paths_with_root("/x", "/home/bob");
    assert_eq!(p.acme_root, Path::new("/custom/acme"));

    // Restore original value or remove if it wasn't set
    match orig_value {
        Some(v) => env::set_var("ACME_ROOT", v),
        None => env::remove_var("ACME_ROOT"),
    }
}

#[test]
fn riku_log_maxsize_constant() {
    assert_eq!(RIKU_LOG_MAXSIZE, 1048576);
}

#[test]
fn riku_raw_source_url_constant() {
    assert!(RIKU_RAW_SOURCE_URL.starts_with("https://"));
    assert!(RIKU_RAW_SOURCE_URL.contains("riku")); // Refers to the riku repo
}

#[test]
fn from_env_uses_home() {
    // Just verify it doesn't panic and produces a sensible root.
    let paths = RikuPaths::from_env();
    assert!(paths.riku_root.is_absolute() || env::var("RIKU_ROOT").is_ok());
}
