//! Nginx config template rendering and config file installation.
//!
//! Registers Tera templates, selects the right template for an app's config,
//! renders the file, and manages the nginx sites-enabled symlink.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::context::build_context;
use super::sanitize::{sanitize_env_for_nginx, sanitize_nginx_value};
use super::validate::validate_nginx_config;
use crate::util::backend::Backend;

/// Use a custom nginx configuration file from the app directory.
pub(super) fn use_custom_nginx_config(
    custom_path: &Path,
    app: &str,
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let config_content = fs::read_to_string(custom_path)?;
    let config_file = paths.nginx_root.join(format!("{}.conf", app));
    crate::util::write_atomic(&config_file, config_content.as_bytes())?;
    validate_nginx_config(&config_file)?;
    crate::util::echo(
        &format!("-----> Custom nginx config installed for '{}'", app),
        "green",
    );
    Ok(())
}

/// Generate nginx configuration from template.
#[allow(clippy::question_mark)]
pub(super) fn generate_nginx_config_from_template(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let config_content = match render_nginx_config(app, app_path, env, backends, paths) {
        Ok(content) => content,
        Err(e) => return Err(e),
    };

    let config_file = paths.nginx_root.join(format!("{}.conf", app));
    if let Err(e) = crate::util::write_atomic(&config_file, config_content.as_bytes()) {
        return Err(e);
    }
    if let Err(e) = validate_nginx_config(&config_file) {
        return Err(e);
    }

    install_nginx_symlink(&config_file, app, paths);

    Ok(())
}

/// Render the app's config text without writing it.
///
/// Every backend address is checked for nginx directive characters before it
/// reaches the template, the same as the ENV values are.
#[allow(clippy::question_mark)]
pub(super) fn render_nginx_config(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<String> {
    let mut tera = tera::Tera::default();
    if let Err(e) = register_templates(&mut tera) {
        return Err(e);
    }

    let env = &sanitize_env_for_nginx(env);
    let mut context = match build_context(app, app_path, env, paths) {
        Ok(context) => context,
        Err(e) => return Err(e),
    };
    let addresses: Result<Vec<String>> = backends
        .iter()
        .map(|backend| sanitize_nginx_value("RIKU_BACKENDS", &backend.to_string()))
        .collect();
    match addresses {
        Ok(addresses) => context.insert("RIKU_BACKENDS", &addresses),
        Err(e) => return Err(e),
    }

    let template_name = select_template(env);
    match tera.render(template_name, &context) {
        Ok(content) => Ok(content),
        Err(e) => Err(anyhow::anyhow!(
            "failed to render {template_name} for '{app}': {e}"
        )),
    }
}

/// Register all nginx Tera templates.
fn register_templates(tera: &mut tera::Tera) -> Result<()> {
    let templates = [
        (
            "nginx.conf.tera",
            include_str!("../../../templates/nginx.conf.tera"),
        ),
        (
            "nginx_https_only.conf.tera",
            include_str!("../../../templates/nginx_https_only.conf.tera"),
        ),
        (
            "nginx_common.conf.tera",
            include_str!("../../../templates/nginx_common.conf.tera"),
        ),
        (
            "nginx_portmap.conf.tera",
            include_str!("../../../templates/nginx_portmap.conf.tera"),
        ),
        (
            "nginx_acme_firstrun.conf.tera",
            include_str!("../../../templates/nginx_acme_firstrun.conf.tera"),
        ),
        (
            "nginx_static.conf.tera",
            include_str!("../../../templates/nginx_static.conf.tera"),
        ),
        (
            "nginx_cache.conf.tera",
            include_str!("../../../templates/nginx_cache.conf.tera"),
        ),
        (
            "nginx_proxy.conf.tera",
            include_str!("../../../templates/nginx_proxy.conf.tera"),
        ),
        (
            "nginx_upstream.conf.tera",
            include_str!("../../../templates/nginx_upstream.conf.tera"),
        ),
        (
            "nginx_uwsgi.conf.tera",
            include_str!("../../../templates/nginx_uwsgi.conf.tera"),
        ),
        (
            "nginx_wsgi.conf.tera",
            include_str!("../../../templates/nginx_wsgi.conf.tera"),
        ),
    ];

    // Tera 2 resolves includes as each template is added, so registering
    // them one by one fails on the first template that includes a later one.
    if let Err(e) = tera.add_raw_templates(templates) {
        return Err(anyhow::anyhow!("failed to register nginx templates: {e}"));
    }
    Ok(())
}

/// Choose the appropriate template based on environment flags.
fn select_template(env: &HashMap<String, String>) -> &'static str {
    if crate::util::flag::https_only(env) {
        "nginx_https_only.conf.tera"
    } else if env.contains_key("RIKU_NGINX_WSGI") {
        "nginx_wsgi.conf.tera"
    } else if env.contains_key("RIKU_NGINX_PORTMAP") {
        "nginx_portmap.conf.tera"
    } else if env.contains_key("RIKU_NGINX_STATIC") {
        "nginx_static.conf.tera"
    } else {
        "nginx.conf.tera"
    }
}

/// Create or update the app's sites-enabled symlink and reload nginx.
///
/// Does nothing when the sites-enabled directory is absent, which is the
/// normal case on a host with no nginx installed.
fn install_nginx_symlink(config_file: &Path, app: &str, paths: &crate::config::RikuPaths) {
    let sites_enabled = &paths.nginx_sites_enabled;
    if !sites_enabled.exists() {
        return;
    }

    let symlink_path = sites_enabled.join(format!("{}.conf", app));

    if symlink_path.symlink_metadata().is_ok() {
        if let Err(e) = fs::remove_file(&symlink_path) {
            tracing::warn!(
                "could not remove old nginx symlink {:?}: {}",
                symlink_path,
                e
            );
        }
    }

    if let Err(e) = std::os::unix::fs::symlink(config_file, &symlink_path) {
        tracing::warn!("could not create nginx symlink {:?}: {}", symlink_path, e);
    } else {
        super::reload_nginx();
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
