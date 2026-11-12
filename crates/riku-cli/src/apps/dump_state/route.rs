//! The router half of an app's state dump: which router serves the app
//! and whether its config is in place.

use serde::Serialize;
use std::path::Path;

use crate::config::router::Router;
use crate::config::RikuPaths;
use crate::util::fs::{presence, Presence};

/// How an app is routed, tagged by router so each carries only its own
/// fields.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "router", rename_all = "snake_case")]
pub enum RouteState {
    Nginx {
        config_exists: bool,
        /// False when this user cannot search the site root, so
        /// `config_exists` says nothing either way.
        config_known: bool,
        enabled: bool,
    },
    Caddy {
        /// The site file path, `None` when the app has no site or when
        /// `site_known` is false.
        site: Option<String>,
        /// False when this user cannot search the site root. The root
        /// owned `/etc/riku/caddy` is closed to the deploy user, so there
        /// `site` says nothing either way.
        site_known: bool,
    },
    Plugin {
        name: String,
    },
    /// The `ROUTER` file could not be read.
    Unknown {
        error: String,
    },
}

/// nginx loads every site config the router helper wrote once the host
/// include is in place, so a config is enabled exactly when both exist.
pub(super) fn nginx_site_state(site: &Path, paths: &RikuPaths) -> RouteState {
    let (config_exists, config_known) = match presence(site) {
        Presence::Present => (true, true),
        Presence::Missing => (false, true),
        Presence::Unknown => (false, false),
    };
    RouteState::Nginx {
        config_exists,
        config_known,
        enabled: config_exists && crate::nginx::includes_riku_sites(paths),
    }
}

fn caddy_state(app: &str, paths: &RikuPaths) -> RouteState {
    let site = crate::caddy::site_path(app, paths);
    match presence(&site) {
        Presence::Present => RouteState::Caddy {
            site: Some(site.display().to_string()),
            site_known: true,
        },
        Presence::Missing => RouteState::Caddy {
            site: None,
            site_known: true,
        },
        Presence::Unknown => RouteState::Caddy {
            site: None,
            site_known: false,
        },
    }
}

/// `app`'s route under the host router.
pub(super) fn route_state(app: &str, paths: &RikuPaths) -> RouteState {
    match Router::load(paths) {
        Ok(Router::Nginx) => nginx_site_state(&crate::nginx::site_path(app), paths),
        Ok(Router::Caddy) => caddy_state(app, paths),
        Ok(Router::Plugin(name)) => RouteState::Plugin { name },
        Err(e) => RouteState::Unknown {
            error: e.to_string(),
        },
    }
}
