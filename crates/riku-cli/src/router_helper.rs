//! `riku router-helper`: the root process that will install router config on
//! behalf of the deploy user.
//!
//! Security model: runs as root and refuses to start as anyone else. It
//! trusts only the deploy user and root, decided in `riku_route::server`
//! from the kernel's peer credentials, and the socket it serves is group
//! restricted to the deploy user's primary group. The deploy user is read
//! from the passwd database (`RIKU_USER`, default `riku`) when the helper
//! starts, not from a request. A request names an app, which is validated as
//! an app name before use; nothing a request carries is a path.
//!
//! `apply` and `remove` install and remove the config of the router the
//! request names, through `riku_route::apply_nginx` and `remove_nginx` or
//! `apply_caddy` and `remove_caddy`. Each runs its route gate again and
//! writes only under `/etc/riku` and `/var/cache/riku`. The request, not the
//! host, names the router until the root owned router setting of the install
//! task exists; each router's files stay in that router's own directory.
//!
//! The helper reads no file the deploy user can write.

use std::path::Path;

use anyhow::{anyhow, Result};
use nix::unistd::Uid;
use riku_container::types::AppName;
use riku_route::{
    apply_caddy, apply_nginx, bind_socket, remove_caddy, remove_nginx, restrict_umask, run,
    CaddyRoots, NginxRoots, Reply, Request, RouterKind, Stage,
};

use crate::config::ROUTER_SOCKET;
use crate::setup::deploy_user::DeployUser;

fn not_root(euid: Uid) -> Option<anyhow::Error> {
    if euid.is_root() {
        return None;
    }
    Some(anyhow!(
        "riku router-helper must run as root; it installs router config the deploy user cannot write"
    ))
}

fn failed(stage: Stage, error: impl ToString) -> Reply {
    Reply::Failed {
        stage,
        error: error.to_string(),
    }
}

fn app_checked(app: &str) -> std::result::Result<AppName, Reply> {
    match AppName::try_from(app) {
        Ok(app) => Ok(app),
        Err(e) => Err(failed(Stage::Schema, e)),
    }
}

fn handle(nginx: &NginxRoots, caddy: &CaddyRoots, request: Request) -> Reply {
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
        Request::Status {} => Reply::Status {
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }
}

/// Serve the router helper socket until the process is stopped.
pub fn cmd_router_helper() -> Result<()> {
    if let Some(refusal) = not_root(Uid::effective()) {
        return Err(refusal);
    }
    restrict_umask();
    let deploy = match DeployUser::lookup() {
        Ok(deploy) => deploy,
        Err(e) => return Err(anyhow!("router helper needs the deploy user: {e}")),
    };
    let listener = match bind_socket(Path::new(ROUTER_SOCKET), deploy.gid()) {
        Ok(listener) => listener,
        Err(e) => return Err(anyhow!("router helper socket: {e}")),
    };
    let nginx = NginxRoots::production();
    let caddy = CaddyRoots::production();
    run(&listener, deploy.uid(), &mut |request| {
        handle(&nginx, &caddy, request)
    })
}

#[cfg(test)]
#[path = "router_helper_tests.rs"]
mod tests;
