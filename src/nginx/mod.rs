//! Nginx configuration generation module.
//!
//! Generates nginx configuration files from templates using the tera templating engine.
//! Handles custom configs, ACME challenge configs, SSL certificates, and Cloudflare ACLs.

mod cloudflare;
mod context;
mod sanitize;
mod ssl;
mod template;
mod validate;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use template::{generate_nginx_config_from_template, use_custom_nginx_config};
use validate::validate_nginx_config;

/// Generate nginx configuration for an app.
/// Checks for custom nginx config first, otherwise generates from template.
pub fn generate_nginx_config(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    // Check for custom nginx config files in the app directory
    let custom_configs = [
        "nginx.conf",
        "nginx.custom.conf",
        "nginx.custom",
        ".nginx.conf",
    ];

    for custom_config in &custom_configs {
        let custom_path = app_path.join(custom_config);
        if custom_path.exists() {
            crate::util::echo(
                &format!("-----> Using custom nginx config: {}", custom_config),
                "green",
            );
            return use_custom_nginx_config(&custom_path, app, paths);
        }
    }

    // No custom config found, generate from template
    generate_nginx_config_from_template(app, app_path, env, paths)
}

/// Ask the running nginx master process to reload its configuration
/// (`nginx -s reload`) — a graceful reload that finishes in-flight
/// connections on old worker processes while new connections pick up the
/// refreshed config, never dropping active traffic the way a restart
/// would. Best-effort: returns `false` (and logs a warning) if nginx isn't
/// installed/running or the reload command fails, rather than treating a
/// missing nginx as fatal to whatever triggered the reload.
pub fn reload_nginx() -> bool {
    match std::process::Command::new("nginx").args(["-s", "reload"]).output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            tracing::warn!(
                "nginx reload failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            false
        }
        Err(e) => {
            tracing::warn!("could not reload nginx: {}", e);
            false
        }
    }
}

/// Remove nginx configuration for an app.
pub fn remove_nginx_config(app: &str, paths: &crate::config::RikuPaths) -> Result<()> {
    let config_file = paths.nginx_root.join(format!("{}.conf", app));
    if config_file.exists() {
        fs::remove_file(&config_file)?;
    }

    // Remove symlink from /etc/nginx/sites-enabled/ if it exists
    let nginx_sites_enabled = Path::new("/etc/nginx/sites-enabled");
    if nginx_sites_enabled.exists() {
        let symlink_path = nginx_sites_enabled.join(format!("{}.conf", app));
        if symlink_path.exists() {
            let _ = fs::remove_file(&symlink_path);
            reload_nginx();
        }
    }

    // Also remove associated socket, cert, and key files
    for ext in ["sock", "key", "crt"] {
        let file = paths.nginx_root.join(format!("{}.{}", app, ext));
        if file.exists() {
            fs::remove_file(&file)?;
        }
    }

    Ok(())
}

/// Generate a minimal nginx configuration for ACME challenges.
pub fn generate_acme_nginx_config(paths: &crate::config::RikuPaths) -> Result<()> {
    let mut tera = tera::Tera::default();
    tera.add_raw_template(
        "acme.conf.tera",
        include_str!("../../templates/nginx_acme_firstrun.conf.tera"),
    )?;

    let mut context = tera::Context::new();
    context.insert("ACME_WWW", &paths.acme_www.to_string_lossy());
    context.insert("NGINX_IPV4_ADDRESS", "0.0.0.0");
    context.insert("NGINX_IPV6_ADDRESS", "[::]");

    let config_content = tera.render("acme.conf.tera", &context)?;

    let config_file = paths.nginx_root.join("acme.conf");
    crate::util::write_atomic(&config_file, config_content.as_bytes())?;

    validate_nginx_config(&config_file)?;

    Ok(())
}
