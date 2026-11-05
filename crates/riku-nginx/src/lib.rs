//! Nginx configuration generation module.
//!
//! Renders a site config from a checked route with the tera templates, and
//! holds what the deploy user still does for nginx: publishing static files,
//! issuing certificates and reading an app's include snippet. Writing the
//! config and reloading nginx belong to the router helper.

// Dependency crates aliased as their former module names so internal
// `crate::config::` / `crate::util::` paths resolve unchanged.
pub(crate) use riku_config as config;
pub(crate) use riku_util as util;

mod app_snippet;
mod backends;
mod cloudflare;
mod default_server;
mod host;
mod remove;
mod route_site;
mod sanitize;
mod snippet_grammar;
mod snippet_limits;
mod ssl;
mod static_copy;
mod static_site;
mod template;

use std::path::Path;

pub use app_snippet::{app_snippet, check_snippet_text, snippet_directives};
pub use backends::{bind_address, default_backends, upstream_port_number};
pub use cloudflare::{cloudflare_allow_config, fetch_cloudflare_url};
pub use default_server::needs_catch_all;
pub use host::{
    breaks_directive, include_file_contents, include_line, includes_riku_sites, write_include_file,
};
pub use remove::remove_deploy_files;
pub use route_site::{render_route_site, RouteSite, SiteCache, SiteFiles, SiteShape};
pub use ssl::ensure_ssl_certificates;
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
