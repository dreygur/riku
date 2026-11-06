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
