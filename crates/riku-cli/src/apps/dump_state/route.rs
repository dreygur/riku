//! The router half of an app's state dump: which router serves the app
//! and whether its config is in place.

use serde::Serialize;

use crate::config::router::Router;
use crate::config::RikuPaths;

/// How an app is routed, tagged by router so each carries only its own
/// fields.
#[derive(Serialize)]
#[serde(tag = "router", rename_all = "snake_case")]
pub(super) enum RouteState {
    Nginx {
        config_exists: bool,
        enabled: bool,
    },
    Caddy {
        /// The site file path, `None` when the app has no site.
        site: Option<String>,
    },
    Plugin {
        name: String,
    },
    /// The `ROUTER` file could not be read.
    Unknown {
        error: String,
    },
}

fn nginx_state(app: &str, paths: &RikuPaths) -> RouteState {
    let config = paths.nginx_root.join(format!("{app}.conf"));
    let enabled = paths.nginx_sites_enabled.join(format!("{app}.conf"));
    RouteState::Nginx {
        config_exists: config.exists(),
        enabled: enabled.exists(),
    }
}

/// `app`'s route under the host router.
pub(super) fn route_state(app: &str, paths: &RikuPaths) -> RouteState {
    match Router::load(paths) {
        Ok(Router::Nginx) => nginx_state(app, paths),
        Ok(Router::Caddy) => {
            let site = crate::caddy::site_path(app, paths);
            RouteState::Caddy {
                site: site.exists().then(|| site.display().to_string()),
            }
        }
        Ok(Router::Plugin(name)) => RouteState::Plugin { name },
        Err(e) => RouteState::Unknown {
            error: e.to_string(),
        },
    }
}
