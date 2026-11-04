//! What app ENV options change in the generated site config.

use tempfile::TempDir;

use crate::test_support::{generate_live, paths_with_nginx, ACCEPT};

fn rendered(env: &[(&str, &str)]) -> String {
    let temp_dir = TempDir::new().unwrap();
    let paths = paths_with_nginx(temp_dir.path(), ACCEPT);
    generate_live(temp_dir.path(), &paths, env)
}

#[test]
fn test_nginx_config_with_bind_address() {
    let config = rendered(&[
        ("RIKU_BIND_ADDRESS", "192.168.1.1"),
        ("RIKU_DOMAINS", "example.com"),
    ]);
    assert!(config.contains("example.com"), "{config}");
}

#[test]
fn test_nginx_config_with_ipv4_address() {
    let config = rendered(&[
        ("RIKU_NGINX_IPV4_ADDRESS", "192.168.1.1"),
        ("RIKU_DOMAINS", "example.com"),
    ]);
    assert!(config.contains("192.168.1.1"), "{config}");
}

#[test]
fn test_nginx_config_disable_ipv6() {
    let config = rendered(&[
        ("RIKU_DISABLE_IPV6", "true"),
        ("RIKU_DOMAINS", "example.com"),
    ]);
    assert!(!config.contains("[::]"), "{config}");
}

#[test]
fn test_nginx_config_with_cache() {
    let config = rendered(&[
        ("RIKU_DOMAINS", "example.com"),
        ("RIKU_NGINX_CACHE_PREFIXES", "/api,/images"),
        ("RIKU_NGINX_CACHE_SIZE", "2"),
        ("RIKU_NGINX_CACHE_TIME", "7200"),
    ]);
    assert!(config.contains("proxy_cache"), "{config}");
    assert!(config.contains("2g"), "{config}");
    assert!(config.contains("7200s"), "{config}");
}

#[test]
fn test_nginx_config_with_cloudflare_acl() {
    let config = rendered(&[
        ("RIKU_DOMAINS", "example.com"),
        ("RIKU_NGINX_CLOUDFLARE_ACL", "true"),
    ]);
    assert!(config.contains("cloudflare"), "{config}");
}

#[test]
fn test_nginx_config_https_only() {
    let config = rendered(&[("RIKU_DOMAINS", "example.com"), ("RIKU_HTTPS_ONLY", "true")]);
    assert!(config.contains("return 301"), "{config}");
    assert!(config.contains("https://"), "{config}");
    assert!(config.contains("ssl"), "{config}");
}
