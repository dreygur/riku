//! Nginx configuration generation module.
//!
//! Generates nginx configuration files from templates using the tera templating engine.
//! Handles ACME challenge configs, SSL certificates, and Cloudflare ACLs.

// Dependency crates aliased as their former module names so internal
// `crate::config::` / `crate::util::` paths resolve unchanged.
pub(crate) use riku_config as config;
pub(crate) use riku_util as util;

mod app_snippet;
mod backends;
mod cloudflare;
mod context;
mod context_cache;
mod default_server;
mod host;
mod host_check;
mod install;
mod reload;
mod remove;
mod sanitize;
mod snippet_grammar;
mod snippet_limits;
mod ssl;
mod static_copy;
mod static_site;
mod template;
mod validate;

#[cfg(test)]
mod env_options_tests;
#[cfg(test)]
mod filter_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::util::backend::Backend;
pub use app_snippet::{app_snippet, check_snippet_text};
pub use backends::{bind_address, default_backends, upstream_port_number};
pub use default_server::needs_catch_all;
pub use host::{
    breaks_directive, include_file_contents, include_line, includes_riku_sites, write_include_file,
};
pub use host_check::{check_host_ready, stray_site_configs, HostReadiness};
pub use reload::reload_nginx_with;
pub use remove::{remove_nginx_files, NginxRemoval};
pub use static_site::{remove_static, static_locations};
pub use template::check_nginx_config;
use template::generate_nginx_config_from_template;

/// Files an app might ship hoping to replace its generated nginx config.
/// None is used: nginx loads every riku site into one server-wide config,
/// so a file from an app repo could take over other apps' domains.
const UNSUPPORTED_APP_CONFIGS: [&str; 4] = [
    "nginx.conf",
    "nginx.custom.conf",
    "nginx.custom",
    ".nginx.conf",
];

fn warn_about_app_nginx_configs(app: &str, app_path: &Path) {
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

/// Generate and install the nginx config for an app from the templates.
/// An nginx config file in the app's own directory is ignored with a warning.
///
/// `backends` are the upstream servers the app's locations proxy to. An empty
/// list is refused, since nginx rejects an upstream with no servers.
///
/// The app's static files are copied to `<static_root>/<app>` first, so the
/// config never points nginx at a copy that is not there yet.
#[allow(clippy::question_mark)]
pub fn generate_nginx_config(
    app: &str,
    app_path: &Path,
    env: &HashMap<String, String>,
    backends: &[Backend],
    paths: &crate::config::RikuPaths,
) -> Result<()> {
    if backends.is_empty() {
        return Err(anyhow::anyhow!("no backends to route '{app}' to"));
    }
    warn_about_app_nginx_configs(app, app_path);
    if let Err(e) = static_site::publish_static(app, app_path, env, paths) {
        return Err(anyhow::anyhow!(
            "cannot publish static files for '{app}': {e}"
        ));
    }
    generate_nginx_config_from_template(app, app_path, env, backends, paths)
}
