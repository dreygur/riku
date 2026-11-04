//! Tera context construction for nginx config templates.
//!
//! Each helper populates a distinct section of the template context from
//! sanitized environment variables and resolved path defaults.

use std::collections::HashMap;
use std::path::Path;

use super::app_snippet::{insert_include_file, SNIPPET_KEY};
use super::backends::{socket_path, upstream_port};
use super::cloudflare::generate_cloudflare_ips_config;
use super::context_cache::insert_cache_context;
use super::ssl::ensure_ssl_certificates;
use super::static_site::{document_root, static_path_dir, static_paths};

/// Insert address-related context values.
pub(super) fn insert_address_context(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
    app: &str,
) {
    let bind_address = env
        .get("RIKU_BIND_ADDRESS")
        .cloned()
        .unwrap_or("127.0.0.1".to_string());

    let nginx_ipv4_address = env
        .get("RIKU_NGINX_IPV4_ADDRESS")
        .cloned()
        .unwrap_or("0.0.0.0".to_string());

    let disable_ipv6 = env
        .get("RIKU_DISABLE_IPV6")
        .map(|v| v.to_lowercase() == "true" || v == "1" || v == "yes")
        .unwrap_or(false);

    let nginx_ipv6_address = if disable_ipv6 {
        String::new()
    } else {
        env.get("RIKU_NGINX_IPV6_ADDRESS")
            .cloned()
            .unwrap_or("[::]".to_string())
    };

    let nginx_server_name = env
        .get("RIKU_DOMAINS")
        .cloned()
        .unwrap_or(format!("{}.example.com", app));

    let nginx_socket = socket_path(app, env, paths);

    context.insert("RIKU_BIND_ADDRESS", &bind_address);
    context.insert("RIKU_NGINX_IPV4_ADDRESS", &nginx_ipv4_address);
    context.insert("RIKU_NGINX_IPV6_ADDRESS", &nginx_ipv6_address);
    context.insert("RIKU_DOMAINS", &nginx_server_name);
    context.insert("RIKU_NGINX_SOCKET", &nginx_socket);
}

/// Insert where nginx finds the app's published static files: the document
/// root and one `location`/`alias` pair per `RIKU_NGINX_STATIC_PATHS` entry.
pub(super) fn insert_static_context(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
    app: &str,
) {
    let aliases: Vec<HashMap<&str, String>> = static_paths(env)
        .into_iter()
        .enumerate()
        .map(|(index, mapping)| {
            let alias = static_path_dir(app, paths, index);
            HashMap::from([
                ("location", mapping.location),
                ("alias", alias.to_string_lossy().into_owned()),
            ])
        })
        .collect();

    context.insert(
        "RIKU_NGINX_DOCUMENT_ROOT",
        &document_root(app, paths).to_string_lossy(),
    );
    context.insert("RIKU_STATIC_ALIASES", &aliases);
}

/// Insert feature-flag context values (Cloudflare ACL, git folder exposure).
pub(super) fn insert_feature_flags(
    context: &mut tera::Context,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) {
    let nginx_cloudflare_acl = env
        .get("RIKU_NGINX_CLOUDFLARE_ACL")
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
        .get("RIKU_NGINX_ALLOW_GIT_FOLDERS")
        .map(|v| v.to_lowercase() == "true" || v == "1" || v == "yes")
        .unwrap_or(false);

    context.insert(
        "RIKU_NGINX_CLOUDFLARE_ACL",
        &nginx_cloudflare_acl.to_string(),
    );
    context.insert(
        "RIKU_NGINX_ALLOW_GIT_FOLDERS",
        &nginx_allow_git_folders.to_string(),
    );
}

/// Insert portmap/proxy port context values.
pub(super) fn insert_portmap_context(context: &mut tera::Context, env: &HashMap<String, String>) {
    let nginx_external_port = env
        .get("RIKU_NGINX_EXTERNAL_PORT")
        .cloned()
        .unwrap_or("80".to_string());

    context.insert("RIKU_NGINX_EXTERNAL_PORT", &nginx_external_port);
    context.insert("RIKU_UPSTREAM_PORT", &upstream_port(env));
}

/// Build the full Tera context from sanitized environment variables and paths.
#[allow(clippy::question_mark)]
pub(super) fn build_context(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> anyhow::Result<tera::Context> {
    let mut context = tera::Context::new();

    // The snippet's file name stays out: templates get only its checked
    // content, never a path they could `include`.
    for (key, value) in env.iter().filter(|(key, _)| *key != SNIPPET_KEY) {
        context.insert(key.clone(), value);
    }
    // Inserted after the ENV copy so an app ENV key of the same name cannot
    // rename the upstream, the cache zone or the certificate files.
    context.insert("APP", app);
    context.insert("APP_UPSTREAM", &crate::backends::upstream_name(app));
    context.insert("INTERNAL_NGINX_APP_ROOT", &app_path.to_string_lossy());

    insert_address_context(&mut context, env, paths, app);
    insert_static_context(&mut context, env, paths, app);
    insert_cache_context(&mut context, env, paths, app);
    insert_feature_flags(&mut context, env, paths);
    if let Err(e) = insert_include_file(&mut context, env, app_path, paths) {
        return Err(e);
    }
    insert_portmap_context(&mut context, env);

    context.insert("RIKU_ROOT", &paths.riku_root.to_string_lossy());
    context.insert("RIKU_NGINX_ROOT", &paths.nginx_root.to_string_lossy());
    context.insert("ACME_WWW", &paths.acme_root.to_string_lossy());
    context.insert(
        "RIKU_ACME_ROOT_CA",
        &env.get("RIKU_ACME_ROOT_CA")
            .cloned()
            .unwrap_or_else(|| "letsencrypt.org".to_string()),
    );

    if crate::util::flag::https_only(env) {
        if let Some(server_name) = env.get("RIKU_DOMAINS") {
            let domains: Vec<String> = server_name
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            if !domains.is_empty() {
                if let Err(e) = ensure_ssl_certificates(app, &domains, paths) {
                    return Err(e);
                }
            }
        }
    }

    if let Some(socket) = env.get("UWSGI_SOCKET") {
        context.insert("UWSGI_SOCKET", socket);
    }

    Ok(context)
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
