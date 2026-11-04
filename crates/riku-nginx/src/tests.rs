//! `generate_nginx_config` itself: it writes the live config, refuses an
//! app with no backend, and ignores an nginx config the app ships.

use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

use super::generate_nginx_config;
use crate::test_support::{generate_live, paths_with_nginx, ACCEPT};

#[test]
fn test_generate_nginx_config() {
    let temp_dir = TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), ACCEPT);

    let config = generate_live(
        temp_dir.path(),
        &paths,
        &[("PORT", "8080"), ("RIKU_DOMAINS", "myapp.example.com")],
    );

    assert!(config.contains("myapp.example.com"), "{config}");
}

#[test]
fn empty_backends_are_refused() {
    let temp_dir = TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), ACCEPT);

    let result = generate_nginx_config("myapp", temp_dir.path(), &HashMap::new(), &[], &paths);

    assert!(result.is_err(), "no backends should be an error");
    assert!(
        !paths.nginx_root.join("myapp.conf").exists(),
        "a config was written without any backend"
    );
}

#[test]
fn an_app_shipped_nginx_conf_is_ignored_for_the_generated_config() {
    let temp_dir = TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), ACCEPT);
    let app_path = temp_dir.path().join("myapp");
    fs::create_dir(&app_path).unwrap();
    fs::write(
        app_path.join("nginx.conf"),
        "server { server_name victim.example.com; }\n",
    )
    .unwrap();

    let live = generate_live(
        temp_dir.path(),
        &paths,
        &[("RIKU_DOMAINS", "myapp.example.com")],
    );

    assert!(!live.contains("victim.example.com"), "{live}");
    assert!(live.contains("myapp.example.com"), "{live}");
}
