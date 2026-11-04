//! The proxy cache section of the nginx template context: zone size, cache
//! times and the on-disk cache path, each from app ENV or its default.

use std::collections::HashMap;

/// Insert cache-related context values.
pub(super) fn insert_cache_context(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
    app: &str,
) {
    let nginx_cache_size = env
        .get("RIKU_NGINX_CACHE_SIZE")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_SIZE_DEFAULT.to_string());

    let nginx_cache_time = env
        .get("RIKU_NGINX_CACHE_TIME")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_TIME_DEFAULT.to_string());

    let nginx_cache_redirects = env
        .get("RIKU_NGINX_CACHE_REDIRECTS")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_REDIRECTS_DEFAULT.to_string());

    let nginx_cache_any = env
        .get("RIKU_NGINX_CACHE_ANY")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_ANY_DEFAULT.to_string());

    let nginx_cache_control = env
        .get("RIKU_NGINX_CACHE_CONTROL")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_CONTROL_DEFAULT.to_string());

    let nginx_cache_expiry = env
        .get("RIKU_NGINX_CACHE_EXPIRY")
        .cloned()
        .unwrap_or(crate::config::NGINX_CACHE_EXPIRY_DEFAULT.to_string());

    let nginx_cache_path = env
        .get("RIKU_NGINX_CACHE_PATH")
        .cloned()
        .unwrap_or(paths.cache_root.join(app).to_string_lossy().to_string());

    context.insert("RIKU_NGINX_CACHE_SIZE", &nginx_cache_size);
    context.insert("RIKU_NGINX_CACHE_TIME", &nginx_cache_time);
    context.insert("RIKU_NGINX_CACHE_REDIRECTS", &nginx_cache_redirects);
    context.insert("RIKU_NGINX_CACHE_ANY", &nginx_cache_any);
    context.insert("RIKU_NGINX_CACHE_CONTROL", &nginx_cache_control);
    context.insert("RIKU_NGINX_CACHE_EXPIRY", &nginx_cache_expiry);
    context.insert("RIKU_NGINX_CACHE_PATH", &nginx_cache_path);
}
