use super::*;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn make_paths(tmp: &TempDir) -> crate::config::RikuPaths {
    let paths = crate::config::RikuPaths::for_tests(tmp.path());
    std::fs::create_dir_all(&paths.nginx_root).unwrap();
    paths
}

// ── insert_address_context ─────────────────────────────────────────────

#[test]
fn test_address_defaults_applied_when_env_empty() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env = HashMap::new();
    let app_path = tmp.path().join("app");

    let mut ctx = tera::Context::new();
    insert_address_context(&mut ctx, &env, &paths, "myapp", &app_path);

    assert_eq!(ctx.get("BIND_ADDRESS").unwrap(), "127.0.0.1");
    assert_eq!(ctx.get("NGINX_IPV4_ADDRESS").unwrap(), "0.0.0.0");
    assert_eq!(ctx.get("NGINX_IPV6_ADDRESS").unwrap(), "[::]");
    assert!(ctx
        .get("NGINX_SERVER_NAME")
        .unwrap()
        .to_string()
        .contains("myapp"));
}

#[test]
fn test_address_custom_bind_address_used() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let mut env = HashMap::new();
    env.insert("BIND_ADDRESS".to_string(), "10.0.0.5".to_string());

    let mut ctx = tera::Context::new();
    insert_address_context(&mut ctx, &env, &paths, "app", &tmp.path().join("app"));

    assert_eq!(ctx.get("BIND_ADDRESS").unwrap(), "10.0.0.5");
}

#[test]
fn test_address_disable_ipv6_clears_ipv6_address() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let mut env = HashMap::new();
    env.insert("DISABLE_IPV6".to_string(), "true".to_string());

    let mut ctx = tera::Context::new();
    insert_address_context(&mut ctx, &env, &paths, "app", &tmp.path().join("app"));

    assert_eq!(ctx.get("NGINX_IPV6_ADDRESS").unwrap(), "");
}

#[test]
fn test_address_disable_ipv6_with_one_value() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let mut env = HashMap::new();
    env.insert("DISABLE_IPV6".to_string(), "1".to_string());

    let mut ctx = tera::Context::new();
    insert_address_context(&mut ctx, &env, &paths, "app", &tmp.path().join("app"));

    assert_eq!(ctx.get("NGINX_IPV6_ADDRESS").unwrap(), "");
}

// ── insert_portmap_context ─────────────────────────────────────────────

#[test]
fn test_portmap_defaults_to_80_and_8080() {
    let env = HashMap::new();
    let mut ctx = tera::Context::new();
    insert_portmap_context(&mut ctx, &env);

    assert_eq!(ctx.get("NGINX_EXTERNAL_PORT").unwrap(), "80");
    assert_eq!(ctx.get("NGINX_INTERNAL_PORT").unwrap(), "8080");
}

#[test]
fn test_portmap_port_env_var_used_as_internal_port() {
    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "5000".to_string());

    let mut ctx = tera::Context::new();
    insert_portmap_context(&mut ctx, &env);

    assert_eq!(ctx.get("NGINX_INTERNAL_PORT").unwrap(), "5000");
}

#[test]
fn test_portmap_explicit_internal_port_overrides_port() {
    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "5000".to_string());
    env.insert("NGINX_INTERNAL_PORT".to_string(), "9000".to_string());

    let mut ctx = tera::Context::new();
    insert_portmap_context(&mut ctx, &env);

    assert_eq!(ctx.get("NGINX_INTERNAL_PORT").unwrap(), "9000");
}

// ── insert_cache_context ───────────────────────────────────────────────

#[test]
fn test_cache_defaults_inserted() {
    let tmp = TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let env = HashMap::new();

    let mut ctx = tera::Context::new();
    insert_cache_context(&mut ctx, &env, &paths, "app");

    assert!(ctx.get("NGINX_CACHE_SIZE").is_some());
    assert!(ctx.get("NGINX_CACHE_TIME").is_some());
    assert!(ctx.get("NGINX_CACHE_PATH").is_some());
}

// ── insert_include_file ────────────────────────────────────────────────

#[test]
fn test_include_file_within_app_dir_is_inserted() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("app");
    std::fs::create_dir_all(&app_path).unwrap();
    std::fs::write(app_path.join("nginx_extra.conf"), "proxy_read_timeout 60;").unwrap();

    let paths = make_paths(&tmp);
    let mut env = HashMap::new();
    env.insert(
        "NGINX_INCLUDE_FILE".to_string(),
        "nginx_extra.conf".to_string(),
    );

    let mut ctx = tera::Context::new();
    insert_include_file(&mut ctx, &env, &app_path, &paths);

    let val = ctx.get("NGINX_INCLUDE_CONTENT").unwrap().to_string();
    assert!(val.contains("proxy_read_timeout"));
}

#[test]
fn test_include_file_outside_app_dir_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("app");
    std::fs::create_dir_all(&app_path).unwrap();
    let paths = make_paths(&tmp);

    let mut env = HashMap::new();
    // Path traversal attempt
    env.insert(
        "NGINX_INCLUDE_FILE".to_string(),
        "../secret.conf".to_string(),
    );

    let mut ctx = tera::Context::new();
    insert_include_file(&mut ctx, &env, &app_path, &paths);

    // The key is always present (so templates never see an undefined
    // variable), but a rejected traversal attempt leaves it empty.
    assert_eq!(ctx.get("NGINX_INCLUDE_CONTENT").unwrap(), "");
}

#[test]
fn test_include_file_content_runs_through_the_filter_chain() {
    let tmp = TempDir::new().unwrap();
    let app_path = tmp.path().join("app");
    std::fs::create_dir_all(&app_path).unwrap();
    let paths = make_paths(&tmp);

    // A real installed filter plugin, dispatched through the actual
    // FilterBus — not a mock — appending a marker line to whatever seed
    // content it receives.
    let bundle = paths.plugin_root.join("augmenter");
    std::fs::create_dir_all(bundle.join("bin")).unwrap();
    std::fs::write(
            bundle.join("bin/on-filter"),
            "#!/bin/sh\nread line\ndata=$(printf '%s' \"$line\" | sed 's/.*\"data\":\"\\([^\"]*\\)\".*/\\1/')\nprintf '{\"data\":\"%s# augmented\\\\n\"}' \"$data\"\n",
        )
        .unwrap();
    std::fs::set_permissions(
        bundle.join("bin/on-filter"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::write(
            bundle.join("riku-plugin.toml"),
            format!(
                "name=\"augmenter\"\nversion=\"1\"\ntype=\"notifier\"\napi={}\nentry=\"bin/on-filter\"\n[filters]\nsubscribe=[\"nginx.include_content\"]\n",
                riku_plugins::RIKU_PLUGIN_API
            ),
        )
        .unwrap();

    let env = HashMap::new();
    let mut ctx = tera::Context::new();
    insert_include_file(&mut ctx, &env, &app_path, &paths);

    let val = ctx.get("NGINX_INCLUDE_CONTENT").unwrap().to_string();
    assert!(val.contains("augmented"), "got: {val}");
}
