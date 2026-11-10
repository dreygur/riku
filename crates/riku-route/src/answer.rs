//! Dispatches one helper request to the router it names.
//!
//! Security: the app name is validated here before it reaches either router,
//! and each router's apply and remove run the route gate again. Nothing in a
//! request is a path.

use riku_container::types::AppName;

use crate::caddy_apply::{apply_caddy, remove_caddy, CaddyRoots};
use crate::nginx_apply::{apply_nginx, remove_nginx};
use crate::nginx_roots::NginxRoots;
use crate::paths_apply::{apply_paths_caddy, apply_paths_nginx};
use crate::protocol::{Reply, Request, RouterKind, Stage};

pub(crate) fn failed(stage: Stage, error: impl ToString) -> Reply {
    Reply::Failed {
        stage,
        error: error.to_string(),
    }
}

fn app_checked(app: &str) -> Result<AppName, Reply> {
    match AppName::try_from(app) {
        Ok(app) => Ok(app),
        Err(e) => Err(failed(Stage::Schema, e)),
    }
}

/// The reply to `request`, installing or removing config under the given
/// roots. `Status` answers with this build's version.
pub fn answer(nginx: &NginxRoots, caddy: &CaddyRoots, request: Request) -> Reply {
    match request {
        Request::Apply { app, router, route } => match app_checked(&app) {
            Ok(app) => match router {
                RouterKind::Nginx => apply_nginx(nginx, &app, &route),
                RouterKind::Caddy => apply_caddy(caddy, &app, &route),
            },
            Err(reply) => reply,
        },
        Request::Remove { app, router } => match app_checked(&app) {
            Ok(app) => match router {
                RouterKind::Nginx => remove_nginx(nginx, &app),
                RouterKind::Caddy => remove_caddy(caddy, &app),
            },
            Err(reply) => reply,
        },
        Request::ApplyPaths {
            router,
            listen,
            apps,
        } => match router {
            RouterKind::Nginx => apply_paths_nginx(nginx, &listen, &apps),
            RouterKind::Caddy => apply_paths_caddy(caddy, &listen, &apps),
        },
        Request::Status {} => Reply::Status {
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }
}
