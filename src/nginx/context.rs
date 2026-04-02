//! Tera context construction for nginx config templates.
//!
//! Each helper populates a distinct section of the template context from
//! sanitized environment variables and resolved path defaults.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::util::ensure_path_within;

use super::cloudflare::generate_cloudflare_ips_config;
use super::ssl::ensure_ssl_certificates;

/// Insert address-related context values.
pub(super) fn insert_address_context(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
    app: &str,
    app_path: &Path,
) {
    let bind_address = env
        .get("BIND_ADDRESS")
        .cloned()
        .unwrap_or("127.0.0.1".to_string());

    let nginx_ipv4_address = env
        .get("NGINX_IPV4_ADDRESS")
        .cloned()
        .unwrap_or("0.0.0.0".to_string());

    let disable_ipv6 = env
        .get("DISABLE_IPV6")
        .map(|v| v.to_lowercase() == "true" || v == "1" || v == "yes")
        .unwrap_or(false);

    let nginx_ipv6_address = if disable_ipv6 {
        String::new()
    } else {
        env.get("NGINX_IPV6_ADDRESS")
            .cloned()
            .unwrap_or("[::]".to_string())
    };

    let nginx_server_name = env
        .get("NGINX_SERVER_NAME")
        .cloned()
        .unwrap_or(format!("{}.example.com", app));

    let nginx_socket = env.get("NGINX_SOCKET").cloned().unwrap_or(
        paths
            .nginx_root
            .join(format!("{}.sock", app))
            .to_string_lossy()
            .to_string(),
    );

    let nginx_document_root = env
        .get("NGINX_DOCUMENT_ROOT")
        .cloned()
        .unwrap_or(format!("{}/public", app_path.to_string_lossy()));

    context.insert("BIND_ADDRESS", &bind_address);
    context.insert("NGINX_IPV4_ADDRESS", &nginx_ipv4_address);
    context.insert("NGINX_IPV6_ADDRESS", &nginx_ipv6_address);
    context.insert("NGINX_SERVER_NAME", &nginx_server_name);
    context.insert("NGINX_SOCKET", &nginx_socket);
    context.insert("NGINX_DOCUMENT_ROOT", &nginx_document_root);
}

/// Insert cache-related context values.
pub(super) fn insert_cache_context(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
    app: &str,
) {
    let nginx_cache_size = env
        .get("NGINX_CACHE_SIZE")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_SIZE_DEFAULT.to_string());

    let nginx_cache_time = env
        .get("NGINX_CACHE_TIME")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_TIME_DEFAULT.to_string());

    let nginx_cache_redirects = env
        .get("NGINX_CACHE_REDIRECTS")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_REDIRECTS_DEFAULT.to_string());

    let nginx_cache_any = env
        .get("NGINX_CACHE_ANY")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_ANY_DEFAULT.to_string());

    let nginx_cache_control = env
        .get("NGINX_CACHE_CONTROL")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_CONTROL_DEFAULT.to_string());

    let nginx_cache_expiry = env
        .get("NGINX_CACHE_EXPIRY")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_EXPIRY_DEFAULT.to_string());

    let nginx_cache_path = env
        .get("NGINX_CACHE_PATH")
        .cloned()
        .unwrap_or(paths.cache_root.join(app).to_string_lossy().to_string());

    context.insert("NGINX_CACHE_SIZE", &nginx_cache_size);
    context.insert("NGINX_CACHE_TIME", &nginx_cache_time);
    context.insert("NGINX_CACHE_REDIRECTS", &nginx_cache_redirects);
    context.insert("NGINX_CACHE_ANY", &nginx_cache_any);
    context.insert("NGINX_CACHE_CONTROL", &nginx_cache_control);
    context.insert("NGINX_CACHE_EXPIRY", &nginx_cache_expiry);
    context.insert("NGINX_CACHE_PATH", &nginx_cache_path);
}

/// Insert feature-flag context values (Cloudflare ACL, git folder exposure).
pub(super) fn insert_feature_flags(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) {
    let nginx_cloudflare_acl = env
        .get("NGINX_CLOUDFLARE_ACL")
        .map(|v| v.to_lowercase() == "true" || v == "1" || v == "yes")
        .unwrap_or(false);

    if nginx_cloudflare_acl {
        if let Err(e) = generate_cloudflare_ips_config(paths) {
            crate::util::echo(
                &format!("Warning: Failed to fetch Cloudflare IPs: {}", e),
                "yellow",
            );
        }
    }

    let nginx_allow_git_folders = env
        .get("NGINX_ALLOW_GIT_FOLDERS")
        .map(|v| v.to_lowercase() == "true" || v == "1" || v == "yes")
        .unwrap_or(false);

    context.insert("NGINX_CLOUDFLARE_ACL", &nginx_cloudflare_acl.to_string());
    context.insert(
        "NGINX_ALLOW_GIT_FOLDERS",
        &nginx_allow_git_folders.to_string(),
    );
}

/// Read and insert NGINX_INCLUDE_FILE content, guarding against path traversal.
pub(super) fn insert_include_file(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    app_path: &Path,
) {
    if let Some(include_file) = env.get("NGINX_INCLUDE_FILE") {
        let include_path = app_path.join(include_file);
        if include_path.exists() {
            match ensure_path_within(&include_path, app_path) {
                Ok(()) => {
                    if let Ok(content) = fs::read_to_string(&include_path) {
                        context.insert("NGINX_INCLUDE_CONTENT", &content);
                    }
                }
                Err(_) => {
                    crate::util::echo(
                        &format!(
                            "Warning: NGINX_INCLUDE_FILE '{}' is outside the app directory, ignoring",
                            include_file
                        ),
                        "yellow",
                    );
                }
            }
        }
    }
}

/// Insert portmap/proxy port context values.
pub(super) fn insert_portmap_context(context: &mut tera::Context, env: &HashMap<String, String>) {
    let nginx_external_port = env
        .get("NGINX_EXTERNAL_PORT")
        .cloned()
        .unwrap_or("80".to_string());

    let nginx_internal_port = env
        .get("NGINX_INTERNAL_PORT")
        .cloned()
        .unwrap_or_else(|| env.get("PORT").cloned().unwrap_or("8080".to_string()));

    context.insert("NGINX_EXTERNAL_PORT", &nginx_external_port);
    context.insert("NGINX_INTERNAL_PORT", &nginx_internal_port);
}

/// Build the full Tera context from sanitized environment variables and paths.
pub(super) fn build_context(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> anyhow::Result<tera::Context> {
    let mut context = tera::Context::new();

    context.insert("APP", app);
    context.insert("INTERNAL_NGINX_APP_ROOT", &app_path.to_string_lossy());

    for (key, value) in env {
        context.insert(key, value);
    }

    insert_address_context(&mut context, env, paths, app, app_path);
    insert_cache_context(&mut context, env, paths, app);
    insert_feature_flags(&mut context, env, paths);
    insert_include_file(&mut context, env, app_path);
    insert_portmap_context(&mut context, env);

    context.insert("RIKU_ROOT", &paths.riku_root.to_string_lossy());
    context.insert("ACME_WWW", &paths.acme_www.to_string_lossy());
    context.insert(
        "ACME_ROOT_CA",
        &env.get("ACME_ROOT_CA")
            .cloned()
            .unwrap_or_else(|| "letsencrypt.org".to_string()),
    );

    if env.contains_key("NGINX_HTTPS_ONLY") {
        if let Some(server_name) = env.get("NGINX_SERVER_NAME") {
            let domains: Vec<String> = server_name
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            if !domains.is_empty() {
                ensure_ssl_certificates(app, &domains, paths)?;
            }
        }
    }

    if let Some(socket) = env.get("UWSGI_SOCKET") {
        context.insert("UWSGI_SOCKET", socket);
    }

    Ok(context)
}
