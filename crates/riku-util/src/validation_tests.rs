use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_sanitize_valid_name() {
    assert_eq!(sanitize_app_name("my-app"), "my-app");
    assert_eq!(sanitize_app_name("my_app.v2"), "my_app.v2");
    assert_eq!(sanitize_app_name("app123"), "app123");
}

#[test]
fn test_sanitize_invalid_chars() {
    assert_eq!(sanitize_app_name("my app!@#"), "myapp");
    assert_eq!(sanitize_app_name("app/name"), "appname");
    assert_eq!(sanitize_app_name("a b c"), "abc");
}

#[test]
fn test_sanitize_leading_slashes() {
    assert_eq!(sanitize_app_name("/my-app"), "my-app");
    assert_eq!(sanitize_app_name("///app"), "app");
    assert_eq!(sanitize_app_name("/"), "");
}

#[test]
fn test_sanitize_trailing_whitespace() {
    assert_eq!(sanitize_app_name("my-app  "), "my-app");
    assert_eq!(sanitize_app_name("app\t"), "app");
}

#[test]
fn test_sanitize_path_traversal() {
    assert_eq!(sanitize_app_name(".."), "");
    assert_eq!(sanitize_app_name("../etc/passwd"), "");
    assert_eq!(sanitize_app_name("app/../secret"), "");
    assert_eq!(sanitize_app_name("my..app"), "");
    assert_eq!(sanitize_app_name("..."), "");
}

#[test]
fn test_sanitize_dot_only_names() {
    assert_eq!(sanitize_app_name("."), "");
    assert_eq!(sanitize_app_name("..."), "");
}

#[test]
fn test_sanitize_valid_dotted_names() {
    assert_eq!(sanitize_app_name("my-app.v2"), "my-app.v2");
    assert_eq!(sanitize_app_name(".hidden-app"), ".hidden-app");
}

#[test]
fn refuses_an_app_name_ending_in_git() {
    let err = validate_app_name("shop.git").unwrap_err().to_string();
    assert!(
        err.contains("reserved"),
        "the error must say why shop.git is refused, got: {err}"
    );
    assert_eq!(sanitize_app_name("shop.git"), "");
    assert_eq!(validate_app_name("shop.gitlab").unwrap(), "shop.gitlab");
}

/// The dashboard, the supervisor control plane and tests call the commands
/// that use this check, so a bad or missing app must come back as an error
/// for the caller, never end the process.
#[test]
fn require_app_returns_an_error_for_a_missing_or_bad_app() {
    let apps = TempDir::new().unwrap();
    fs::create_dir(apps.path().join("shop")).unwrap();

    assert_eq!(require_app("shop", apps.path()).unwrap(), "shop");
    assert_eq!(
        require_app("ghost", apps.path()).unwrap_err().to_string(),
        "app 'ghost' not found."
    );
    assert_eq!(
        require_app("../etc", apps.path()).unwrap_err().to_string(),
        "invalid app name '../etc'."
    );
}

#[test]
fn test_ensure_path_within_accepts_child() {
    let temp_dir = TempDir::new().unwrap();
    let child = temp_dir.path().join("subdir");
    fs::create_dir(&child).unwrap();
    assert!(ensure_path_within(&child, temp_dir.path()).is_ok());
}

#[test]
fn test_ensure_path_within_rejects_outside() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path().join("root");
    fs::create_dir(&root).unwrap();
    assert!(ensure_path_within(temp_dir.path(), &root).is_err());
}
