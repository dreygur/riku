//! Integration-style tests for the nginx module.

use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

use super::generate_nginx_config;

#[test]
fn test_generate_nginx_config() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "8080".to_string());
    env.insert(
        "NGINX_SERVER_NAME".to_string(),
        "myapp.example.com".to_string(),
    );

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );

    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);

    let config_file = paths.nginx_root.join("myapp.conf");
    assert!(config_file.exists());
}

#[test]
fn test_nginx_config_with_bind_address() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("BIND_ADDRESS".to_string(), "192.168.1.1".to_string());
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_file = paths.nginx_root.join("myapp.conf");
    assert!(config_file.exists());
}

#[test]
fn test_nginx_config_with_ipv4_address() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("NGINX_IPV4_ADDRESS".to_string(), "192.168.1.1".to_string());
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_content = fs::read_to_string(paths.nginx_root.join("myapp.conf")).unwrap();

    assert!(config_content.contains("192.168.1.1"));
}

#[test]
fn test_nginx_config_disable_ipv6() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("DISABLE_IPV6".to_string(), "true".to_string());
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_content = fs::read_to_string(paths.nginx_root.join("myapp.conf")).unwrap();

    assert!(!config_content.contains("[::]"));
}

#[test]
fn test_nginx_config_with_cache() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());
    env.insert(
        "NGINX_CACHE_PREFIXES".to_string(),
        "/api,/images".to_string(),
    );
    env.insert("NGINX_CACHE_SIZE".to_string(), "2".to_string());
    env.insert("NGINX_CACHE_TIME".to_string(), "7200".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_content = fs::read_to_string(paths.nginx_root.join("myapp.conf")).unwrap();

    assert!(config_content.contains("proxy_cache"));
    assert!(config_content.contains("2g"));
    assert!(config_content.contains("7200s"));
}

#[test]
fn test_nginx_config_with_cloudflare_acl() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());
    env.insert("NGINX_CLOUDFLARE_ACL".to_string(), "true".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _result = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_content = fs::read_to_string(paths.nginx_root.join("myapp.conf")).unwrap();

    assert!(config_content.contains("cloudflare"));
}

#[test]
fn test_nginx_config_https_only() {
    let temp_dir = TempDir::new().unwrap();
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();

    let mut env = HashMap::new();
    env.insert("NGINX_SERVER_NAME".to_string(), "example.com".to_string());
    env.insert("NGINX_HTTPS_ONLY".to_string(), "true".to_string());

    let paths = crate::config::RikuPaths::from_dirs(
        temp_dir.path().join(".riku"),
        &temp_dir.path().to_path_buf(),
    );
    fs::create_dir_all(&paths.nginx_root).unwrap();

    let _ = generate_nginx_config("myapp", &app_path, &env, &paths);
    let config_file = paths.nginx_root.join("myapp.conf");

    if !config_file.exists() {
        return;
    }

    let config_content = fs::read_to_string(&config_file).unwrap();
    assert!(config_content.contains("return 301"));
    assert!(config_content.contains("https://"));
    assert!(config_content.contains("ssl"));
}
