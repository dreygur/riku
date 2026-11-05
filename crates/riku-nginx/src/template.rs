//! Nginx config template rendering and config file installation.
//!
//! Registers Tera templates, selects the right template for an app's config,
//! renders it, and installs it in `nginx_root`.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use super::context::build_context;
use super::install::{check_site, install_site};
use super::sanitize::{sanitize_env_for_nginx, sanitize_nginx_value};
use crate::util::backend::Backend;

/// Generate nginx configuration from template.
pub(super) fn generate_nginx_config_from_template(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let config_content = match render_nginx_config(app, app_path, env, backends, paths) {
        Ok(content) => content,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot render nginx config for '{app}': {e}"
            ))
        }
    };

    install_site(app, &config_content, paths)
}

/// Render the app's config for `backends` and have `nginx -t` check it,
/// without touching the live config. A deploy runs this before it starts a
/// copy, so a refused `RIKU_NGINX_INCLUDE_FILE` snippet or a config nginx
/// rejects fails the deploy while the old copies still serve alone.
pub fn check_nginx_config(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    let config_content = match render_nginx_config(app, app_path, env, backends, paths) {
        Ok(content) => content,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "cannot render nginx config for '{app}': {e}"
            ))
        }
    };
    check_site(app, &config_content, paths)
}

/// Render the app's config text without writing it.
///
/// Every backend address is checked before it reaches the template: nginx
/// directive characters are refused as for ENV values, and so is whitespace in
/// a socket path, which would let the rest of the path act as a `server`
/// parameter such as `weight=0`.
pub(super) fn render_nginx_config(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<String> {
    let mut tera = tera::Tera::default();
    if let Err(e) = register_templates(&mut tera) {
        return Err(anyhow::anyhow!("cannot load nginx templates: {e}"));
    }

    let env = &sanitize_env_for_nginx(env);
    let mut context = match build_context(app, app_path, env, paths) {
        Ok(context) => context,
        Err(e) => return Err(anyhow::anyhow!("cannot build nginx context: {e}")),
    };
    let addresses: Result<Vec<String>> = backends.iter().map(backend_address).collect();
    match addresses {
        Ok(addresses) => context.insert("RIKU_BACKENDS", &addresses),
        Err(e) => return Err(anyhow::anyhow!("refusing backend: {e}")),
    }

    let template_name = select_template(env);
    match tera.render(template_name, &context) {
        Ok(content) => Ok(content),
        Err(e) => Err(anyhow::anyhow!(
            "failed to render {template_name} for '{app}': {e}"
        )),
    }
}

fn backend_address(backend: &Backend) -> Result<String> {
    let address = backend.to_string();
    if matches!(backend, Backend::Unix(_)) && address.contains(|c: char| c.is_ascii_whitespace()) {
        return Err(anyhow::anyhow!(
            "socket path '{address}' contains whitespace"
        ));
    }
    sanitize_nginx_value("RIKU_BACKENDS", &address)
}

/// Register all nginx Tera templates.
fn register_templates(tera: &mut tera::Tera) -> Result<()> {
    let templates = [
        (
            "nginx_acme.conf.tera",
            include_str!("../../../templates/nginx_acme.conf.tera"),
        ),
        (
            "nginx_site_locations.conf.tera",
            include_str!("../../../templates/nginx_site_locations.conf.tera"),
        ),
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

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
