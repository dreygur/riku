//! Nginx configuration generation module.
//!
//! Renders a site config from a checked route with the tera templates, and
//! holds what the deploy user still does for nginx: publishing static files
//! and issuing certificates. Writing the config and reloading nginx belong to the router helper.

// Dependency crates aliased as their former module names so internal
// `crate::config::` / `crate::util::` paths resolve unchanged.
pub(crate) use riku_config as config;
pub(crate) use riku_util as util;

mod backends;
mod default_server;
mod host;
mod remove;
mod route_site;
mod sanitize;
mod ssl;
mod ssl_single;
mod static_copy;
mod static_site;
mod template;

use std::path::Path;

pub use backends::{bind_address, default_backends, upstream_port_number};
pub use default_server::needs_catch_all;
pub use host::{include_file_contents, include_line, includes_riku_sites, write_include_file};
pub use remove::remove_deploy_files;
pub use route_site::{render_route_site, RouteSite, SiteFiles, SiteShape};
pub use sanitize::breaks_directive;
pub use ssl::ensure_ssl_certificates;
pub use ssl_single::issue_single;
pub use static_site::{publish_static, remove_static, static_locations};

/// Files an app might ship hoping to replace its generated nginx config.
/// None is used: nginx loads every riku site into one server-wide config,
/// so a file from an app repo could take over other apps' domains.
const UNSUPPORTED_APP_CONFIGS: [&str; 4] = [
    "nginx.conf",
    "nginx.custom.conf",
    "nginx.custom",
    ".nginx.conf",
];

/// Say so when an app ships a file that looks like its own nginx config.
pub fn warn_about_app_nginx_configs(app: &str, app_path: &Path) {
    for name in UNSUPPORTED_APP_CONFIGS {
        if !app_path.join(name).exists() {
            continue;
        }
        tracing::warn!("app '{app}' ships {name}, which riku ignores");
        crate::util::echo(
            &format!(
                "-----> Ignoring {name}: custom nginx configs are not supported, using the generated config"
            ),
            "yellow",
        );
    }
}
