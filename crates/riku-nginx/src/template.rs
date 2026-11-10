//! The nginx Tera templates and the backend address check every render uses.

use anyhow::Result;

use super::sanitize::sanitize_nginx_value;
use crate::util::backend::Backend;

/// A backend as it is written in an `upstream` block. Nginx directive
/// characters are refused, and so is whitespace in a socket path, which would
/// let the rest of the path act as a `server` parameter such as `weight=0`.
pub(super) fn backend_address(backend: &Backend) -> Result<String> {
    let address = backend.to_string();
    if matches!(backend, Backend::Unix(_)) && address.contains(|c: char| c.is_ascii_whitespace()) {
        return Err(anyhow::anyhow!(
            "socket path '{address}' contains whitespace"
        ));
    }
    sanitize_nginx_value("RIKU_BACKENDS", &address)
}

/// Register all nginx Tera templates.
pub(super) fn register_templates(tera: &mut tera::Tera) -> Result<()> {
    let templates = [
        (
            "nginx_acme.conf.tera",
            include_str!("../../../templates/nginx_acme.conf.tera"),
        ),
        (
            "nginx_paths.conf.tera",
            include_str!("../../../templates/nginx_paths.conf.tera"),
        ),
        (
            "nginx_open_port.conf.tera",
            include_str!("../../../templates/nginx_open_port.conf.tera"),
        ),
        (
            "nginx_also_tls.conf.tera",
            include_str!("../../../templates/nginx_also_tls.conf.tera"),
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
